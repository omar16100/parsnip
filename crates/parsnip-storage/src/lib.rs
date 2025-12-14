//! Parsnip Storage - Storage backends for the knowledge graph
//!
//! This crate provides different storage backends for persisting
//! the knowledge graph data.

pub mod error;
pub mod traits;

#[cfg(feature = "redb")]
pub mod redb;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub mod memory;

pub use error::{StorageError, StorageResult};
pub use traits::StorageBackend;

#[cfg(feature = "redb")]
pub use redb::RedbStorage;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStorage;

pub use memory::MemoryStorage;
