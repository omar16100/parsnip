//! Query types for searching the knowledge graph

use crate::project::ProjectId;
use serde::{Deserialize, Serialize};

/// Search mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// Exact substring matching
    #[default]
    Exact,
    /// Fuzzy matching with Levenshtein distance
    Fuzzy,
    /// Full-text search with ranking
    FullText,
    /// Hybrid: combines fuzzy and full-text
    Hybrid,
    /// Vector/semantic search using embeddings
    Vector,
}

/// Tag matching mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagMatchMode {
    /// Match any of the specified tags
    #[default]
    Any,
    /// Match all of the specified tags
    All,
}

/// Project scope for search
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectScope {
    /// Search in a single project
    Single(ProjectId),
    /// Search in multiple specific projects
    Multiple(Vec<ProjectId>),
    /// Search across all projects
    #[default]
    All,
}

/// Pagination options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Page number (0-indexed)
    #[serde(default)]
    pub page: usize,

    /// Number of results per page
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: default_page_size(),
        }
    }
}

fn default_page_size() -> usize {
    100
}

impl Pagination {
    pub fn new(page: usize, page_size: usize) -> Self {
        Self {
            page,
            page_size: page_size.min(1000), // Max 1000 per page
        }
    }

    pub fn offset(&self) -> usize {
        self.page * self.page_size
    }
}

/// Search query builder
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Text to search for
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Search mode
    #[serde(default)]
    pub mode: SearchMode,

    /// Fuzzy search threshold (0.0-1.0, lower = more results)
    #[serde(default = "default_fuzzy_threshold")]
    pub fuzzy_threshold: f32,

    /// Query embedding for vector search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_embedding: Option<Vec<f32>>,

    /// Similarity threshold for vector search (0.0-1.0, higher = stricter)
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

    /// Filter by entity types
    #[serde(default)]
    pub entity_types: Vec<String>,

    /// Filter by tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Tag matching mode
    #[serde(default)]
    pub tag_match_mode: TagMatchMode,

    /// Project scope
    #[serde(default)]
    pub projects: ProjectScope,

    /// Pagination
    #[serde(default)]
    pub pagination: Pagination,

    /// Include relations in results
    #[serde(default = "default_true")]
    pub include_relations: bool,
}

fn default_fuzzy_threshold() -> f32 {
    0.3
}

fn default_similarity_threshold() -> f32 {
    0.7
}

fn default_true() -> bool {
    true
}

impl SearchQuery {
    /// Create a new search query with text
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            ..Default::default()
        }
    }

    /// Alias for new() - create search query with text
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(text)
    }

    /// Create an empty search query (for tag-only search)
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set search mode
    pub fn with_mode(mut self, mode: SearchMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set fuzzy threshold
    pub fn with_fuzzy_threshold(mut self, threshold: f32) -> Self {
        self.fuzzy_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set query embedding for vector search
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.query_embedding = Some(embedding);
        self.mode = SearchMode::Vector;
        self
    }

    /// Set similarity threshold for vector search
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Add entity type filter
    pub fn with_entity_type(mut self, entity_type: impl Into<String>) -> Self {
        self.entity_types.push(entity_type.into());
        self
    }

    /// Add tag filter
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set tag match mode
    pub fn with_tag_match_mode(mut self, mode: TagMatchMode) -> Self {
        self.tag_match_mode = mode;
        self
    }

    /// Search in a specific project
    pub fn in_project(mut self, project_id: ProjectId) -> Self {
        self.projects = ProjectScope::Single(project_id);
        self
    }

    /// Search across all projects
    pub fn in_all_projects(mut self) -> Self {
        self.projects = ProjectScope::All;
        self
    }

    /// Set pagination
    pub fn with_pagination(mut self, page: usize, page_size: usize) -> Self {
        self.pagination = Pagination::new(page, page_size);
        self
    }
}

/// Paginated search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResults<T> {
    /// The data for this page
    pub data: Vec<T>,

    /// Pagination metadata
    pub pagination: PaginationInfo,
}

/// Pagination metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub current_page: usize,
    pub page_size: usize,
    pub total_count: usize,
    pub total_pages: usize,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

impl PaginationInfo {
    pub fn new(current_page: usize, page_size: usize, total_count: usize) -> Self {
        let total_pages = total_count.div_ceil(page_size);
        Self {
            current_page,
            page_size,
            total_count,
            total_pages,
            has_next_page: current_page + 1 < total_pages,
            has_previous_page: current_page > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_builder() {
        let query = SearchQuery::new("rust programming")
            .with_mode(SearchMode::Fuzzy)
            .with_fuzzy_threshold(0.4)
            .with_entity_type("person")
            .with_tag("technical")
            .in_all_projects();

        assert_eq!(query.text, Some("rust programming".to_string()));
        assert_eq!(query.mode, SearchMode::Fuzzy);
        assert_eq!(query.fuzzy_threshold, 0.4);
        assert!(query.entity_types.contains(&"person".to_string()));
        assert!(query.tags.contains(&"technical".to_string()));
        assert!(matches!(query.projects, ProjectScope::All));
    }

    #[test]
    fn test_pagination() {
        let pagination = Pagination::new(2, 50);
        assert_eq!(pagination.offset(), 100);
    }

    #[test]
    fn test_pagination_info() {
        let info = PaginationInfo::new(1, 10, 35);
        assert_eq!(info.total_pages, 4);
        assert!(info.has_next_page);
        assert!(info.has_previous_page);
    }
}
