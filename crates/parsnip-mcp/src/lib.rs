//! Parsnip MCP - Model Context Protocol server
//!
//! Provides MCP server implementation for AI assistant integration.

pub mod handlers;
pub mod server;
pub mod tools;
pub mod transport;

#[cfg(feature = "sse")]
pub mod sse;

/// JSON-RPC method prefix for the storage RPC surface used by remote CLI clients.
/// Distinct from the MCP `tools/*` surface, which is for LLM clients.
pub const STORAGE_METHOD_PREFIX: &str = "storage/";

/// Capabilities advertised on `/health` so clients can detect version skew before
/// issuing calls the daemon does not implement.
pub const CAPABILITIES: &[&str] = &["storage/v1", "search/v1"];

pub use server::McpServer;

#[cfg(feature = "sse")]
pub use sse::run_sse_server;
