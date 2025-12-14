//! ReDB storage backend

use crate::error::{StorageError, StorageResult};
use crate::traits::StorageBackend;
use async_trait::async_trait;
use parsnip_core::{Entity, Graph, Project, ProjectId, Relation};
use redb::{Database, ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Mutex;

// Table definitions
const ENTITIES: TableDefinition<&str, &[u8]> = TableDefinition::new("entities");
const RELATIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("relations");
const PROJECTS: TableDefinition<&str, &[u8]> = TableDefinition::new("projects");

/// ReDB storage backend
pub struct RedbStorage {
    db: Mutex<Database>,
}

impl RedbStorage {
    /// Open or create a ReDB database at the given path
    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let db = Database::create(path).map_err(|e| StorageError::Database(e.to_string()))?;

        // Initialize tables
        {
            let write_txn = db
                .begin_write()
                .map_err(|e| StorageError::Database(e.to_string()))?;
            {
                let _ = write_txn.open_table(ENTITIES);
                let _ = write_txn.open_table(RELATIONS);
                let _ = write_txn.open_table(PROJECTS);
            }
            write_txn
                .commit()
                .map_err(|e| StorageError::Database(e.to_string()))?;
        }

        Ok(Self { db: Mutex::new(db) })
    }

    fn make_entity_key(project_id: &ProjectId, name: &str) -> String {
        format!("{}:{}", project_id, name)
    }

    fn make_relation_key(project_id: &ProjectId, from: &str, to: &str, rel_type: &str) -> String {
        format!("{}:{}:{}:{}", project_id, from, to, rel_type)
    }
}

#[async_trait]
impl StorageBackend for RedbStorage {
    async fn initialize(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn close(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn health_check(&self) -> StorageResult<bool> {
        Ok(true)
    }

    async fn save_entity(&self, entity: &Entity) -> StorageResult<()> {
        let key = Self::make_entity_key(&entity.project_id, &entity.name);
        let value = serde_json::to_vec(entity)?;

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(ENTITIES)?;
            table.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn get_entity(
        &self,
        name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Option<Entity>> {
        let key = Self::make_entity_key(project_id, name);

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(ENTITIES)?;

        if let Some(value) = table.get(key.as_str())? {
            let entity: Entity = serde_json::from_slice(value.value())?;
            Ok(Some(entity))
        } else {
            Ok(None)
        }
    }

    async fn get_all_entities(&self, project_id: &ProjectId) -> StorageResult<Vec<Entity>> {
        let prefix = format!("{}:", project_id);

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(ENTITIES)?;

        let mut entities = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            if key.value().starts_with(&prefix) {
                let entity: Entity = serde_json::from_slice(value.value())?;
                entities.push(entity);
            }
        }

        Ok(entities)
    }

    async fn get_all_entities_all_projects(&self) -> StorageResult<Vec<Entity>> {
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(ENTITIES)?;

        let mut entities = Vec::new();
        for entry in table.iter()? {
            let (_, value) = entry?;
            let entity: Entity = serde_json::from_slice(value.value())?;
            entities.push(entity);
        }

        Ok(entities)
    }

    async fn delete_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<()> {
        let key = Self::make_entity_key(project_id, name);

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(ENTITIES)?;
            table.remove(key.as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn save_relation(&self, relation: &Relation) -> StorageResult<()> {
        let key = Self::make_relation_key(
            &relation.project_id,
            &relation.from_name,
            &relation.to_name,
            &relation.relation_type,
        );
        let value = serde_json::to_vec(relation)?;

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(RELATIONS)?;
            table.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn get_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Vec<Relation>> {
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(RELATIONS)?;

        let prefix = format!("{}:", project_id);
        let mut relations = Vec::new();

        for entry in table.iter()? {
            let (key, value) = entry?;
            if key.value().starts_with(&prefix) {
                let relation: Relation = serde_json::from_slice(value.value())?;
                if relation.from_name == entity_name || relation.to_name == entity_name {
                    relations.push(relation);
                }
            }
        }

        Ok(relations)
    }

    async fn get_all_relations(&self, project_id: &ProjectId) -> StorageResult<Vec<Relation>> {
        let prefix = format!("{}:", project_id);

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(RELATIONS)?;

        let mut relations = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            if key.value().starts_with(&prefix) {
                let relation: Relation = serde_json::from_slice(value.value())?;
                relations.push(relation);
            }
        }

        Ok(relations)
    }

    async fn get_all_relations_all_projects(&self) -> StorageResult<Vec<Relation>> {
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(RELATIONS)?;

        let mut relations = Vec::new();
        for entry in table.iter()? {
            let (_, value) = entry?;
            let relation: Relation = serde_json::from_slice(value.value())?;
            relations.push(relation);
        }

        Ok(relations)
    }

    async fn get_relations_for_entity_global(
        &self,
        entity_name: &str,
    ) -> StorageResult<Vec<Relation>> {
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(RELATIONS)?;

        let mut relations = Vec::new();
        for entry in table.iter()? {
            let (_, value) = entry?;
            let relation: Relation = serde_json::from_slice(value.value())?;
            if relation.from_name == entity_name || relation.to_name == entity_name {
                relations.push(relation);
            }
        }

        Ok(relations)
    }

    async fn delete_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        let key = Self::make_relation_key(project_id, from, to, relation_type);

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(RELATIONS)?;
            table.remove(key.as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn delete_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        let prefix = format!("{}:", project_id);

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let keys_to_delete: Vec<String> = {
            let table = write_txn.open_table(RELATIONS)?;
            let mut keys = Vec::new();
            for entry in table.iter()? {
                let (key, value) = entry?;
                if key.value().starts_with(&prefix) {
                    let relation: Relation = serde_json::from_slice(value.value())?;
                    if relation.from_name == entity_name || relation.to_name == entity_name {
                        keys.push(key.value().to_string());
                    }
                }
            }
            keys
        };

        {
            let mut table = write_txn.open_table(RELATIONS)?;
            for key in keys_to_delete {
                table.remove(key.as_str())?;
            }
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn save_project(&self, project: &Project) -> StorageResult<()> {
        let value = serde_json::to_vec(project)?;

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(PROJECTS)?;
            table.insert(project.name.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn get_project(&self, name: &str) -> StorageResult<Option<Project>> {
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(PROJECTS)?;

        if let Some(value) = table.get(name)? {
            let project: Project = serde_json::from_slice(value.value())?;
            Ok(Some(project))
        } else {
            Ok(None)
        }
    }

    async fn get_project_by_id(&self, id: &ProjectId) -> StorageResult<Option<Project>> {
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(PROJECTS)?;

        for entry in table.iter()? {
            let (_, value) = entry?;
            let project: Project = serde_json::from_slice(value.value())?;
            if &project.id == id {
                return Ok(Some(project));
            }
        }

        Ok(None)
    }

    async fn get_all_projects(&self) -> StorageResult<Vec<Project>> {
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let read_txn = db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn.open_table(PROJECTS)?;

        let mut projects = Vec::new();
        for entry in table.iter()? {
            let (_, value) = entry?;
            let project: Project = serde_json::from_slice(value.value())?;
            projects.push(project);
        }

        Ok(projects)
    }

    async fn delete_project(&self, name: &str) -> StorageResult<()> {
        // First get the project to find its ID
        let project = match self.get_project(name).await? {
            Some(p) => p,
            None => return Ok(()),
        };

        // Delete all entities and relations for this project
        let entities = self.get_all_entities(&project.id).await?;
        for entity in &entities {
            self.delete_entity(&entity.name, &project.id).await?;
        }

        let relations = self.get_all_relations(&project.id).await?;
        for relation in &relations {
            self.delete_relation(
                &relation.from_name,
                &relation.to_name,
                &relation.relation_type,
                &project.id,
            )
            .await?;
        }

        // Delete the project itself
        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(PROJECTS)?;
            table.remove(name)?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn save_graph(&self, graph: &Graph, _project_id: &ProjectId) -> StorageResult<()> {
        // Use batch methods for transactional efficiency
        self.save_entities_batch(&graph.entities).await?;
        self.save_relations_batch(&graph.relations).await?;
        Ok(())
    }

    async fn save_entities_batch(&self, entities: &[Entity]) -> StorageResult<()> {
        if entities.is_empty() {
            return Ok(());
        }

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(ENTITIES)?;
            for entity in entities {
                let key = Self::make_entity_key(&entity.project_id, &entity.name);
                let value = serde_json::to_vec(entity)?;
                table.insert(key.as_str(), value.as_slice())?;
            }
        }
        write_txn.commit()?;
        tracing::debug!(
            "Batch saved {} entities in single transaction",
            entities.len()
        );

        Ok(())
    }

    async fn save_relations_batch(&self, relations: &[Relation]) -> StorageResult<()> {
        if relations.is_empty() {
            return Ok(());
        }

        let db = self
            .db
            .lock()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let mut table = write_txn.open_table(RELATIONS)?;
            for relation in relations {
                let key = Self::make_relation_key(
                    &relation.project_id,
                    &relation.from_name,
                    &relation.to_name,
                    &relation.relation_type,
                );
                let value = serde_json::to_vec(relation)?;
                table.insert(key.as_str(), value.as_slice())?;
            }
        }
        write_txn.commit()?;
        tracing::debug!(
            "Batch saved {} relations in single transaction",
            relations.len()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_redb_storage() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.redb");

        let storage = RedbStorage::open(&db_path).unwrap();
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
