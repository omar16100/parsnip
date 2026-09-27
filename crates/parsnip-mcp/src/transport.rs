//! MCP transport implementations

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// The only JSON-RPC version this server speaks.
pub const JSONRPC_VERSION: &str = "2.0";

fn jsonrpc_version() -> String {
    JSONRPC_VERSION.to_string()
}

/// JSON-RPC request
///
/// `Serialize` as well as `Deserialize` so remote clients can build requests with the same
/// type the server parses them with.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

impl JsonRpcRequest {
    /// Build a request for a client call.
    pub fn new(
        id: serde_json::Value,
        method: impl Into<String>,
        params: serde_json::Value,
    ) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            id,
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC response
///
/// `jsonrpc` is an owned `String` rather than `&'static str` so this can derive `Deserialize`
/// for client use.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    /// Structured payload; carries the storage error kind for remote clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Error response carrying a structured `data` payload.
    pub fn error_with_data(
        id: serde_json::Value,
        code: i32,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
        }
    }
}

/// Stdio transport for MCP
pub struct StdioTransport {
    reader: BufReader<tokio::io::Stdin>,
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioTransport {
    /// Create a new stdio transport
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(tokio::io::stdin()),
        }
    }

    /// Read a JSON-RPC request from stdin
    pub async fn read_request(&mut self) -> std::io::Result<Option<JsonRpcRequest>> {
        let mut line = String::new();

        if self.reader.read_line(&mut line).await? == 0 {
            return Ok(None);
        }

        let request: JsonRpcRequest = serde_json::from_str(&line)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(Some(request))
    }

    /// Write a JSON-RPC response to stdout
    pub async fn write_response(response: &JsonRpcResponse) -> std::io::Result<()> {
        let mut stdout = tokio::io::stdout();
        let json = serde_json::to_string(response)?;
        stdout.write_all(json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        Ok(())
    }
}
