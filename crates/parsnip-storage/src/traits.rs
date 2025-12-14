//! Storage backend trait definitions

use crate::error::StorageResult;
use async_trait::async_trait;
use parsnip_core::{Entity, Graph, Project, ProjectId, Relation};

/// Trait for storage backend implementations
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Initialize the storage (create tables, etc.)
    async fn initialize(&self) -> StorageResult<()>;

    /// Close the storage connection
    async fn close(&self) -> StorageResult<()>;

    /// Health check
    async fn health_check(&self) -> StorageResult<bool>;

    // ─────────────────────────────────────────────────────────────────────────
    // Entity Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Save an entity
    async fn save_entity(&self, entity: &Entity) -> StorageResult<()>;

    /// Get an entity by name and project
    async fn get_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<Option<Entity>>;

    /// Get all entities for a project
    async fn get_all_entities(&self, project_id: &ProjectId) -> StorageResult<Vec<Entity>>;

    /// Get all entities across all projects
    async fn get_all_entities_all_projects(&self) -> StorageResult<Vec<Entity>>;

    /// Delete an entity
    async fn delete_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<()>;

    // ─────────────────────────────────────────────────────────────────────────
    // Relation Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Save a relation
    async fn save_relation(&self, relation: &Relation) -> StorageResult<()>;

    /// Get relations for an entity
    async fn get_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Vec<Relation>>;

    /// Get all relations for a project
    async fn get_all_relations(&self, project_id: &ProjectId) -> StorageResult<Vec<Relation>>;

    /// Delete a relation
    async fn delete_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()>;

    /// Delete all relations involving an entity
    async fn delete_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()>;

    // ─────────────────────────────────────────────────────────────────────────
    // Project Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Save a project
    async fn save_project(&self, project: &Project) -> StorageResult<()>;

    /// Get a project by name
    async fn get_project(&self, name: &str) -> StorageResult<Option<Project>>;

    /// Get a project by ID
    async fn get_project_by_id(&self, id: &ProjectId) -> StorageResult<Option<Project>>;

    /// Get all projects
    async fn get_all_projects(&self) -> StorageResult<Vec<Project>>;

    /// Delete a project and all its data
    async fn delete_project(&self, name: &str) -> StorageResult<()>;

    // ─────────────────────────────────────────────────────────────────────────
    // Bulk Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Load entire graph for a project
    async fn load_graph(&self, project_id: &ProjectId) -> StorageResult<Graph> {
        let entities = self.get_all_entities(project_id).await?;
        let relations = self.get_all_relations(project_id).await?;
        Ok(Graph { entities, relations })
    }

    /// Save entire graph for a project (replaces existing)
    async fn save_graph(&self, graph: &Graph, project_id: &ProjectId) -> StorageResult<()>;
}
