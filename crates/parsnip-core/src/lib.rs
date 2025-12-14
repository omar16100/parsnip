//! Parsnip Core - Graph engine for memory management
//!
//! This crate provides the core data types and traits for the Parsnip
//! knowledge graph system.

pub mod entity;
pub mod error;
pub mod graph;
pub mod observation;
pub mod project;
pub mod query;
pub mod relation;
pub mod traversal;

pub use entity::{Entity, EntityId, EntityType, NewEntity};
pub use error::{Error, Result};
pub use graph::{Graph, KnowledgeGraph};
pub use observation::{Observation, ObservationId};
pub use project::{Project, ProjectId};
pub use query::{Pagination, ProjectScope, SearchMode, SearchQuery, TagMatchMode};
pub use relation::{Direction, NewRelation, Relation, RelationId};
pub use traversal::{GraphPath, PathEdge, TraversalEngine, TraversalQuery, TraversalResult, TraversalStats};
