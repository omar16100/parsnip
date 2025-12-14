//! Full-text search using Tantivy

use async_trait::async_trait;
use std::path::Path;
use std::sync::RwLock;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{Schema, Field, Value, STORED, TEXT, STRING},
    Index, IndexReader, IndexWriter, TantivyDocument,
    ReloadPolicy,
};

use parsnip_core::{Entity, ProjectId, SearchQuery};
use crate::traits::{Result, SearchEngine, SearchError};

/// Full-text search engine using Tantivy
pub struct FullTextSearchEngine {
    index: Index,
    reader: IndexReader,
    writer: RwLock<IndexWriter>,
    #[allow(dead_code)]
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
    async fn search(&self, query: &SearchQuery, entities: &[Entity]) -> Result<Vec<Entity>> {
        let text = match &query.text {
            Some(t) if !t.is_empty() => t,
            _ => return Ok(Vec::new()),
        };

        // Rebuild index with provided entities for accurate search
        self.rebuild_index(entities).await?;

        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.name_field, self.content_field]);

        let parsed_query = query_parser
            .parse_query(text)
            .map_err(|e| SearchError::Query(e.to_string()))?;

        let limit = query.pagination.page_size;
        let top_docs = searcher
            .search(&parsed_query, &TopDocs::with_limit(limit))
            .map_err(|e| SearchError::Query(e.to_string()))?;

        // Collect matching entity names
        let mut matching_names = std::collections::HashSet::new();
        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)
                .map_err(|e| SearchError::Internal(e.to_string()))?;

            if let Some(name) = doc.get_first(self.name_field).and_then(|v| v.as_str()) {
                matching_names.insert(name.to_string());
            }
        }

        // Return matching entities
        Ok(entities.iter()
            .filter(|e| matching_names.contains(&e.name))
            .cloned()
            .collect())
    }

    async fn index_entity(&self, entity: &Entity, _project_id: &ProjectId) -> Result<()> {
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

    async fn remove_entity(&self, entity_name: &str, _project_id: &ProjectId) -> Result<()> {
        let mut writer = self.writer.write()
            .map_err(|e| SearchError::Internal(format!("Lock error: {}", e)))?;

        let term = tantivy::Term::from_field_text(self.name_field, entity_name);
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

    #[tokio::test]
    async fn test_fulltext_search() {
        let engine = FullTextSearchEngine::in_memory().unwrap();
        let project_id = ProjectId::new();

        let mut entity = parsnip_core::Entity::new(project_id.clone(), "John_Smith", "person");
        entity.add_observation("Senior engineer at Google working on distributed systems");

        let entities = vec![entity];
        let query = SearchQuery::new("distributed systems");
        let results = engine.search(&query, &entities).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].name, "John_Smith");
    }
}
