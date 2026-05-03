use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_max_resolution")]
    pub max_resolution: u32,
    #[serde(default = "default_color_theme")]
    pub color_theme: String,
    #[serde(default = "default_window_spawn_behavior")]
    pub window_spawn_behavior: String,
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
}

fn default_max_resolution() -> u32 { 1024 }
fn default_color_theme() -> String { "viridis".to_string() }
fn default_window_spawn_behavior() -> String { "center".to_string() }
fn default_cache_dir() -> String {
    if let Some(proj_dirs) = ProjectDirs::from("com", "phosphene", "phosphene") {
        proj_dirs.cache_dir().to_string_lossy().to_string()
    } else {
        "~/.cache/phosphene".to_string() // Fallback
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_resolution: default_max_resolution(),
            color_theme: default_color_theme(),
            window_spawn_behavior: default_window_spawn_behavior(),
            cache_dir: default_cache_dir(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("com", "phosphene", "phosphene") {
            let config_dir = proj_dirs.config_dir();
            let config_file = config_dir.join("config.toml");

            if let Ok(content) = fs::read_to_string(&config_file) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Config::default()
    }
}
