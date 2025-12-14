//! Relation (edge) types and operations

use crate::entity::EntityId;
use crate::project::ProjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

/// Unique identifier for a relation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationId(pub Ulid);

impl RelationId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for RelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Direction for graph traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}

/// A relation (edge) between two entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    /// Unique identifier
    pub id: RelationId,

    /// Project this relation belongs to
    pub project_id: ProjectId,

    /// Source entity ID
    pub from_id: EntityId,

    /// Source entity name (for convenience)
    pub from_name: String,

    /// Target entity ID
    pub to_id: EntityId,

    /// Target entity name (for convenience)
    pub to_name: String,

    /// Type of relationship (e.g., "works_at", "mentors")
    pub relation_type: String,

    /// Optional weight/strength of relationship
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,

    /// Arbitrary metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl Relation {
    /// Create a new relation with entity IDs
    pub fn new(
        project_id: ProjectId,
        from_id: EntityId,
        from_name: impl Into<String>,
        to_id: EntityId,
        to_name: impl Into<String>,
        relation_type: impl Into<String>,
    ) -> Self {
        Self {
            id: RelationId::new(),
            project_id,
            from_id,
            from_name: from_name.into(),
            to_id,
            to_name: to_name.into(),
            relation_type: relation_type.into(),
            weight: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Create a new relation from just names (generates new EntityIds)
    /// Use this for simple CLI usage where you don't have the entity IDs
    pub fn from_names(
        project_id: ProjectId,
        from_name: impl Into<String>,
        to_name: impl Into<String>,
        relation_type: impl Into<String>,
    ) -> Self {
        Self {
            id: RelationId::new(),
            project_id,
            from_id: EntityId::new(),
            from_name: from_name.into(),
            to_id: EntityId::new(),
            to_name: to_name.into(),
            relation_type: relation_type.into(),
            weight: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Set the weight of this relation
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = Some(weight);
        self
    }
}

/// Data for creating a new relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRelation {
    pub from: String,
    pub to: String,
    pub relation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl NewRelation {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        relation_type: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            relation_type: relation_type.into(),
            weight: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = Some(weight);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_creation() {
        let project_id = ProjectId::new();
        let from_id = EntityId::new();
        let to_id = EntityId::new();

        let relation = Relation::new(
            project_id,
            from_id,
            "John_Smith",
            to_id,
            "Google",
            "works_at",
        );

        assert_eq!(relation.from_name, "John_Smith");
        assert_eq!(relation.to_name, "Google");
        assert_eq!(relation.relation_type, "works_at");
        assert!(relation.weight.is_none());
    }

    #[test]
    fn test_relation_with_weight() {
        let project_id = ProjectId::new();
        let from_id = EntityId::new();
        let to_id = EntityId::new();

        let relation = Relation::new(
            project_id,
            from_id,
            "John_Smith",
            to_id,
            "Jane_Doe",
            "mentors",
        )
        .with_weight(0.8);

        assert_eq!(relation.weight, Some(0.8));
    }
}
