//! Parsnip Core - Graph engine for memory management
//!
//! This crate provides the core data types and traits for the Parsnip
//! knowledge graph system.

pub mod entity;
pub mod error;
pub mod graph;
pub mod limits;
pub mod observation;
pub mod project;
pub mod query;
pub mod relation;
pub mod traversal;

pub use entity::{Entity, EntityId, EntityType, NewEntity};
pub use error::{Error, Result};
pub use graph::{Graph, KnowledgeGraph};
pub use limits::{
    validate_batch_entities, validate_batch_relations, validate_entity_name, validate_observation,
    validate_project_name, validate_tag, validate_traversal_depth, ValidationError,
    MAX_BATCH_ENTITIES, MAX_BATCH_RELATIONS, MAX_ENTITY_NAME_LEN, MAX_OBSERVATIONS_PER_ENTITY,
    MAX_OBSERVATION_LEN, MAX_PROJECT_NAME_LEN, MAX_TAGS_PER_ENTITY, MAX_TAG_LEN,
    MAX_TRAVERSAL_DEPTH, MAX_TRAVERSAL_NODES,
};
pub use observation::{Observation, ObservationId};
pub use project::{Project, ProjectId};
pub use query::{Pagination, ProjectScope, SearchMode, SearchQuery, TagMatchMode};
pub use relation::{Direction, NewRelation, Relation, RelationId};
pub use traversal::{
    GraphPath, PathEdge, TraversalEngine, TraversalQuery, TraversalResult, TraversalStats,
};
