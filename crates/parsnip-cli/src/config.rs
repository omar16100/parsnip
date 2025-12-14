//! CLI configuration with TOML support

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Get default config directory
pub fn default_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("parsnip");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".parsnip")
}

/// Get default data directory
pub fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("parsnip")
}

/// Get config file path
pub fn config_file_path() -> PathBuf {
    default_config_dir().join("config.toml")
}

/// Configuration for the CLI (serializable to TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default project namespace
    #[serde(default = "default_project")]
    pub default_project: String,

    /// Data directory for database and indices
    #[serde(default)]
    pub data_dir: Option<PathBuf>,

    /// Log level (error, warn, info, debug, trace)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Default output format (table, json, csv)
    #[serde(default = "default_output_format")]
    pub output_format: String,
}

fn default_project() -> String {
    "default".to_string()
}

fn default_log_level() -> String {
    "warn".to_string()
}

fn default_output_format() -> String {
    "table".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_project: default_project(),
            data_dir: None,
            log_level: default_log_level(),
            output_format: default_output_format(),
        }
    }
}

impl Config {
    /// Load config from file, or return defaults if not found
    pub fn load() -> Self {
        let path = config_file_path();
        tracing::debug!("Loading config from: {:?}", path);

        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => {
                    tracing::debug!("Loaded config successfully");
                    config
                }
                Err(e) => {
                    tracing::warn!("Failed to parse config file: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("Failed to read config file: {}", e);
                }
                Self::default()
            }
        }
    }

    /// Save config to file
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_file_path();
        let dir = path.parent().expect("config path has parent");

        std::fs::create_dir_all(dir)?;

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        tracing::info!("Saved config to {:?}", path);
        Ok(())
    }

    /// Get a config value by key
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "default_project" => Some(self.default_project.clone()),
            "data_dir" => self.data_dir.as_ref().map(|p| p.display().to_string()),
            "log_level" => Some(self.log_level.clone()),
            "output_format" => Some(self.output_format.clone()),
            _ => None,
        }
    }

    /// Set a config value by key
    pub fn set(&mut self, key: &str, value: &str) -> anyhow::Result<()> {
        match key {
            "default_project" => self.default_project = value.to_string(),
            "data_dir" => self.data_dir = Some(PathBuf::from(value)),
            "log_level" => {
                if !["error", "warn", "info", "debug", "trace"].contains(&value) {
                    anyhow::bail!("Invalid log level: {}", value);
                }
                self.log_level = value.to_string();
            }
            "output_format" => {
                if !["table", "json", "csv"].contains(&value) {
                    anyhow::bail!("Invalid output format: {}", value);
                }
                self.output_format = value.to_string();
            }
            _ => anyhow::bail!("Unknown config key: {}", key),
        }
        Ok(())
    }

    /// List all config keys
    pub fn keys() -> Vec<&'static str> {
        vec!["default_project", "data_dir", "log_level", "output_format"]
    }

    /// Get the effective data directory
    #[allow(dead_code)]
    pub fn effective_data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(default_data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_project, "default");
        assert_eq!(config.log_level, "warn");
        assert_eq!(config.output_format, "table");
        assert!(config.data_dir.is_none());
    }

    #[test]
    fn test_toml_roundtrip() {
        let config = Config {
            default_project: "myproject".to_string(),
            log_level: "debug".to_string(),
            ..Default::default()
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(loaded.default_project, "myproject");
        assert_eq!(loaded.log_level, "debug");
    }

    #[test]
    fn test_get_set() {
        let mut config = Config::default();

        config.set("default_project", "test").unwrap();
        assert_eq!(config.get("default_project"), Some("test".to_string()));

        config.set("log_level", "info").unwrap();
        assert_eq!(config.get("log_level"), Some("info".to_string()));
    }

    #[test]
    fn test_invalid_values() {
        let mut config = Config::default();

        assert!(config.set("log_level", "invalid").is_err());
        assert!(config.set("output_format", "xml").is_err());
        assert!(config.set("unknown_key", "value").is_err());
    }
}
