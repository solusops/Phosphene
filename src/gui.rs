use eframe::egui;
use crate::cli::CliArgs;
use crate::config::Config;
use crate::cache::{CacheLayer, FileState};
use crate::render::Renderer;
use std::path::{Path, PathBuf};
use image::ImageReader;

pub fn get_eframe_options() -> eframe::NativeOptions {
    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_decorations(false)
        .with_transparent(false); // Changed to false for correct image display
    options
}

pub struct PhospheneApp {
    filename: String,
    size_bytes: u64,
    was_modified: bool,
    texture: Option<egui::TextureHandle>,
    img_path: Option<PathBuf>,
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

        Self {
            filename,
            size_bytes: size,
            was_modified,
            texture: None,
            img_path: Some(img_path),
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
}

impl eframe::App for PhospheneApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Exit on ESC
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            std::process::exit(0);
        }

        // Load texture on first frame
        if self.texture.is_none() {
            if let Some(path) = &self.img_path {
                if let Ok(img) = ImageReader::open(path).unwrap().decode() {
                    let size = [img.width() as _, img.height() as _];
                    let image_buffer = img.to_rgba8();
                    let pixels = image_buffer.as_flat_samples();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        size,
                        pixels.as_slice(),
                    );
                    self.texture = Some(ctx.load_texture(
                        "hilbert_curve",
                        color_image,
                        egui::TextureOptions::LINEAR
                    ));

                    // Resize window to match image size + footer space
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                        egui::vec2(img.width() as f32, img.height() as f32 + 30.0)
                    ));
                }
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(20, 20, 20)))
            .show(ctx, |ui| {

            ui.vertical(|ui| {
                if let Some(texture) = &self.texture {
                    let available_size = ui.available_size();
                    // Image takes most space, leave 30px for footer
                    let img_size = egui::vec2(available_size.x, available_size.y - 30.0);
                    ui.image((texture.id(), img_size));
                }

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
                });
            });
        });
    }
}
