//! Hybrid search combining fuzzy and full-text search

use async_trait::async_trait;
use std::collections::HashSet;
use std::path::Path;

use crate::fulltext::FullTextSearchEngine;
use crate::fuzzy::FuzzySearchEngine;
use crate::traits::{Result, SearchEngine};
use parsnip_core::{Entity, ProjectId, SearchMode, SearchQuery};

/// Hybrid search engine combining fuzzy and full-text search
pub struct HybridSearchEngine {
    fuzzy: FuzzySearchEngine,
    fulltext: FullTextSearchEngine,
}

impl HybridSearchEngine {
    pub fn new(index_path: &Path) -> Result<Self> {
        Ok(Self {
            fuzzy: FuzzySearchEngine::new(),
            fulltext: FullTextSearchEngine::new(index_path)?,
        })
    }

    pub fn in_memory() -> Result<Self> {
        Ok(Self {
            fuzzy: FuzzySearchEngine::new(),
            fulltext: FullTextSearchEngine::in_memory()?,
        })
    }
}

#[async_trait]
impl SearchEngine for HybridSearchEngine {
    async fn search(&self, query: &SearchQuery, entities: &[Entity]) -> Result<Vec<Entity>> {
        match query.mode {
            SearchMode::Exact => {
                // For exact mode, use fulltext with exact matching
                self.fulltext.search(query, entities).await
            }
            SearchMode::Fuzzy => self.fuzzy.search(query, entities).await,
            SearchMode::FullText => self.fulltext.search(query, entities).await,
            SearchMode::Hybrid => {
                // Combine results from both engines
                let fuzzy_results = self.fuzzy.search(query, entities).await?;
                let fulltext_results = self.fulltext.search(query, entities).await?;

                // Merge and deduplicate by name
                let mut seen: HashSet<String> = HashSet::new();
                let mut combined = Vec::new();

                // Prioritize fulltext results (typically more relevant)
                for entity in fulltext_results {
                    if seen.insert(entity.name.clone()) {
                        combined.push(entity);
                    }
                }

                // Add fuzzy results not already included
                for entity in fuzzy_results {
                    if seen.insert(entity.name.clone()) {
                        combined.push(entity);
                    }
                }

                Ok(combined)
            }
            SearchMode::Vector => {
                // Vector search requires VectorSearchEngine, not available in HybridSearchEngine
                tracing::warn!("Vector search mode not supported in HybridSearchEngine");
                Ok(Vec::new())
            }
        }
    }

    async fn index_entity(&self, entity: &Entity, project_id: &ProjectId) -> Result<()> {
        // Index in both engines
        self.fuzzy.index_entity(entity, project_id).await?;
        self.fulltext.index_entity(entity, project_id).await?;
        Ok(())
    }

    async fn remove_entity(&self, entity_name: &str, project_id: &ProjectId) -> Result<()> {
        self.fuzzy.remove_entity(entity_name, project_id).await?;
        self.fulltext.remove_entity(entity_name, project_id).await?;
        Ok(())
    }

    async fn rebuild_index(&self, entities: &[Entity]) -> Result<()> {
        self.fuzzy.rebuild_index(entities).await?;
        self.fulltext.rebuild_index(entities).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hybrid_search() {
        let engine = HybridSearchEngine::in_memory().unwrap();
        let project_id = ProjectId::new();

        let mut entity = parsnip_core::Entity::new(project_id.clone(), "John_Smith", "person");
        entity.add_observation("Senior engineer at Google");

        let entities = vec![entity];
        let query = SearchQuery::new("john engineer").with_mode(SearchMode::Hybrid);

        let results = engine.search(&query, &entities).await.unwrap();
        assert!(!results.is_empty());
    }
}
