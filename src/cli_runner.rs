use crate::cli::CliArgs;
use crate::config::Config;
use crate::cache::{CacheLayer, FileState};
use crate::render::Renderer;
use std::path::{Path, PathBuf};

pub fn run_cli(args: CliArgs, config: Config) {
    let file_path = Path::new(&args.file_path);
    if !file_path.exists() {
        eprintln!("Error: File '{}' does not exist.", args.file_path);
        std::process::exit(1);
    }

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
                // Edge case: cache entry exists but PNG was deleted. We need to regenerate.
                regenerate_and_update(&cache_layer, file_path, mtime, size, hash, &p, config.max_resolution);
            } else {
                let _ = cache_layer.update_cache(file_path, mtime, size, hash, &p);
            }
            (p, false)
        }
        FileState::Modified(p) => {
            regenerate_and_update(&cache_layer, file_path, mtime, size, hash, &p, config.max_resolution);
            (p, true)
        }
        FileState::New(p) => {
            regenerate_and_update(&cache_layer, file_path, mtime, size, hash, &p, config.max_resolution);
            (p, false)
        }
    };

    // Use viuer to render the image inline to standard output
    let conf = viuer::Config {
        transparent: true,
        absolute_offset: false,
        ..Default::default()
    };
    if let Err(e) = viuer::print_from_file(&img_path, &conf) {
        eprintln!("Error rendering image with viuer: {}", e);
    }

    // Print Metadata & Security Output
    let hash_hex = hex::encode(hash);
    println!("\nFile: {}", args.file_path);
    println!("Size: {} bytes", size);
    println!("BLAKE3 Hash: {}", hash_hex);

    if was_modified {
        println!("\n\x1b[31;1m[!] WARNING: FILE WAS SILENTLY MODIFIED SINCE LAST VIEWED [!]\x1b[0m");
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
