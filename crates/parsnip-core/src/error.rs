//! Error types for Parsnip Core

use thiserror::Error;

/// Result type alias using Parsnip's Error
pub type Result<T> = std::result::Result<T, Error>;

/// Parsnip error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Entity already exists: {0}")]
    EntityExists(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Project already exists: {0}")]
    ProjectExists(String),

    #[error("Relation not found: {from} -> {to}")]
    RelationNotFound { from: String, to: String },

    #[error("Invalid entity name: {0}")]
    InvalidEntityName(String),

    #[error("Invalid project name: {0}")]
    InvalidProjectName(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Search error: {0}")]
    Search(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
