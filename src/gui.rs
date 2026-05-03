use eframe::egui;
use crate::cli::CliArgs;
use crate::config::Config;
use crate::cache::{CacheLayer, FileState};
use crate::render::{Renderer, xy2d};
use std::path::{Path, PathBuf};
use image::ImageReader;

pub fn get_eframe_options() -> eframe::NativeOptions {
    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_decorations(false)
        .with_transparent(false) // Changed to false for correct image display
        .with_inner_size([800.0, 850.0])
        .with_min_inner_size([600.0, 650.0])
        .with_position(egui::Pos2::new(100.0, 100.0)); // fallback if compositor ignores centering
    options
}

pub struct PhospheneApp {
    filename: String,
    size_bytes: u64,
    was_modified: bool,
    texture: Option<egui::TextureHandle>,
    img_path: Option<PathBuf>,
    zoom_level: f32,
    base_img_size: egui::Vec2,
    file_bytes: Vec<u8>, // We load the actual file bytes for the interactive tooltip
    max_res: u32,
}

impl PhospheneApp {
    pub fn new(cc: &eframe::CreationContext<'_>, args: CliArgs, config: Config) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let file_path = Path::new(&args.file_path);
        if !file_path.exists() {
            eprintln!("Error: File '{}' does not exist.", args.file_path);
            std::process::exit(1);
        }

        let filename = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let cache_dir = PathBuf::from(&config.cache_dir);
        let cache_layer = match CacheLayer::new(cache_dir.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error initializing cache layer: {}", e);
                std::process::exit(1);
            }
        };

        let (state, hash, mtime, size) = match cache_layer.check_file(file_path) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Error checking file in cache: {}", e);
                std::process::exit(1);
            }
        };

        let (img_path, was_modified) = match state {
            FileState::Unchanged(p) => {
                if !p.exists() {
                    Self::regenerate_and_update(&cache_layer, file_path, mtime, size, hash, &p, config.max_resolution);
                } else {
                    // Always update mtime on unchanged file to avoid cache miss loop
                    let _ = cache_layer.update_cache(file_path, mtime, size, hash, &p);
                }
                (p, false)
            }
            FileState::Modified(p) => {
                Self::regenerate_and_update(&cache_layer, file_path, mtime, size, hash, &p, config.max_resolution);
                (p, true)
            }
            FileState::New(p) => {
                Self::regenerate_and_update(&cache_layer, file_path, mtime, size, hash, &p, config.max_resolution);
                (p, false)
            }
        };

        // For the tooltip, we read the bytes into memory (could also use memmap2 here but this is simpler for UI logic)
        let file_bytes = std::fs::read(file_path).unwrap_or_default();

        Self {
            filename,
            size_bytes: size,
            was_modified,
            texture: None,
            img_path: Some(img_path),
            zoom_level: 1.0,
            base_img_size: egui::vec2(0.0, 0.0),
            file_bytes,
            max_res: config.max_resolution,
        }
    }

    fn regenerate_and_update(cache: &CacheLayer, file_path: &Path, mtime: u64, size: u64, hash: [u8; 32], img_path: &Path, max_res: u32) {
        let file = match std::fs::File::open(file_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file for rendering: {}", e);
                std::process::exit(1);
            }
        };

        let mmap = unsafe { memmap2::MmapOptions::new().map(&file) }.unwrap_or_else(|_| memmap2::MmapMut::map_anon(0).unwrap().make_read_only().unwrap());

        if let Err(e) = Renderer::generate_image(&mmap, max_res, img_path) {
            eprintln!("Error generating image: {}", e);
            std::process::exit(1);
        }

        if let Err(e) = cache.update_cache(file_path, mtime, size, hash, img_path) {
            eprintln!("Error updating cache: {}", e);
            std::process::exit(1);
        }
    }

    fn draw_legend(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("0x00 (Low Entropy / Nulls)").color(egui::Color32::from_rgb(68, 1, 84)));
            let (rect, _resp) = ui.allocate_exact_size(egui::vec2(100.0, 10.0), egui::Sense::hover());
            let num_steps = crate::render::VIRIDIS_MAP.len();
            for i in 0..num_steps {
                let color = crate::render::VIRIDIS_MAP[i];
                let color32 = egui::Color32::from_rgb(color[0], color[1], color[2]);
                let w = rect.width() / num_steps as f32;
                let x = rect.left() + (i as f32) * w;
                let step_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, rect.height()));
                ui.painter().rect_filled(step_rect, 0.0, color32);
            }
            ui.label(egui::RichText::new("0xFF (High Entropy / Crypto)").color(egui::Color32::from_rgb(253, 231, 37)));
        });
    }
}

impl eframe::App for PhospheneApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Exit on ESC
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            std::process::exit(0);
        }

        // Handle zooming
        self.zoom_level += ctx.input(|i| i.smooth_scroll_delta.y) * 0.005;
        self.zoom_level = self.zoom_level.clamp(0.1, 10.0); // Limit zoom

        // Load texture on first frame
        if self.texture.is_none() {
            if let Some(path) = &self.img_path {
                if let Ok(img) = ImageReader::open(path).unwrap().decode() {
                    let size = [img.width() as _, img.height() as _];
                    self.base_img_size = egui::vec2(img.width() as f32, img.height() as f32);
                    let image_buffer = img.to_rgba8();
                    let pixels = image_buffer.as_flat_samples();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        size,
                        pixels.as_slice(),
                    );
                    self.texture = Some(ctx.load_texture(
                        "hilbert_curve",
                        color_image,
                        egui::TextureOptions::NEAREST // Changed to NEAREST for sharp pixels
                    ));
                }
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(20, 20, 20)))
            .show(ctx, |ui| {

            // Allow dragging the window by clicking on the background
            let interact = ui.interact(ui.max_rect(), ui.id().with("background"), egui::Sense::drag());
            if interact.dragged() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            ui.vertical(|ui| {
                if let Some(texture) = &self.texture {
                    let available_size = ui.available_size();
                    // Image takes most space, leave space for footer and legend
                    let max_img_size = egui::vec2(available_size.x, available_size.y - 60.0);

                    egui::ScrollArea::both().show(ui, |ui| {
                        let mut img_size = self.base_img_size * self.zoom_level;
                        if self.zoom_level == 1.0 {
                            // Scale to fill the view naturally on startup, maintaining aspect ratio
                            let aspect = img_size.x / img_size.y;
                            if max_img_size.x / max_img_size.y > aspect {
                                img_size = egui::vec2(max_img_size.y * aspect, max_img_size.y);
                            } else {
                                img_size = egui::vec2(max_img_size.x, max_img_size.x / aspect);
                            }
                        }

                        let img = egui::Image::new(texture).fit_to_exact_size(img_size).sense(egui::Sense::click_and_drag());

                        let response = ui.add(img);

                        // Dragging the image itself should also move the window
                        if response.dragged() {
                             ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }

                        // Interactive Tooltip
                        if response.hovered() {
                            if let Some(pos) = response.hover_pos() {
                                // Convert pointer pos to relative image coords
                                let rect = response.rect;
                                let rel_x = (pos.x - rect.left()) / rect.width();
                                let rel_y = (pos.y - rect.top()) / rect.height();

                                // Convert relative to absolute grid pixel (bound by max_res)
                                let grid_x = (rel_x * self.max_res as f32).floor() as u32;
                                let grid_y = (rel_y * self.max_res as f32).floor() as u32;

                                let grid_x = grid_x.clamp(0, self.max_res - 1);
                                let grid_y = grid_y.clamp(0, self.max_res - 1);

                                // Map 2D to 1D index
                                let p = xy2d(self.max_res, grid_x, grid_y);
                                let total_pixels = self.max_res * self.max_res;

                                // Map 1D index to file offset
                                let file_size = self.file_bytes.len();
                                if file_size > 0 {
                                    let offset = ((p as f64 / total_pixels as f64) * file_size as f64) as usize;
                                    let offset = offset.clamp(0, file_size - 1);

                                    let byte_val = self.file_bytes[offset];
                                    let ascii_char = if byte_val >= 32 && byte_val <= 126 {
                                        byte_val as char
                                    } else {
                                        '.'
                                    };

                                    egui::show_tooltip_at_pointer(ctx, ui.layer_id(), egui::Id::new("tooltip"), |ui: &mut egui::Ui| {
                                        ui.label(format!("Offset: 0x{:08X}", offset));
                                        ui.label(format!("Value: 0x{:02X}", byte_val));
                                        ui.label(format!("ASCII: {}", ascii_char));
                                    });
                                }
                            }
                        }
                    });
                }

                // Legend
                self.draw_legend(ui);

                // Minimalist Footer
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&self.filename).color(egui::Color32::WHITE));
                    ui.label(egui::RichText::new(format!("{} bytes", self.size_bytes)).color(egui::Color32::GRAY));

                    if self.was_modified {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("⚠ MODIFIED").color(egui::Color32::RED).strong());
                        });
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("[ X ]").clicked() {
                            std::process::exit(0);
                        }
                    });
                });
            });

            // Add resize grip to bottom right corner
            let rect = ui.max_rect();
            let size = 16.0;
            let resize_rect = egui::Rect::from_min_max(rect.max - egui::vec2(size, size), rect.max);
            let resize_interact = ui.interact(resize_rect, ui.id().with("resize_grip"), egui::Sense::drag());
            if resize_interact.hovered() {
                 ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeNwSe);
            }
            if resize_interact.dragged() {
                 ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast));
            }
        });
    }
}
