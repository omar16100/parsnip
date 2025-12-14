//! Search engine traits

use async_trait::async_trait;
use parsnip_core::{Entity, ProjectId, SearchQuery};

pub use crate::error::{SearchError, SearchResult as Result};

/// Result from search including score
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub entity: Entity,
    pub score: f32,
}

/// Trait for search engines
#[async_trait]
pub trait SearchEngine: Send + Sync {
    /// Search entities based on query
    async fn search(&self, query: &SearchQuery, entities: &[Entity]) -> Result<Vec<Entity>>;

    /// Index an entity (optional for stateless engines)
    async fn index_entity(&self, _entity: &Entity, _project_id: &ProjectId) -> Result<()> {
        Ok(())
    }

    /// Remove an entity from index (optional for stateless engines)
    async fn remove_entity(&self, _entity_name: &str, _project_id: &ProjectId) -> Result<()> {
        Ok(())
    }

    /// Rebuild entire index (optional for stateless engines)
    async fn rebuild_index(&self, _entities: &[Entity]) -> Result<()> {
        Ok(())
    }
}
