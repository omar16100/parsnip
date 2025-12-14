//! Storage error types

use thiserror::Error;

/// Result type alias for storage operations
pub type StorageResult<T> = std::result::Result<T, StorageError>;

/// Storage-specific error types
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Duplicate entity: {0}")]
    DuplicateEntity(String),

    #[error("Duplicate project: {0}")]
    DuplicateProject(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[cfg(feature = "redb")]
    #[error("ReDB error: {0}")]
    Redb(#[from] ::redb::Error),

    #[cfg(feature = "redb")]
    #[error("ReDB database error: {0}")]
    RedbDatabase(#[from] ::redb::DatabaseError),

    #[cfg(feature = "redb")]
    #[error("ReDB table error: {0}")]
    RedbTable(#[from] ::redb::TableError),

    #[cfg(feature = "redb")]
    #[error("ReDB storage error: {0}")]
    RedbStorage(#[from] ::redb::StorageError),

    #[cfg(feature = "redb")]
    #[error("ReDB commit error: {0}")]
    RedbCommit(#[from] ::redb::CommitError),

    #[cfg(feature = "redb")]
    #[error("ReDB transaction error: {0}")]
    RedbTransaction(#[from] ::redb::TransactionError),

    #[cfg(feature = "sqlite")]
    #[error("SQLite error: {0}")]
    Sqlite(#[from] ::rusqlite::Error),
}
