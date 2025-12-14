//! Output formatting utilities

use serde::Serialize;

/// Output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

impl From<&str> for OutputFormat {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            "csv" => Self::Csv,
            _ => Self::Table,
        }
    }
}

/// Format output based on format type
pub fn format_output<T: Serialize>(data: &T, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".to_string())
        }
        OutputFormat::Csv => {
            // TODO: Implement CSV formatting
            "CSV output not yet implemented".to_string()
        }
        OutputFormat::Table => {
            // TODO: Implement table formatting
            "Table output not yet implemented".to_string()
        }
    }
}
