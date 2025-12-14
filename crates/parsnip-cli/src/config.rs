//! CLI configuration

use std::path::PathBuf;

/// Get default data directory
pub fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".parsnip")
}

/// Configuration for the CLI
#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub default_project: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            default_project: "default".to_string(),
        }
    }
}
