//! MCP server implementation

use std::sync::Arc;

use std::collections::HashMap;
use parsnip_core::{Direction, Entity, Project, Relation, SearchMode, SearchQuery, TraversalEngine, TraversalQuery};
use parsnip_search::{ExactSearchEngine, FuzzySearchEngine, SearchEngine};
#[cfg(feature = "fulltext")]
use parsnip_search::FullTextSearchEngine;
use parsnip_storage::StorageBackend;
use serde::{Deserialize, Serialize};

use crate::handlers::ToolCallResponse;
use crate::tools::get_tools;
use crate::transport::{JsonRpcRequest, JsonRpcResponse, StdioTransport};

const SERVER_NAME: &str = "parsnip";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// MCP Server for Parsnip
pub struct McpServer<S: StorageBackend> {
    storage: Arc<S>,
}

impl<S: StorageBackend + Send + Sync + 'static> McpServer<S> {
    pub fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }

    /// Start the MCP server on stdio
    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        tracing::info!("Starting MCP server on stdio");

        loop {
            match StdioTransport::read_request().await {
                Ok(Some(request)) => {
                    tracing::debug!("Received request: {:?}", request.method);
                    let response = self.handle_request(request).await;
                    if let Err(e) = StdioTransport::write_response(&response).await {
                        tracing::error!("Failed to write response: {}", e);
                    }
                }
                Ok(None) => {
                    tracing::info!("EOF on stdin, shutting down");
                    break;
                }
                Err(e) => {
                    tracing::error!("Failed to read request: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a JSON-RPC request (public for SSE transport)
    pub async fn handle_request_public(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        self.handle_request(request).await
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id).await,
            "initialized" => JsonRpcResponse::success(request.id, serde_json::json!({})),
            "tools/list" => self.handle_tools_list(request.id).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,
            "ping" => JsonRpcResponse::success(request.id, serde_json::json!({})),
            _ => JsonRpcResponse::error(request.id, -32601, format!("Method not found: {}", request.method)),
        }
    }

    async fn handle_initialize(&self, id: serde_json::Value) -> JsonRpcResponse {
        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        });
        JsonRpcResponse::success(id, result)
    }

    async fn handle_tools_list(&self, id: serde_json::Value) -> JsonRpcResponse {
        let tools = get_tools();
        JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, id: serde_json::Value, params: serde_json::Value) -> JsonRpcResponse {
        #[derive(Deserialize)]
        struct ToolCallParams {
            name: String,
            #[serde(default)]
            arguments: serde_json::Value,
        }

        let params: ToolCallParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return JsonRpcResponse::error(id, -32602, format!("Invalid params: {}", e)),
        };

        tracing::debug!("Tool call: {} with args: {:?}", params.name, params.arguments);

        let response = match params.name.as_str() {
            "search_knowledge" => self.handle_search(params.arguments).await,
            "create_entities" => self.handle_create_entities(params.arguments).await,
            "add_observations" => self.handle_add_observations(params.arguments).await,
            "create_relations" => self.handle_create_relations(params.arguments).await,
            "delete_entities" => self.handle_delete_entities(params.arguments).await,
            "delete_relations" => self.handle_delete_relations(params.arguments).await,
            "delete_observations" => self.handle_delete_observations(params.arguments).await,
            "read_graph" => self.handle_read_graph(params.arguments).await,
            "open_nodes" => self.handle_open_nodes(params.arguments).await,
            "add_tags" => self.handle_add_tags(params.arguments).await,
            "remove_tags" => self.handle_remove_tags(params.arguments).await,
            "traverse_graph" => self.handle_traverse_graph(params.arguments).await,
            "list_projects" => self.handle_list_projects().await,
            _ => ToolCallResponse::error(format!("Unknown tool: {}", params.name)),
        };

        match serde_json::to_value(response) {
            Ok(val) => JsonRpcResponse::success(id, val),
            Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {}", e)),
        }
    }

    async fn get_or_create_project(&self, project_name: &str) -> anyhow::Result<Project> {
        if let Some(project) = self.storage.get_project(project_name).await? {
            return Ok(project);
        }
        let project = Project::new(project_name);
        self.storage.save_project(&project).await?;
        Ok(project)
    }

    async fn handle_search(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SearchArgs {
            query: Option<String>,
            project_id: Option<String>,
            search_mode: Option<String>,
            fuzzy_threshold: Option<f32>,
            exact_tags: Option<Vec<String>>,
            page: Option<usize>,
            page_size: Option<usize>,
        }

        let args: SearchArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        // Get entities
        let entities = if let Some(ref project_name) = args.project_id {
            match self.get_or_create_project(project_name).await {
                Ok(project) => match self.storage.get_all_entities(&project.id).await {
                    Ok(e) => e,
                    Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
                },
                Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
            }
        } else {
            match self.storage.get_all_entities_all_projects().await {
                Ok(e) => e,
                Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
            }
        };

        // Build query
        let mut query = if let Some(ref text) = args.query {
            SearchQuery::text(text)
        } else {
            SearchQuery::empty()
        };

        if let Some(ref tags) = args.exact_tags {
            for tag in tags {
                query = query.with_tag(tag);
            }
        }

        if let Some(ref mode) = args.search_mode {
            query = query.with_mode(match mode.as_str() {
                "fuzzy" => SearchMode::Fuzzy,
                "fulltext" => SearchMode::FullText,
                "hybrid" => SearchMode::Hybrid,
                _ => SearchMode::Exact,
            });
        }

        if let Some(threshold) = args.fuzzy_threshold {
            query = query.with_fuzzy_threshold(threshold);
        }

        if let Some(page) = args.page {
            query = query.with_pagination(page, args.page_size.unwrap_or(100));
        }

        // Perform search
        let results = match query.mode {
            SearchMode::Fuzzy => {
                let engine = FuzzySearchEngine::new();
                engine.search(&query, &entities).await
            }
            #[cfg(feature = "fulltext")]
            SearchMode::FullText | SearchMode::Hybrid => {
                match FullTextSearchEngine::in_memory() {
                    Ok(engine) => engine.search(&query, &entities).await,
                    Err(e) => {
                        tracing::warn!("Failed to create fulltext engine: {}, falling back to exact", e);
                        let engine = ExactSearchEngine::new();
                        engine.search(&query, &entities).await
                    }
                }
            }
            #[cfg(not(feature = "fulltext"))]
            SearchMode::FullText | SearchMode::Hybrid => {
                tracing::warn!("Fulltext search not enabled, falling back to exact");
                let engine = ExactSearchEngine::new();
                engine.search(&query, &entities).await
            }
            _ => {
                let engine = ExactSearchEngine::new();
                engine.search(&query, &entities).await
            }
        };

        match results {
            Ok(entities) => {
                let result = SearchResult {
                    entities: entities.iter().map(EntityResult::from).collect(),
                    relations: vec![],
                    pagination: PaginationInfo {
                        current_page: args.page.unwrap_or(0),
                        page_size: args.page_size.unwrap_or(100),
                        total_count: entities.len(),
                        total_pages: 1,
                        has_next_page: false,
                        has_previous_page: args.page.unwrap_or(0) > 0,
                    },
                };
                ToolCallResponse::text(serde_json::to_string_pretty(&result).unwrap())
            }
            Err(e) => ToolCallResponse::error(format!("Search error: {}", e)),
        }
    }

    async fn handle_create_entities(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CreateArgs {
            project_id: Option<String>,
            entities: Vec<EntityInput>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct EntityInput {
            name: String,
            entity_type: String,
            observations: Vec<String>,
            #[serde(default)]
            tags: Vec<String>,
        }

        let args: CreateArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut created = 0;
        for input in args.entities {
            let mut entity = Entity::new(project.id.clone(), &input.name, input.entity_type.as_str());
            for obs in input.observations {
                entity.add_observation(&obs);
            }
            for tag in input.tags {
                entity.add_tag(&tag);
            }
            if let Err(e) = self.storage.save_entity(&entity).await {
                return ToolCallResponse::error(format!("Failed to save entity: {}", e));
            }
            created += 1;
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Created {} entities", created))
    }

    async fn handle_add_observations(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AddObsArgs {
            project_id: Option<String>,
            observations: Vec<ObservationInput>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ObservationInput {
            entity_name: String,
            observations: Vec<String>,
        }

        let args: AddObsArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut added = 0;
        for input in args.observations {
            let entity = match self.storage.get_entity(&input.entity_name, &project.id).await {
                Ok(Some(e)) => e,
                Ok(None) => return ToolCallResponse::error(format!("Entity not found: {}", input.entity_name)),
                Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
            };

            let mut updated = entity;
            for obs in &input.observations {
                updated.add_observation(obs);
            }
            if let Err(e) = self.storage.save_entity(&updated).await {
                return ToolCallResponse::error(format!("Failed to save entity: {}", e));
            }
            added += input.observations.len();
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Added {} observations", added))
    }

    async fn handle_create_relations(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CreateRelArgs {
            project_id: Option<String>,
            relations: Vec<RelationInput>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RelationInput {
            from: String,
            to: String,
            relation_type: String,
        }

        let args: CreateRelArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut created = 0;
        for input in args.relations {
            let relation = Relation::from_names(
                project.id.clone(),
                &input.from,
                &input.to,
                &input.relation_type,
            );
            if let Err(e) = self.storage.save_relation(&relation).await {
                return ToolCallResponse::error(format!("Failed to save relation: {}", e));
            }
            created += 1;
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Created {} relations", created))
    }

    async fn handle_delete_entities(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DeleteArgs {
            project_id: Option<String>,
            entity_names: Vec<String>,
        }

        let args: DeleteArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut deleted = 0;
        for name in args.entity_names {
            if let Err(e) = self.storage.delete_relations_for_entity(&name, &project.id).await {
                tracing::warn!("Failed to delete relations for {}: {}", name, e);
            }
            if let Err(e) = self.storage.delete_entity(&name, &project.id).await {
                return ToolCallResponse::error(format!("Failed to delete entity: {}", e));
            }
            deleted += 1;
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Deleted {} entities", deleted))
    }

    async fn handle_delete_relations(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DeleteRelArgs {
            project_id: Option<String>,
            relations: Vec<RelationRef>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RelationRef {
            from: String,
            to: String,
            relation_type: String,
        }

        let args: DeleteRelArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut deleted = 0;
        for rel in args.relations {
            if let Err(e) = self.storage.delete_relation(&rel.from, &rel.to, &rel.relation_type, &project.id).await {
                return ToolCallResponse::error(format!("Failed to delete relation: {}", e));
            }
            deleted += 1;
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Deleted {} relations", deleted))
    }

    async fn handle_delete_observations(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DeleteObsArgs {
            project_id: Option<String>,
            deletions: Vec<ObsDeletion>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ObsDeletion {
            entity_name: String,
            observations: Vec<String>,
        }

        let args: DeleteObsArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut deleted = 0;
        for del in args.deletions {
            let entity = match self.storage.get_entity(&del.entity_name, &project.id).await {
                Ok(Some(e)) => e,
                Ok(None) => return ToolCallResponse::error(format!("Entity not found: {}", del.entity_name)),
                Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
            };

            let mut updated = entity;
            updated.observations.retain(|o| !del.observations.contains(&o.content));
            deleted += del.observations.len();

            if let Err(e) = self.storage.save_entity(&updated).await {
                return ToolCallResponse::error(format!("Failed to save entity: {}", e));
            }
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Deleted {} observations", deleted))
    }

    async fn handle_read_graph(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ReadGraphArgs {
            project_id: Option<String>,
        }

        let args: ReadGraphArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let entities = match self.storage.get_all_entities(&project.id).await {
            Ok(e) => e,
            Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
        };

        let relations = match self.storage.get_all_relations(&project.id).await {
            Ok(r) => r,
            Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
        };

        let result = GraphResult {
            entities: entities.iter().map(EntityResult::from).collect(),
            relations: relations.iter().map(RelationResult::from).collect(),
        };

        ToolCallResponse::text(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn handle_open_nodes(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct OpenNodesArgs {
            project_id: Option<String>,
            names: Vec<String>,
        }

        let args: OpenNodesArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut entities = Vec::new();
        let mut relations = Vec::new();

        for name in &args.names {
            if let Ok(Some(entity)) = self.storage.get_entity(name, &project.id).await {
                entities.push(EntityResult::from(&entity));

                if let Ok(rels) = self.storage.get_relations_for_entity(name, &project.id).await {
                    for rel in rels {
                        relations.push(RelationResult::from(&rel));
                    }
                }
            }
        }

        let result = GraphResult { entities, relations };
        ToolCallResponse::text(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn handle_add_tags(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AddTagsArgs {
            project_id: Option<String>,
            updates: Vec<TagUpdate>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TagUpdate {
            entity_name: String,
            tags: Vec<String>,
        }

        let args: AddTagsArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut added = 0;
        for update in args.updates {
            let entity = match self.storage.get_entity(&update.entity_name, &project.id).await {
                Ok(Some(e)) => e,
                Ok(None) => return ToolCallResponse::error(format!("Entity not found: {}", update.entity_name)),
                Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
            };

            let mut updated = entity;
            for tag in &update.tags {
                updated.add_tag(tag);
            }
            added += update.tags.len();

            if let Err(e) = self.storage.save_entity(&updated).await {
                return ToolCallResponse::error(format!("Failed to save entity: {}", e));
            }
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Added {} tags", added))
    }

    async fn handle_remove_tags(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RemoveTagsArgs {
            project_id: Option<String>,
            updates: Vec<TagUpdate>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TagUpdate {
            entity_name: String,
            tags: Vec<String>,
        }

        let args: RemoveTagsArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        let mut removed = 0;
        for update in args.updates {
            let entity = match self.storage.get_entity(&update.entity_name, &project.id).await {
                Ok(Some(e)) => e,
                Ok(None) => return ToolCallResponse::error(format!("Entity not found: {}", update.entity_name)),
                Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
            };

            let mut updated = entity;
            updated.tags.retain(|t| !update.tags.contains(t));
            removed += update.tags.len();

            if let Err(e) = self.storage.save_entity(&updated).await {
                return ToolCallResponse::error(format!("Failed to save entity: {}", e));
            }
        }

        ToolCallResponse::text(format!("✅ SUCCESS: Removed {} tags", removed))
    }

    async fn handle_traverse_graph(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TraverseArgs {
            project_id: Option<String>,
            start: String,
            target: Option<String>,
            max_depth: Option<u32>,
            direction: Option<String>,
            entity_type_filter: Option<Vec<String>>,
            relation_type_filter: Option<Vec<String>>,
            use_weights: Option<bool>,
        }

        let args: TraverseArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project_name = args.project_id.as_deref().unwrap_or("default");
        let project = match self.get_or_create_project(project_name).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Project error: {}", e)),
        };

        // Load entities and relations
        let entities: HashMap<String, Entity> = match self.storage.get_all_entities(&project.id).await {
            Ok(e) => e.into_iter().map(|ent| (ent.name.clone(), ent)).collect(),
            Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
        };

        let relations = match self.storage.get_all_relations(&project.id).await {
            Ok(r) => r,
            Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
        };

        // Check if start entity exists
        if !entities.contains_key(&args.start) {
            return ToolCallResponse::error(format!("Start entity '{}' not found", args.start));
        }

        // Check if target entity exists (if specified)
        if let Some(ref target) = args.target {
            if !entities.contains_key(target) {
                return ToolCallResponse::error(format!("Target entity '{}' not found", target));
            }
        }

        // Build traversal query
        let direction = match args.direction.as_deref() {
            Some("outgoing") => Direction::Outgoing,
            Some("incoming") => Direction::Incoming,
            _ => Direction::Both,
        };

        let mut query = TraversalQuery::new(&args.start)
            .with_depth(args.max_depth.unwrap_or(10))
            .with_direction(direction);

        if let Some(target) = args.target {
            query = query.find_path_to(&target);
        }

        if args.use_weights.unwrap_or(false) {
            query = query.weighted();
        }

        if let Some(ref etypes) = args.entity_type_filter {
            query = query.filter_entity_types(etypes.clone());
        }

        if let Some(ref rtypes) = args.relation_type_filter {
            query = query.filter_relation_types(rtypes.clone());
        }

        tracing::info!(
            "Traversing from '{}' (target: {:?}, depth: {}, direction: {:?})",
            args.start, query.target, query.max_depth, query.direction
        );

        // Execute traversal
        let result = TraversalEngine::execute(&query, &entities, &relations);

        // Convert to JSON response
        let response = TraversalResultJson {
            paths: result.paths.iter().map(|p| PathJson {
                nodes: p.nodes.clone(),
                edges: p.edges.iter().map(|e| PathEdgeJson {
                    from: e.from.clone(),
                    to: e.to.clone(),
                    relation_type: e.relation_type.clone(),
                    weight: e.weight,
                }).collect(),
                total_weight: p.total_weight,
                length: p.length,
            }).collect(),
            visited_entities: result.visited_entities.clone(),
            entities: result.entities.iter().map(EntityResult::from).collect(),
            relations: result.relations.iter().map(RelationResult::from).collect(),
            stats: TraversalStatsJson {
                nodes_visited: result.stats.nodes_visited,
                edges_traversed: result.stats.edges_traversed,
                max_depth_reached: result.stats.max_depth_reached,
            },
        };

        ToolCallResponse::text(serde_json::to_string_pretty(&response).unwrap())
    }

    async fn handle_list_projects(&self) -> ToolCallResponse {
        let projects = match self.storage.get_all_projects().await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(format!("Storage error: {}", e)),
        };

        let mut results = Vec::new();
        for project in projects {
            let entity_count = self.storage.get_all_entities(&project.id).await
                .map(|e| e.len())
                .unwrap_or(0);
            let relation_count = self.storage.get_all_relations(&project.id).await
                .map(|r| r.len())
                .unwrap_or(0);

            results.push(ProjectResult {
                name: project.name,
                description: project.description,
                entity_count,
                relation_count,
                created_at: project.created_at.to_rfc3339(),
            });
        }

        let response = ProjectListResult { projects: results };
        ToolCallResponse::text(serde_json::to_string_pretty(&response).unwrap())
    }
}

// Result types for JSON responses
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    entities: Vec<EntityResult>,
    relations: Vec<RelationResult>,
    pagination: PaginationInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphResult {
    entities: Vec<EntityResult>,
    relations: Vec<RelationResult>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityResult {
    name: String,
    entity_type: String,
    observations: Vec<String>,
    tags: Vec<String>,
}

impl From<&Entity> for EntityResult {
    fn from(e: &Entity) -> Self {
        Self {
            name: e.name.clone(),
            entity_type: e.entity_type.0.clone(),
            observations: e.observations.iter().map(|o| o.content.clone()).collect(),
            tags: e.tags.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RelationResult {
    from: String,
    to: String,
    relation_type: String,
}

impl From<&Relation> for RelationResult {
    fn from(r: &Relation) -> Self {
        Self {
            from: r.from_name.clone(),
            to: r.to_name.clone(),
            relation_type: r.relation_type.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaginationInfo {
    current_page: usize,
    page_size: usize,
    total_count: usize,
    total_pages: usize,
    has_next_page: bool,
    has_previous_page: bool,
}

// Traversal result types
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TraversalResultJson {
    paths: Vec<PathJson>,
    visited_entities: Vec<String>,
    entities: Vec<EntityResult>,
    relations: Vec<RelationResult>,
    stats: TraversalStatsJson,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathJson {
    nodes: Vec<String>,
    edges: Vec<PathEdgeJson>,
    total_weight: f64,
    length: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathEdgeJson {
    from: String,
    to: String,
    relation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    weight: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TraversalStatsJson {
    nodes_visited: usize,
    edges_traversed: usize,
    max_depth_reached: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectListResult {
    projects: Vec<ProjectResult>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectResult {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    entity_count: usize,
    relation_count: usize,
    created_at: String,
}
