//! SQLite storage backend

use crate::error::{StorageError, StorageResult};
use crate::traits::StorageBackend;
use async_trait::async_trait;
use parsnip_core::{Entity, Graph, Project, ProjectId, Relation};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

/// SQLite storage backend
pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    /// Open or create a SQLite database at the given path
    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let conn = Connection::open(path).map_err(|e| StorageError::Database(e.to_string()))?;

        let storage = Self { conn: Mutex::new(conn) };
        storage.init_tables()?;

        Ok(storage)
    }

    /// Create an in-memory SQLite database (for testing)
    pub fn in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory().map_err(|e| StorageError::Database(e.to_string()))?;

        let storage = Self { conn: Mutex::new(conn) };
        storage.init_tables()?;

        Ok(storage)
    }

    fn init_tables(&self) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                name TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS entities (
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                data TEXT NOT NULL,
                PRIMARY KEY (project_id, name)
            );

            CREATE TABLE IF NOT EXISTS relations (
                project_id TEXT NOT NULL,
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                data TEXT NOT NULL,
                PRIMARY KEY (project_id, from_name, to_name, relation_type)
            );

            CREATE INDEX IF NOT EXISTS idx_entities_project ON entities(project_id);
            CREATE INDEX IF NOT EXISTS idx_relations_project ON relations(project_id);
            CREATE INDEX IF NOT EXISTS idx_relations_from ON relations(project_id, from_name);
            CREATE INDEX IF NOT EXISTS idx_relations_to ON relations(project_id, to_name);
            "#
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl StorageBackend for SqliteStorage {
    async fn initialize(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn close(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn health_check(&self) -> StorageResult<bool> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;
        conn.execute("SELECT 1", []).map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(true)
    }

    async fn save_entity(&self, entity: &Entity) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;
        let data = serde_json::to_string(entity)?;

        conn.execute(
            "INSERT OR REPLACE INTO entities (project_id, name, data) VALUES (?1, ?2, ?3)",
            params![entity.project_id.to_string(), entity.name, data]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<Option<Entity>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT data FROM entities WHERE project_id = ?1 AND name = ?2"
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        let result = stmt.query_row(params![project_id.to_string(), name], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        });

        match result {
            Ok(data) => {
                let entity: Entity = serde_json::from_str(&data)?;
                Ok(Some(entity))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e.to_string())),
        }
    }

    async fn get_all_entities(&self, project_id: &ProjectId) -> StorageResult<Vec<Entity>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT data FROM entities WHERE project_id = ?1"
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt.query_map(params![project_id.to_string()], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        }).map_err(|e| StorageError::Database(e.to_string()))?;

        let mut entities = Vec::new();
        for row in rows {
            let data = row.map_err(|e| StorageError::Database(e.to_string()))?;
            let entity: Entity = serde_json::from_str(&data)?;
            entities.push(entity);
        }

        Ok(entities)
    }

    async fn get_all_entities_all_projects(&self) -> StorageResult<Vec<Entity>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare("SELECT data FROM entities")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        }).map_err(|e| StorageError::Database(e.to_string()))?;

        let mut entities = Vec::new();
        for row in rows {
            let data = row.map_err(|e| StorageError::Database(e.to_string()))?;
            let entity: Entity = serde_json::from_str(&data)?;
            entities.push(entity);
        }

        Ok(entities)
    }

    async fn delete_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        conn.execute(
            "DELETE FROM entities WHERE project_id = ?1 AND name = ?2",
            params![project_id.to_string(), name]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    async fn save_relation(&self, relation: &Relation) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;
        let data = serde_json::to_string(relation)?;

        conn.execute(
            "INSERT OR REPLACE INTO relations (project_id, from_name, to_name, relation_type, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                relation.project_id.to_string(),
                relation.from_name,
                relation.to_name,
                relation.relation_type,
                data
            ]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Vec<Relation>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT data FROM relations WHERE project_id = ?1 AND (from_name = ?2 OR to_name = ?2)"
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt.query_map(params![project_id.to_string(), entity_name], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        }).map_err(|e| StorageError::Database(e.to_string()))?;

        let mut relations = Vec::new();
        for row in rows {
            let data = row.map_err(|e| StorageError::Database(e.to_string()))?;
            let relation: Relation = serde_json::from_str(&data)?;
            relations.push(relation);
        }

        Ok(relations)
    }

    async fn get_all_relations(&self, project_id: &ProjectId) -> StorageResult<Vec<Relation>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT data FROM relations WHERE project_id = ?1"
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt.query_map(params![project_id.to_string()], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        }).map_err(|e| StorageError::Database(e.to_string()))?;

        let mut relations = Vec::new();
        for row in rows {
            let data = row.map_err(|e| StorageError::Database(e.to_string()))?;
            let relation: Relation = serde_json::from_str(&data)?;
            relations.push(relation);
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
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        conn.execute(
            "DELETE FROM relations WHERE project_id = ?1 AND from_name = ?2 AND to_name = ?3 AND relation_type = ?4",
            params![project_id.to_string(), from, to, relation_type]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        conn.execute(
            "DELETE FROM relations WHERE project_id = ?1 AND (from_name = ?2 OR to_name = ?2)",
            params![project_id.to_string(), entity_name]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    async fn save_project(&self, project: &Project) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;
        let data = serde_json::to_string(project)?;

        conn.execute(
            "INSERT OR REPLACE INTO projects (name, data) VALUES (?1, ?2)",
            params![project.name, data]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_project(&self, name: &str) -> StorageResult<Option<Project>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare("SELECT data FROM projects WHERE name = ?1")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let result = stmt.query_row(params![name], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        });

        match result {
            Ok(data) => {
                let project: Project = serde_json::from_str(&data)?;
                Ok(Some(project))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e.to_string())),
        }
    }

    async fn get_project_by_id(&self, id: &ProjectId) -> StorageResult<Option<Project>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare("SELECT data FROM projects")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        }).map_err(|e| StorageError::Database(e.to_string()))?;

        for row in rows {
            let data = row.map_err(|e| StorageError::Database(e.to_string()))?;
            let project: Project = serde_json::from_str(&data)?;
            if &project.id == id {
                return Ok(Some(project));
            }
        }

        Ok(None)
    }

    async fn get_all_projects(&self) -> StorageResult<Vec<Project>> {
        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        let mut stmt = conn.prepare("SELECT data FROM projects")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        }).map_err(|e| StorageError::Database(e.to_string()))?;

        let mut projects = Vec::new();
        for row in rows {
            let data = row.map_err(|e| StorageError::Database(e.to_string()))?;
            let project: Project = serde_json::from_str(&data)?;
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

        let conn = self.conn.lock().map_err(|e| StorageError::Database(e.to_string()))?;

        // Delete all entities and relations for this project
        conn.execute(
            "DELETE FROM entities WHERE project_id = ?1",
            params![project.id.to_string()]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        conn.execute(
            "DELETE FROM relations WHERE project_id = ?1",
            params![project.id.to_string()]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        // Delete the project itself
        conn.execute(
            "DELETE FROM projects WHERE name = ?1",
            params![name]
        ).map_err(|e| StorageError::Database(e.to_string()))?;

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

    #[tokio::test]
    async fn test_sqlite_storage() {
        let storage = SqliteStorage::in_memory().unwrap();
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
        storage.delete_entity("TestEntity", &project.id).await.unwrap();
        let retrieved = storage.get_entity("TestEntity", &project.id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_relations() {
        let storage = SqliteStorage::in_memory().unwrap();

        let project = Project::new("test");
        storage.save_project(&project).await.unwrap();

        let relation = Relation::from_names(
            project.id.clone(),
            "John",
            "Google",
            "works_at",
        );
        storage.save_relation(&relation).await.unwrap();

        let relations = storage.get_relations_for_entity("John", &project.id).await.unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].relation_type, "works_at");
    }
}
