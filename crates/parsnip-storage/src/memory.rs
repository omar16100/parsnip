//! In-memory storage backend for testing

use crate::error::{StorageError, StorageResult};
use crate::traits::StorageBackend;
use async_trait::async_trait;
use parsnip_core::{Entity, Graph, Project, ProjectId, Relation};
use std::collections::HashMap;
use std::sync::RwLock;

/// In-memory storage backend
///
/// Useful for testing and temporary storage.
pub struct MemoryStorage {
    entities: RwLock<HashMap<(ProjectId, String), Entity>>,
    relations: RwLock<Vec<Relation>>,
    projects: RwLock<HashMap<String, Project>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            entities: RwLock::new(HashMap::new()),
            relations: RwLock::new(Vec::new()),
            projects: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for MemoryStorage {
    async fn initialize(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn close(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn health_check(&self) -> StorageResult<bool> {
        Ok(true)
    }

    // Entity operations

    async fn save_entity(&self, entity: &Entity) -> StorageResult<()> {
        let mut entities = self
            .entities
            .write()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        entities.insert(
            (entity.project_id.clone(), entity.name.clone()),
            entity.clone(),
        );
        Ok(())
    }

    async fn get_entity(
        &self,
        name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Option<Entity>> {
        let entities = self
            .entities
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(entities
            .get(&(project_id.clone(), name.to_string()))
            .cloned())
    }

    async fn get_all_entities(&self, project_id: &ProjectId) -> StorageResult<Vec<Entity>> {
        let entities = self
            .entities
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(entities
            .iter()
            .filter(|((pid, _), _)| pid == project_id)
            .map(|(_, e)| e.clone())
            .collect())
    }

    async fn get_all_entities_all_projects(&self) -> StorageResult<Vec<Entity>> {
        let entities = self
            .entities
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(entities.values().cloned().collect())
    }

    async fn delete_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<()> {
        let mut entities = self
            .entities
            .write()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        entities.remove(&(project_id.clone(), name.to_string()));
        Ok(())
    }

    // Relation operations

    async fn save_relation(&self, relation: &Relation) -> StorageResult<()> {
        let mut relations = self
            .relations
            .write()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;

        // Check for duplicate
        let exists = relations.iter().any(|r| {
            r.project_id == relation.project_id
                && r.from_name == relation.from_name
                && r.to_name == relation.to_name
                && r.relation_type == relation.relation_type
        });

        if !exists {
            relations.push(relation.clone());
        }
        Ok(())
    }

    async fn get_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Vec<Relation>> {
        let relations = self
            .relations
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(relations
            .iter()
            .filter(|r| {
                r.project_id == *project_id
                    && (r.from_name == entity_name || r.to_name == entity_name)
            })
            .cloned()
            .collect())
    }

    async fn get_all_relations(&self, project_id: &ProjectId) -> StorageResult<Vec<Relation>> {
        let relations = self
            .relations
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(relations
            .iter()
            .filter(|r| r.project_id == *project_id)
            .cloned()
            .collect())
    }

    async fn get_all_relations_all_projects(&self) -> StorageResult<Vec<Relation>> {
        let relations = self
            .relations
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(relations.iter().cloned().collect())
    }

    async fn get_relations_for_entity_global(
        &self,
        entity_name: &str,
    ) -> StorageResult<Vec<Relation>> {
        let relations = self
            .relations
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(relations
            .iter()
            .filter(|r| r.from_name == entity_name || r.to_name == entity_name)
            .cloned()
            .collect())
    }

    async fn delete_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        let mut relations = self
            .relations
            .write()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        relations.retain(|r| {
            !(r.project_id == *project_id
                && r.from_name == from
                && r.to_name == to
                && r.relation_type == relation_type)
        });
        Ok(())
    }

    async fn delete_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        let mut relations = self
            .relations
            .write()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        relations.retain(|r| {
            !(r.project_id == *project_id
                && (r.from_name == entity_name || r.to_name == entity_name))
        });
        Ok(())
    }

    // Project operations

    async fn save_project(&self, project: &Project) -> StorageResult<()> {
        let mut projects = self
            .projects
            .write()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        projects.insert(project.name.clone(), project.clone());
        Ok(())
    }

    async fn get_project(&self, name: &str) -> StorageResult<Option<Project>> {
        let projects = self
            .projects
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(projects.get(name).cloned())
    }

    async fn get_project_by_id(&self, id: &ProjectId) -> StorageResult<Option<Project>> {
        let projects = self
            .projects
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(projects.values().find(|p| p.id == *id).cloned())
    }

    async fn get_all_projects(&self) -> StorageResult<Vec<Project>> {
        let projects = self
            .projects
            .read()
            .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
        Ok(projects.values().cloned().collect())
    }

    async fn delete_project(&self, name: &str) -> StorageResult<()> {
        let project = {
            let projects = self
                .projects
                .read()
                .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
            projects.get(name).cloned()
        };

        if let Some(project) = project {
            // Delete all entities
            {
                let mut entities = self
                    .entities
                    .write()
                    .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
                entities.retain(|(pid, _), _| pid != &project.id);
            }

            // Delete all relations
            {
                let mut relations = self
                    .relations
                    .write()
                    .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
                relations.retain(|r| r.project_id != project.id);
            }

            // Delete project
            {
                let mut projects = self
                    .projects
                    .write()
                    .map_err(|e| StorageError::Database(format!("Lock error: {}", e)))?;
                projects.remove(name);
            }
        }

        Ok(())
    }

    async fn save_graph(&self, graph: &Graph, _project_id: &ProjectId) -> StorageResult<()> {
        for entity in &graph.entities {
            self.save_entity(entity).await?;
        }
        for relation in &graph.relations {
            self.save_relation(relation).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsnip_core::Entity;

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStorage::new();
        storage.initialize().await.unwrap();

        // Create a project
        let project = Project::new("test-project");
        storage.save_project(&project).await.unwrap();

        // Create an entity
        let entity = Entity::new(project.id.clone(), "TestEntity", "test");
        storage.save_entity(&entity).await.unwrap();

        // Retrieve the entity
        let retrieved = storage.get_entity("TestEntity", &project.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "TestEntity");

        // Delete the entity
        storage
            .delete_entity("TestEntity", &project.id)
            .await
            .unwrap();
        let retrieved = storage.get_entity("TestEntity", &project.id).await.unwrap();
        assert!(retrieved.is_none());
    }
}
