//! Entity (node) types and operations

use crate::observation::Observation;
use crate::project::ProjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

/// Unique identifier for an entity
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub Ulid);

impl EntityId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn from_string(s: &str) -> Result<Self, ulid::DecodeError> {
        Ok(Self(Ulid::from_string(s)?))
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Entity type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityType(pub String);

impl EntityType {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for EntityType {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for EntityType {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&String> for EntityType {
    fn from(s: &String) -> Self {
        Self(s.clone())
    }
}

/// An entity in the knowledge graph (a node)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier
    pub id: EntityId,

    /// Project this entity belongs to
    pub project_id: ProjectId,

    /// Entity name (unique within project)
    pub name: String,

    /// Entity type/category
    pub entity_type: EntityType,

    /// Observations (facts) about this entity
    pub observations: Vec<Observation>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Arbitrary metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Optional vector embedding for semantic search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

impl Entity {
    /// Create a new entity
    pub fn new(
        project_id: ProjectId,
        name: impl Into<String>,
        entity_type: impl Into<EntityType>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: EntityId::new(),
            project_id,
            name: name.into(),
            entity_type: entity_type.into(),
            observations: Vec::new(),
            tags: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            embedding: None,
        }
    }

    /// Add an observation to this entity
    pub fn add_observation(&mut self, content: impl Into<String>) -> &Observation {
        let obs = Observation::new(content);
        self.observations.push(obs);
        self.updated_at = Utc::now();
        // Safe: we just pushed an element, so last() is guaranteed to be Some
        self.observations.last().expect("observations cannot be empty after push")
    }

    /// Add a tag to this entity
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a tag from this entity
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Check if entity has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// Data for creating a new entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEntity {
    pub name: String,
    pub entity_type: String,
    pub observations: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl NewEntity {
    pub fn new(name: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entity_type: entity_type.into(),
            observations: Vec::new(),
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_observation(mut self, obs: impl Into<String>) -> Self {
        self.observations.push(obs.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_creation() {
        let project_id = ProjectId::new();
        let entity = Entity::new(project_id, "John_Smith", "person");

        assert_eq!(entity.name, "John_Smith");
        assert_eq!(entity.entity_type.as_str(), "person");
        assert!(entity.observations.is_empty());
        assert!(entity.tags.is_empty());
    }

    #[test]
    fn test_add_observation() {
        let project_id = ProjectId::new();
        let mut entity = Entity::new(project_id, "John_Smith", "person");

        entity.add_observation("Works at Google");
        assert_eq!(entity.observations.len(), 1);
        assert_eq!(entity.observations[0].content, "Works at Google");
    }

    #[test]
    fn test_tags() {
        let project_id = ProjectId::new();
        let mut entity = Entity::new(project_id, "John_Smith", "person");

        entity.add_tag("technical");
        entity.add_tag("mentor");
        entity.add_tag("technical"); // Duplicate, should not add

        assert_eq!(entity.tags.len(), 2);
        assert!(entity.has_tag("technical"));
        assert!(entity.has_tag("mentor"));

        entity.remove_tag("mentor");
        assert!(!entity.has_tag("mentor"));
    }
}
