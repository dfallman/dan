use serde::Deserialize;
use std::path::PathBuf;

/// Editor configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Tab width in spaces.
    pub tab_width: usize,
    /// Whether to expand tabs to spaces.
    pub expand_tab: bool,
    /// Show line numbers.
    pub line_numbers: bool,
    /// Scroll padding (lines above/below cursor to keep visible).
    pub scroll_off: usize,
    /// Theme name.
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
            expand_tab: true,
            line_numbers: true,
            scroll_off: 5,
            theme: "default".to_string(),
        }
    }
}

impl Config {
    /// Load config from the default config path (~/.config/dan/config.toml).
    pub fn load() -> Self {
        if let Some(path) = config_path() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(config) = toml::from_str(&content) {
                        return config;
                    }
                }
            }
        }
        Self::default()
    }
}

/// Get the config file path.
fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("dan").join("config.toml"))
}
