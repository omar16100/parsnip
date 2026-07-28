//! MCP tool call request and response types.
//!
//! Tool dispatch itself lives in [`crate::server`]. This module previously also held a
//! `ToolHandler` with a second, older implementation of eight tools; it was never
//! constructed and diverged from the live dispatch path, so it has been removed.

use serde::{Deserialize, Serialize};

/// MCP tool call request
#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// MCP tool call response
#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
}

/// Content block for responses
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

impl ToolCallResponse {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
            is_error: None,
        }
    }

    pub fn json<T: Serialize>(data: &T) -> Self {
        match serde_json::to_string_pretty(data) {
            Ok(json) => Self::text(json),
            Err(e) => Self::error(format!("JSON serialization error: {}", e)),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: message.into(),
            }],
            is_error: Some(true),
        }
    }
}
