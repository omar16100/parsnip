//! Parsnip MCP - Model Context Protocol server
//!
//! Provides MCP server implementation for AI assistant integration.

pub mod handlers;
pub mod server;
pub mod tools;
pub mod transport;

#[cfg(feature = "sse")]
pub mod sse;

pub use server::McpServer;

#[cfg(feature = "sse")]
pub use sse::run_sse_server;
