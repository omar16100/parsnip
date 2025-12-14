//! Project (namespace) types and operations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Unique identifier for a project
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub Ulid);

impl ProjectId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn from_string(s: &str) -> Result<Self, ulid::DecodeError> {
        Ok(Self(Ulid::from_string(s)?))
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A project (namespace) in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Unique identifier
    pub id: ProjectId,

    /// Project name (unique, alphanumeric with underscores/hyphens)
    pub name: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Project settings
    #[serde(default)]
    pub settings: ProjectSettings,
}

impl Project {
    /// Create a new project
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: ProjectId::new(),
            name: name.into(),
            description: None,
            created_at: Utc::now(),
            settings: ProjectSettings::default(),
        }
    }

    /// Create project with description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Validate project name (alphanumeric, underscores, hyphens only)
    pub fn validate_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 100
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }
}

/// Project-specific settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSettings {
    /// Whether to enable full-text search indexing
    #[serde(default = "default_true")]
    pub fulltext_enabled: bool,

    /// Default fuzzy search threshold
    #[serde(default = "default_fuzzy_threshold")]
    pub fuzzy_threshold: f32,
}

fn default_true() -> bool {
    true
}

fn default_fuzzy_threshold() -> f32 {
    0.3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation() {
        let project = Project::new("my-project");
        assert_eq!(project.name, "my-project");
        assert!(project.description.is_none());
    }

    #[test]
    fn test_project_with_description() {
        let project = Project::new("security-research")
            .with_description("Security vulnerability findings");
        assert_eq!(project.description, Some("Security vulnerability findings".to_string()));
    }

    #[test]
    fn test_validate_project_name() {
        assert!(Project::validate_name("my-project"));
        assert!(Project::validate_name("my_project_123"));
        assert!(Project::validate_name("MyProject"));
        assert!(!Project::validate_name(""));
        assert!(!Project::validate_name("my project")); // space
        assert!(!Project::validate_name("my.project")); // dot
    }
}
