//! Search error types

use thiserror::Error;

/// Result type alias for search operations
pub type SearchResult<T> = std::result::Result<T, SearchError>;

/// Search-specific error types
#[derive(Error, Debug)]
pub enum SearchError {
    #[error("Index error: {0}")]
    Index(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
