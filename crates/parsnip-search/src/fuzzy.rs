//! Fuzzy search using nucleo

use async_trait::async_trait;
use nucleo_matcher::{
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
    Config, Matcher,
};

use crate::error::SearchResult;
use crate::traits::SearchEngine;
use parsnip_core::{Entity, ProjectScope, SearchQuery, TagMatchMode};

/// Stateless fuzzy search engine using nucleo
pub struct FuzzySearchEngine {
    pub default_threshold: f32,
}

impl FuzzySearchEngine {
    pub fn new() -> Self {
        Self {
            default_threshold: 0.3,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.default_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    fn create_searchable(entity: &Entity) -> String {
        let mut parts = vec![entity.name.clone(), entity.entity_type.0.clone()];
        parts.extend(entity.observations.iter().map(|o| o.content.clone()));
        parts.extend(entity.tags.clone());
        parts.join(" ")
    }

    fn score_entity(entity: &Entity, pattern: &str, matcher: &mut Matcher) -> Option<u32> {
        let pat = Pattern::new(
            pattern,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let searchable = Self::create_searchable(entity);
        let mut buf = Vec::new();
        pat.score(
            nucleo_matcher::Utf32Str::new(&searchable, &mut buf),
            matcher,
        )
    }

    fn matches_filters(entity: &Entity, query: &SearchQuery) -> bool {
        // Check project scope
        let project_matches = match &query.projects {
            ProjectScope::All => true,
            ProjectScope::Single(pid) => entity.project_id == *pid,
            ProjectScope::Multiple(pids) => pids.contains(&entity.project_id),
        };

        if !project_matches {
            return false;
        }

        // Check entity type filter
        if !query.entity_types.is_empty() && !query.entity_types.contains(&entity.entity_type.0) {
            return false;
        }

        // Check tag filter
        if !query.tags.is_empty() {
            let tag_matches = match query.tag_match_mode {
                TagMatchMode::Any => query.tags.iter().any(|t| entity.tags.contains(t)),
                TagMatchMode::All => query.tags.iter().all(|t| entity.tags.contains(t)),
            };
            if !tag_matches {
                return false;
            }
        }

        true
    }
}

impl Default for FuzzySearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchEngine for FuzzySearchEngine {
    async fn search(&self, query: &SearchQuery, entities: &[Entity]) -> SearchResult<Vec<Entity>> {
        let search_text = match &query.text {
            Some(t) if !t.is_empty() => t,
            _ => {
                // No text query, just filter by other criteria
                return Ok(entities
                    .iter()
                    .filter(|e| Self::matches_filters(e, query))
                    .cloned()
                    .collect());
            }
        };

        let mut matcher = Matcher::new(Config::DEFAULT);

        // Score and filter entities
        let mut scored: Vec<(Entity, u32)> = entities
            .iter()
            .filter(|e| Self::matches_filters(e, query))
            .filter_map(|e| {
                Self::score_entity(e, search_text, &mut matcher).map(|score| (e.clone(), score))
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(scored.into_iter().map(|(e, _)| e).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsnip_core::ProjectId;

    fn create_test_entity(name: &str, entity_type: &str, project_id: &ProjectId) -> Entity {
        let mut entity = Entity::new(project_id.clone(), name, entity_type);
        entity.add_observation("Test observation about Rust programming");
        entity
    }

    #[tokio::test]
    async fn test_fuzzy_search() {
        let search = FuzzySearchEngine::new();
        let project_id = ProjectId::new();
        let entities = vec![
            create_test_entity("John_Smith", "person", &project_id),
            create_test_entity("Jane_Doe", "person", &project_id),
            create_test_entity("Johnny_Appleseed", "person", &project_id),
        ];

        let query = SearchQuery::text("John").in_all_projects();
        let results = search.search(&query, &entities).await.unwrap();

        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.name == "John_Smith"));
    }

    #[tokio::test]
    async fn test_fuzzy_search_typo() {
        let search = FuzzySearchEngine::new();
        let project_id = ProjectId::new();
        let entities = vec![
            create_test_entity("John_Smith", "person", &project_id),
            create_test_entity("Jane_Doe", "person", &project_id),
        ];

        let query = SearchQuery::text("Jhon").in_all_projects();
        let results = search.search(&query, &entities).await.unwrap();

        // Fuzzy should still find John even with typo
        assert!(results.iter().any(|e| e.name == "John_Smith"));
    }
}
