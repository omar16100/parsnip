//! Parsnip MCP - Model Context Protocol server
//!
//! Provides MCP server implementation for AI assistant integration.

pub mod handlers;
pub mod server;
pub mod tools;
pub mod transport;

pub use server::McpServer;
