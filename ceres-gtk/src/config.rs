use adw::glib;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Configuration data that persists across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Color correction mode
    pub color_correction: String,

    /// Game Boy model: "dmg", "mgb", or "cgb"
    pub gb_model: String,

    /// Path to the last opened ROM file
    pub last_rom_path: Option<String>,

    /// Whether to use pixel-perfect scaling
    pub pixel_perfect: bool,

    /// Shader mode: "Nearest", "Scale2x", "Scale3x", "LCD", "CRT"
    pub shader_mode: String,

    /// Volume level (0.0 to 1.0)
    pub volume: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            last_rom_path: None,
            gb_model: "cgb".to_owned(),
            shader_mode: "Nearest".to_owned(),
            pixel_perfect: false,
            color_correction: "Modern Balanced".to_owned(),
            volume: 1.0,
        }
    }
}

impl Config {
    /// Returns the path where the config file is stored
    fn config_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(ceres_std::cli::CERES_BIN);
        path.push("config.json");
        path
    }

    /// Load configuration from disk, or return default if it doesn't exist
    pub fn load() -> Self {
        let config_path = Self::config_path();

        fs::read_to_string(&config_path).map_or_else(
            |_| Self::default(),
            |contents| match serde_json::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Failed to parse config file: {}. Using defaults.", e);
                    Self::default()
                }
            },
        )
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to pretty JSON
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, contents)?;

        Ok(())
    }
}
