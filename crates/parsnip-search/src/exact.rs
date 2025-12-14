//! Exact search engine - simple substring matching

use async_trait::async_trait;

use crate::traits::{Result, SearchEngine};
use parsnip_core::{Entity, SearchQuery, TagMatchMode};

/// Simple exact substring search engine (stateless)
pub struct ExactSearchEngine;

impl ExactSearchEngine {
    pub fn new() -> Self {
        Self
    }

    fn create_searchable(entity: &Entity) -> String {
        let mut parts = vec![
            entity.name.to_lowercase(),
            entity.entity_type.0.to_lowercase(),
        ];
        parts.extend(entity.observations.iter().map(|o| o.content.to_lowercase()));
        parts.extend(entity.tags.iter().map(|t| t.to_lowercase()));
        parts.join(" ")
    }

    fn matches_query(entity: &Entity, query: &SearchQuery) -> bool {
        // Filter by entity type if specified
        if !query.entity_types.is_empty() {
            let type_match = query
                .entity_types
                .iter()
                .any(|t| t.to_lowercase() == entity.entity_type.0.to_lowercase());
            if !type_match {
                return false;
            }
        }

        // Filter by tags if specified
        if !query.tags.is_empty() {
            let search_tags: Vec<String> = query.tags.iter().map(|t| t.to_lowercase()).collect();
            let entity_tags: Vec<String> = entity.tags.iter().map(|t| t.to_lowercase()).collect();

            let tag_match = match query.tag_match_mode {
                TagMatchMode::Any => search_tags.iter().any(|t| entity_tags.contains(t)),
                TagMatchMode::All => search_tags.iter().all(|t| entity_tags.contains(t)),
            };
            if !tag_match {
                return false;
            }
        }

        // Filter by text if specified
        if let Some(ref search_text) = query.text {
            if !search_text.is_empty() {
                let searchable = Self::create_searchable(entity);
                let search_lower = search_text.to_lowercase();
                if !searchable.contains(&search_lower) {
                    return false;
                }
            }
        }

        true
    }
}

impl Default for ExactSearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchEngine for ExactSearchEngine {
    async fn search(&self, query: &SearchQuery, entities: &[Entity]) -> Result<Vec<Entity>> {
        let mut results: Vec<Entity> = entities
            .iter()
            .filter(|entity| Self::matches_query(entity, query))
            .cloned()
            .collect();

        // Apply pagination
        let offset = query.pagination.offset();
        let limit = query.pagination.page_size;

        if offset < results.len() {
            results = results.into_iter().skip(offset).take(limit).collect();
        } else {
            results = Vec::new();
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsnip_core::ProjectId;

    fn create_test_entity(name: &str, entity_type: &str, project_id: &ProjectId) -> Entity {
        let mut entity = Entity::new(project_id.clone(), name, entity_type);
        entity.add_observation("Test observation");
        entity.add_tag("test-tag");
        entity
    }

    #[tokio::test]
    async fn test_exact_search_by_name() {
        let search = ExactSearchEngine::new();
        let project_id = ProjectId::new();
        let entities = vec![
            create_test_entity("John_Smith", "person", &project_id),
            create_test_entity("Jane_Doe", "person", &project_id),
            create_test_entity("Google", "company", &project_id),
        ];

        let query = SearchQuery::text("John").in_all_projects();
        let results = search.search(&query, &entities).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "John_Smith");
    }

    #[tokio::test]
    async fn test_exact_search_by_type() {
        let search = ExactSearchEngine::new();
        let project_id = ProjectId::new();
        let entities = vec![
            create_test_entity("John_Smith", "person", &project_id),
            create_test_entity("Google", "company", &project_id),
        ];

        let query = SearchQuery::empty()
            .with_entity_type("person")
            .in_all_projects();
        let results = search.search(&query, &entities).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "John_Smith");
    }

    #[tokio::test]
    async fn test_exact_search_by_tag() {
        let search = ExactSearchEngine::new();
        let project_id = ProjectId::new();
        let mut entity1 = create_test_entity("John_Smith", "person", &project_id);
        entity1.add_tag("urgent");
        let entity2 = create_test_entity("Jane_Doe", "person", &project_id);
        let entities = vec![entity1, entity2];

        let query = SearchQuery::empty().with_tag("urgent").in_all_projects();
        let results = search.search(&query, &entities).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "John_Smith");
    }

    #[tokio::test]
    async fn test_empty_search() {
        let search = ExactSearchEngine::new();
        let entities = vec![];

        let query = SearchQuery::text("anything").in_all_projects();
        let results = search.search(&query, &entities).await.unwrap();
        assert!(results.is_empty());
    }
}
