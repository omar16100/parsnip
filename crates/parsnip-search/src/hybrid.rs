//! Hybrid search combining fuzzy and full-text search

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use parsnip_core::{Entity, EntityId, ProjectId, SearchQuery, SearchMode};
use crate::fuzzy::FuzzySearchEngine;
use crate::fulltext::FullTextSearchEngine;
use crate::traits::{Result, SearchEngine, SearchError, SearchHit};

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
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>> {
        match query.mode {
            SearchMode::Exact => {
                // For exact mode, use fulltext with exact matching
                self.fulltext.search(query).await
            }
            SearchMode::Fuzzy => {
                self.fuzzy.search(query).await
            }
            SearchMode::FullText => {
                self.fulltext.search(query).await
            }
            SearchMode::Hybrid => {
                // Combine results from both engines
                let fuzzy_hits = self.fuzzy.search(query).await?;
                let fulltext_hits = self.fulltext.search(query).await?;

                // Merge and deduplicate by entity_id
                let mut combined: HashMap<String, SearchHit> = HashMap::new();

                for hit in fuzzy_hits {
                    let key = hit.entity_id.to_string();
                    combined.entry(key).or_insert(hit);
                }

                for hit in fulltext_hits {
                    let key = hit.entity_id.to_string();
                    combined.entry(key)
                        .and_modify(|existing| {
                            // Boost score if found in both
                            existing.score = (existing.score + hit.score) / 2.0 * 1.2;
                        })
                        .or_insert(hit);
                }

                let mut hits: Vec<_> = combined.into_values().collect();
                hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

                Ok(hits)
            }
        }
    }

    async fn index_entity(&self, entity: &Entity) -> Result<()> {
        // Index in both engines
        self.fuzzy.index_entity(entity).await?;
        self.fulltext.index_entity(entity).await?;
        Ok(())
    }

    async fn remove_entity(&self, entity_id: &EntityId, project_id: &ProjectId) -> Result<()> {
        self.fuzzy.remove_entity(entity_id, project_id).await?;
        self.fulltext.remove_entity(entity_id, project_id).await?;
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
    use parsnip_core::NewEntity;

    #[tokio::test]
    async fn test_hybrid_search() {
        let engine = HybridSearchEngine::in_memory().unwrap();
        let project_id = ProjectId::new();

        let entity = NewEntity::new("John_Smith", "person")
            .unwrap()
            .with_observation("Senior engineer at Google")
            .build(project_id.clone());

        engine.index_entity(&entity).await.unwrap();

        // Test hybrid mode
        let query = SearchQuery::new()
            .text("john engineer")
            .mode(SearchMode::Hybrid);

        let hits = engine.search(&query).await.unwrap();
        assert!(!hits.is_empty());
    }
}
