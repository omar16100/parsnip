//! Full-text search using Tantivy

use async_trait::async_trait;
use std::path::Path;
use std::sync::RwLock;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{Schema, Field, STORED, TEXT, STRING},
    Index, IndexReader, IndexWriter, Document, TantivyDocument,
    ReloadPolicy,
};

use parsnip_core::{Entity, EntityId, ProjectId, SearchQuery};
use crate::traits::{Result, SearchEngine, SearchError, SearchHit};

/// Full-text search engine using Tantivy
pub struct FullTextSearchEngine {
    index: Index,
    reader: IndexReader,
    writer: RwLock<IndexWriter>,
    schema: Schema,
    // Fields
    entity_id_field: Field,
    project_id_field: Field,
    name_field: Field,
    content_field: Field,
}

impl FullTextSearchEngine {
    pub fn new(index_path: &Path) -> Result<Self> {
        let mut schema_builder = Schema::builder();
        
        let entity_id_field = schema_builder.add_text_field("entity_id", STRING | STORED);
        let project_id_field = schema_builder.add_text_field("project_id", STRING | STORED);
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        
        let schema = schema_builder.build();

        let dir = MmapDirectory::open(index_path)
            .map_err(|e| SearchError::Index(e.to_string()))?;
        
        let index = Index::open_or_create(dir, schema.clone())
            .map_err(|e| SearchError::Index(e.to_string()))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        let writer = index
            .writer(50_000_000) // 50MB buffer
            .map_err(|e| SearchError::Index(e.to_string()))?;

        Ok(Self {
            index,
            reader,
            writer: RwLock::new(writer),
            schema,
            entity_id_field,
            project_id_field,
            name_field,
            content_field,
        })
    }

    /// Create in-memory index for testing
    pub fn in_memory() -> Result<Self> {
        let mut schema_builder = Schema::builder();
        
        let entity_id_field = schema_builder.add_text_field("entity_id", STRING | STORED);
        let project_id_field = schema_builder.add_text_field("project_id", STRING | STORED);
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        
        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema.clone());

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        let writer = index
            .writer(50_000_000)
            .map_err(|e| SearchError::Index(e.to_string()))?;

        Ok(Self {
            index,
            reader,
            writer: RwLock::new(writer),
            schema,
            entity_id_field,
            project_id_field,
            name_field,
            content_field,
        })
    }

    fn create_document(&self, entity: &Entity) -> TantivyDocument {
        let mut doc = TantivyDocument::new();
        doc.add_text(self.entity_id_field, entity.id.to_string());
        doc.add_text(self.project_id_field, entity.project_id.to_string());
        doc.add_text(self.name_field, &entity.name);
        
        // Combine all searchable content
        let content: String = std::iter::once(entity.name.as_str())
            .chain(std::iter::once(entity.entity_type.0.as_str()))
            .chain(entity.observations.iter().map(|o| o.content.as_str()))
            .chain(entity.tags.iter().map(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        
        doc.add_text(self.content_field, content);
        doc
    }
}

#[async_trait]
impl SearchEngine for FullTextSearchEngine {
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>> {
        let text = match &query.text {
            Some(t) if !t.is_empty() => t,
            _ => return Ok(Vec::new()),
        };

        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.name_field, self.content_field]);
        
        let parsed_query = query_parser
            .parse_query(text)
            .map_err(|e| SearchError::Query(e.to_string()))?;

        let limit = query.pagination.page_size;
        let top_docs = searcher
            .search(&parsed_query, &TopDocs::with_limit(limit))
            .map_err(|e| SearchError::Query(e.to_string()))?;

        let mut hits = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)
                .map_err(|e| SearchError::Internal(e.to_string()))?;
            
            let entity_id_str = doc
                .get_first(self.entity_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            
            let project_id_str = doc
                .get_first(self.project_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            let name = doc
                .get_first(self.name_field)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            if let (Ok(entity_id), Ok(project_id)) = (
                EntityId::from_string(entity_id_str),
                ProjectId::from_string(project_id_str),
            ) {
                hits.push(SearchHit {
                    entity_id,
                    project_id,
                    name,
                    score,
                });
            }
        }

        Ok(hits)
    }

    async fn index_entity(&self, entity: &Entity) -> Result<()> {
        let doc = self.create_document(entity);
        
        let mut writer = self.writer.write()
            .map_err(|e| SearchError::Internal(format!("Lock error: {}", e)))?;
        
        // Delete existing document with same entity_id
        let term = tantivy::Term::from_field_text(self.entity_id_field, &entity.id.to_string());
        writer.delete_term(term);
        
        writer.add_document(doc)
            .map_err(|e| SearchError::Index(e.to_string()))?;
        
        writer.commit()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        self.reader.reload()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        Ok(())
    }

    async fn remove_entity(&self, entity_id: &EntityId, _project_id: &ProjectId) -> Result<()> {
        let mut writer = self.writer.write()
            .map_err(|e| SearchError::Internal(format!("Lock error: {}", e)))?;
        
        let term = tantivy::Term::from_field_text(self.entity_id_field, &entity_id.to_string());
        writer.delete_term(term);
        
        writer.commit()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        self.reader.reload()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        Ok(())
    }

    async fn rebuild_index(&self, entities: &[Entity]) -> Result<()> {
        let mut writer = self.writer.write()
            .map_err(|e| SearchError::Internal(format!("Lock error: {}", e)))?;
        
        writer.delete_all_documents()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        for entity in entities {
            let doc = self.create_document(entity);
            writer.add_document(doc)
                .map_err(|e| SearchError::Index(e.to_string()))?;
        }

        writer.commit()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        self.reader.reload()
            .map_err(|e| SearchError::Index(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsnip_core::NewEntity;

    #[tokio::test]
    async fn test_fulltext_search() {
        let engine = FullTextSearchEngine::in_memory().unwrap();
        let project_id = ProjectId::new();

        let entity = NewEntity::new("John_Smith", "person")
            .unwrap()
            .with_observation("Senior engineer at Google working on distributed systems")
            .build(project_id.clone());

        engine.index_entity(&entity).await.unwrap();

        let query = SearchQuery::new().text("distributed systems");
        let hits = engine.search(&query).await.unwrap();
        
        assert!(!hits.is_empty());
        assert_eq!(hits[0].name, "John_Smith");
    }
}
