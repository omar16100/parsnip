//! Vector/semantic search using embeddings

use async_trait::async_trait;

use parsnip_core::{Entity, ProjectScope, SearchQuery, TagMatchMode};
use crate::error::SearchResult;
use crate::traits::SearchEngine;

/// Stateless vector search engine using cosine similarity
pub struct VectorSearchEngine {
    pub default_threshold: f32,
}

impl VectorSearchEngine {
    pub fn new() -> Self {
        Self {
            default_threshold: 0.7,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.default_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Compute cosine similarity between two vectors
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
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
        if !query.entity_types.is_empty()
            && !query.entity_types.contains(&entity.entity_type.0)
        {
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

impl Default for VectorSearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchEngine for VectorSearchEngine {
    async fn search(
        &self,
        query: &SearchQuery,
        entities: &[Entity],
    ) -> SearchResult<Vec<Entity>> {
        let query_embedding = match &query.query_embedding {
            Some(emb) if !emb.is_empty() => emb,
            _ => {
                // No query embedding, just filter by other criteria
                tracing::debug!("No query embedding provided, returning filtered entities");
                return Ok(entities
                    .iter()
                    .filter(|e| Self::matches_filters(e, query))
                    .cloned()
                    .collect());
            }
        };

        let threshold = if query.similarity_threshold > 0.0 {
            query.similarity_threshold
        } else {
            self.default_threshold
        };

        // Score entities with embeddings
        let mut scored: Vec<(Entity, f32)> = entities
            .iter()
            .filter(|e| Self::matches_filters(e, query))
            .filter_map(|e| {
                e.embedding.as_ref().map(|entity_emb| {
                    let score = Self::cosine_similarity(query_embedding, entity_emb);
                    (e.clone(), score)
                })
            })
            .filter(|(_, score)| *score >= threshold)
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        tracing::debug!(
            "Vector search found {} entities above threshold {}",
            scored.len(),
            threshold
        );

        Ok(scored.into_iter().map(|(e, _)| e).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsnip_core::ProjectId;

    fn create_test_entity_with_embedding(
        name: &str,
        entity_type: &str,
        project_id: &ProjectId,
        embedding: Vec<f32>,
    ) -> Entity {
        let mut entity = Entity::new(project_id.clone(), name, entity_type);
        entity.embedding = Some(embedding);
        entity
    }

    #[tokio::test]
    async fn test_vector_search_finds_similar() {
        let search = VectorSearchEngine::new().with_threshold(0.5);
        let project_id = ProjectId::new();

        // Create entities with embeddings
        let entities = vec![
            create_test_entity_with_embedding(
                "rust_programming",
                "topic",
                &project_id,
                vec![1.0, 0.0, 0.0],
            ),
            create_test_entity_with_embedding(
                "python_programming",
                "topic",
                &project_id,
                vec![0.9, 0.1, 0.0],
            ),
            create_test_entity_with_embedding(
                "cooking_recipes",
                "topic",
                &project_id,
                vec![0.0, 0.0, 1.0],
            ),
        ];

        // Query for something similar to rust_programming
        let query = SearchQuery::empty()
            .with_embedding(vec![1.0, 0.0, 0.0])
            .with_similarity_threshold(0.5)
            .in_all_projects();

        let results = search.search(&query, &entities).await.unwrap();

        assert_eq!(results.len(), 2); // rust and python, not cooking
        assert_eq!(results[0].name, "rust_programming");
        assert_eq!(results[1].name, "python_programming");
    }

    #[tokio::test]
    async fn test_vector_search_respects_threshold() {
        let search = VectorSearchEngine::new();
        let project_id = ProjectId::new();

        let entities = vec![
            create_test_entity_with_embedding(
                "exact_match",
                "topic",
                &project_id,
                vec![1.0, 0.0, 0.0],
            ),
            create_test_entity_with_embedding(
                "partial_match",
                "topic",
                &project_id,
                vec![0.7, 0.3, 0.0],
            ),
        ];

        // High threshold should only match exact
        let query = SearchQuery::empty()
            .with_embedding(vec![1.0, 0.0, 0.0])
            .with_similarity_threshold(0.95)
            .in_all_projects();

        let results = search.search(&query, &entities).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "exact_match");
    }

    #[tokio::test]
    async fn test_vector_search_handles_missing_embeddings() {
        let search = VectorSearchEngine::new().with_threshold(0.5);
        let project_id = ProjectId::new();

        let mut entity_without = Entity::new(project_id.clone(), "no_embedding", "topic");
        entity_without.embedding = None;

        let entities = vec![
            create_test_entity_with_embedding(
                "has_embedding",
                "topic",
                &project_id,
                vec![1.0, 0.0, 0.0],
            ),
            entity_without,
        ];

        let query = SearchQuery::empty()
            .with_embedding(vec![1.0, 0.0, 0.0])
            .in_all_projects();

        let results = search.search(&query, &entities).await.unwrap();

        // Only entities with embeddings should be returned
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "has_embedding");
    }

    #[tokio::test]
    async fn test_cosine_similarity() {
        // Identical vectors
        assert!((VectorSearchEngine::cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 0.001);

        // Orthogonal vectors
        assert!((VectorSearchEngine::cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 0.001);

        // Opposite vectors
        assert!((VectorSearchEngine::cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 0.001);

        // Empty vectors
        assert_eq!(VectorSearchEngine::cosine_similarity(&[], &[]), 0.0);

        // Mismatched lengths
        assert_eq!(VectorSearchEngine::cosine_similarity(&[1.0], &[1.0, 0.0]), 0.0);
    }
}
