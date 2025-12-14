//! Parsnip Storage - Storage backends for the knowledge graph
//!
//! This crate provides different storage backends for persisting
//! the knowledge graph data.

#![allow(clippy::result_large_err)]

pub mod error;
pub mod migration;
pub mod traits;

#[cfg(feature = "redb")]
pub mod redb;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub mod memory;

pub use error::{StorageError, StorageResult};
pub use migration::{Migratable, SchemaVersion, CURRENT_VERSION};
pub use traits::StorageBackend;

#[cfg(feature = "redb")]
pub use redb::RedbStorage;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStorage;

pub use memory::MemoryStorage;
