//! Knowledge graph trait definition

use crate::entity::{Entity, NewEntity};
use crate::error::Result;
use crate::project::{Project, ProjectId};
use crate::query::{PaginatedResults, SearchQuery};
use crate::relation::{Direction, NewRelation, Relation};
use async_trait::async_trait;

/// Graph containing entities and their relations
#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entities(mut self, entities: Vec<Entity>) -> Self {
        self.entities = entities;
        self
    }

    pub fn with_relations(mut self, relations: Vec<Relation>) -> Self {
        self.relations = relations;
        self
    }
}

/// Main trait for knowledge graph operations
///
/// All storage backends implement this trait.
#[async_trait]
pub trait KnowledgeGraph: Send + Sync {
    // ─────────────────────────────────────────────────────────────────────────
    // Entity Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a new entity
    async fn create_entity(&self, entity: NewEntity, project: &ProjectId) -> Result<Entity>;

    /// Get an entity by name
    async fn get_entity(&self, name: &str, project: &ProjectId) -> Result<Option<Entity>>;

    /// Get multiple entities by name
    async fn get_entities(&self, names: &[String], project: &ProjectId) -> Result<Vec<Entity>>;

    /// Update an entity
    async fn update_entity(&self, entity: &Entity) -> Result<Entity>;

    /// Delete an entity and its relations
    async fn delete_entity(&self, name: &str, project: &ProjectId) -> Result<()>;

    /// Add observations to an entity
    async fn add_observations(
        &self,
        name: &str,
        observations: Vec<String>,
        project: &ProjectId,
    ) -> Result<Entity>;

    /// Remove specific observations from an entity
    async fn remove_observations(
        &self,
        name: &str,
        observation_ids: &[String],
        project: &ProjectId,
    ) -> Result<Entity>;

    /// Add tags to an entity
    async fn add_tags(&self, name: &str, tags: Vec<String>, project: &ProjectId) -> Result<Entity>;

    /// Remove tags from an entity
    async fn remove_tags(&self, name: &str, tags: &[String], project: &ProjectId)
        -> Result<Entity>;

    // ─────────────────────────────────────────────────────────────────────────
    // Relation Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Create a new relation
    async fn create_relation(&self, relation: NewRelation, project: &ProjectId)
        -> Result<Relation>;

    /// Get relations for an entity
    async fn get_relations(
        &self,
        entity_name: &str,
        direction: Direction,
        project: &ProjectId,
    ) -> Result<Vec<Relation>>;

    /// Delete a relation
    async fn delete_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        project: &ProjectId,
    ) -> Result<()>;

    // ─────────────────────────────────────────────────────────────────────────
    // Graph Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Read the entire graph for a project
    async fn read_graph(&self, project: &ProjectId) -> Result<Graph>;

    /// Traverse the graph from a starting entity
    async fn traverse(
        &self,
        start: &str,
        depth: u32,
        direction: Direction,
        project: &ProjectId,
    ) -> Result<Graph>;

    // ─────────────────────────────────────────────────────────────────────────
    // Search Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Search entities
    async fn search(&self, query: SearchQuery) -> Result<PaginatedResults<Entity>>;

    // ─────────────────────────────────────────────────────────────────────────
    // Project Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// List all projects
    async fn list_projects(&self) -> Result<Vec<Project>>;

    /// Create a new project
    async fn create_project(&self, name: &str, description: Option<&str>) -> Result<Project>;

    /// Get a project by name
    async fn get_project(&self, name: &str) -> Result<Option<Project>>;

    /// Get a project by ID
    async fn get_project_by_id(&self, id: &ProjectId) -> Result<Option<Project>>;

    /// Delete a project and all its data
    async fn delete_project(&self, name: &str) -> Result<()>;

    /// Get or create the default project
    async fn get_or_create_default_project(&self) -> Result<Project>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_builder() {
        let graph = Graph::new().with_entities(vec![]).with_relations(vec![]);

        assert!(graph.entities.is_empty());
        assert!(graph.relations.is_empty());
    }
}
