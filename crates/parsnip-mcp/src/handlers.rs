//! MCP request handlers

use std::sync::Arc;

use parsnip_core::{Entity, Project, Relation, SearchQuery};
use parsnip_search::{ExactSearchEngine, FuzzySearchEngine, SearchEngine};
use parsnip_storage::StorageBackend;
use serde::{Deserialize, Serialize};

/// MCP tool call request
#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// MCP tool call response
#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
}

/// Content block for responses
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

impl ToolCallResponse {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
            is_error: None,
        }
    }

    pub fn json<T: Serialize>(data: &T) -> Self {
        match serde_json::to_string_pretty(data) {
            Ok(json) => Self::text(json),
            Err(e) => Self::error(format!("JSON serialization error: {}", e)),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: message.into(),
            }],
            is_error: Some(true),
        }
    }
}

/// Tool handler that processes tool calls
pub struct ToolHandler<S: StorageBackend> {
    storage: Arc<S>,
}

impl<S: StorageBackend + 'static> ToolHandler<S> {
    pub fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }

    pub async fn handle(&self, request: ToolCallRequest) -> ToolCallResponse {
        tracing::debug!("Handling tool call: {}", request.name);

        match request.name.as_str() {
            "search_knowledge" => self.search_knowledge(request.arguments).await,
            "create_entities" => self.create_entities(request.arguments).await,
            "add_observations" => self.add_observations(request.arguments).await,
            "create_relations" => self.create_relations(request.arguments).await,
            "delete_entities" => self.delete_entities(request.arguments).await,
            "delete_relations" => self.delete_relations(request.arguments).await,
            "read_graph" => self.read_graph(request.arguments).await,
            "open_nodes" => self.open_nodes(request.arguments).await,
            _ => ToolCallResponse::error(format!("Unknown tool: {}", request.name)),
        }
    }

    async fn get_or_create_project(&self, project_id: Option<&str>) -> Result<Project, String> {
        let name = project_id.unwrap_or("default");

        match self.storage.get_project(name).await {
            Ok(Some(project)) => Ok(project),
            Ok(None) => {
                let project = Project::new(name);
                self.storage
                    .save_project(&project)
                    .await
                    .map_err(|e| e.to_string())?;
                tracing::info!("Created new project: {}", name);
                Ok(project)
            }
            Err(e) => Err(e.to_string()),
        }
    }

    async fn search_knowledge(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            query: Option<String>,
            project_id: Option<String>,
            search_all: Option<bool>,
            #[serde(rename = "searchMode")]
            search_mode: Option<String>,
            #[serde(rename = "fuzzyThreshold")]
            fuzzy_threshold: Option<f32>,
            #[serde(rename = "exactTags")]
            exact_tags: Option<Vec<String>>,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        // Build query
        let mut query = if let Some(ref q) = args.query {
            SearchQuery::new(q)
        } else {
            SearchQuery::empty()
        };

        // Add tags
        if let Some(tags) = args.exact_tags {
            for tag in tags {
                query = query.with_tag(tag);
            }
        }

        // Set fuzzy threshold
        if let Some(threshold) = args.fuzzy_threshold {
            query = query.with_fuzzy_threshold(threshold);
        }

        // Get entities to search (default: search all projects)
        let entities = if args.search_all.unwrap_or(true) {
            match self.storage.get_all_entities_all_projects().await {
                Ok(e) => e,
                Err(e) => return ToolCallResponse::error(format!("Failed to get entities: {}", e)),
            }
        } else {
            let project = match self.get_or_create_project(args.project_id.as_deref()).await {
                Ok(p) => p,
                Err(e) => return ToolCallResponse::error(e),
            };
            match self.storage.get_all_entities(&project.id).await {
                Ok(e) => e,
                Err(e) => return ToolCallResponse::error(format!("Failed to get entities: {}", e)),
            }
        };

        // Perform search
        let results = match args.search_mode.as_deref() {
            Some("fuzzy") => {
                let engine = FuzzySearchEngine::new();
                engine.search(&query, &entities).await
            }
            _ => {
                let engine = ExactSearchEngine::new();
                engine.search(&query, &entities).await
            }
        };

        match results {
            Ok(entities) => {
                #[derive(Serialize)]
                struct SearchResult {
                    entities: Vec<EntityOutput>,
                    relations: Vec<RelationOutput>,
                }

                #[derive(Serialize)]
                struct EntityOutput {
                    name: String,
                    #[serde(rename = "entityType")]
                    entity_type: String,
                    observations: Vec<String>,
                    tags: Vec<String>,
                }

                #[derive(Serialize)]
                struct RelationOutput {
                    from: String,
                    to: String,
                    #[serde(rename = "relationType")]
                    relation_type: String,
                }

                let entity_outputs: Vec<EntityOutput> = entities
                    .iter()
                    .map(|e| EntityOutput {
                        name: e.name.clone(),
                        entity_type: e.entity_type.0.clone(),
                        observations: e.observations.iter().map(|o| o.content.clone()).collect(),
                        tags: e.tags.clone(),
                    })
                    .collect();

                // Get relations for found entities
                let mut all_relations = Vec::new();
                for entity in &entities {
                    if let Ok(rels) = self
                        .storage
                        .get_relations_for_entity(&entity.name, &entity.project_id)
                        .await
                    {
                        for rel in rels {
                            all_relations.push(RelationOutput {
                                from: rel.from_name.clone(),
                                to: rel.to_name.clone(),
                                relation_type: rel.relation_type.clone(),
                            });
                        }
                    }
                }

                // Deduplicate relations
                all_relations.sort_by(|a, b| {
                    (&a.from, &a.to, &a.relation_type).cmp(&(&b.from, &b.to, &b.relation_type))
                });
                all_relations.dedup_by(|a, b| {
                    a.from == b.from && a.to == b.to && a.relation_type == b.relation_type
                });

                ToolCallResponse::json(&SearchResult {
                    entities: entity_outputs,
                    relations: all_relations,
                })
            }
            Err(e) => ToolCallResponse::error(format!("Search failed: {}", e)),
        }
    }

    async fn create_entities(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            project_id: Option<String>,
            entities: Vec<EntityInput>,
        }

        #[derive(Deserialize)]
        struct EntityInput {
            name: String,
            #[serde(rename = "entityType")]
            entity_type: String,
            observations: Vec<String>,
            tags: Option<Vec<String>>,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project = match self.get_or_create_project(args.project_id.as_deref()).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(e),
        };

        let mut created = Vec::new();
        for input in args.entities {
            let mut entity =
                Entity::new(project.id.clone(), &input.name, input.entity_type.as_str());
            for obs in &input.observations {
                entity.add_observation(obs);
            }
            if let Some(tags) = input.tags {
                for tag in tags {
                    entity.add_tag(tag);
                }
            }

            if let Err(e) = self.storage.save_entity(&entity).await {
                return ToolCallResponse::error(format!(
                    "Failed to create entity {}: {}",
                    input.name, e
                ));
            }
            created.push(input.name);
        }

        ToolCallResponse::json(&serde_json::json!({
            "created": created,
            "count": created.len()
        }))
    }

    async fn add_observations(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            project_id: Option<String>,
            observations: Vec<ObservationInput>,
        }

        #[derive(Deserialize)]
        struct ObservationInput {
            #[serde(rename = "entityName")]
            entity_name: String,
            observations: Vec<String>,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project = match self.get_or_create_project(args.project_id.as_deref()).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(e),
        };

        let mut updated = Vec::new();
        for input in args.observations {
            let mut entity = match self
                .storage
                .get_entity(&input.entity_name, &project.id)
                .await
            {
                Ok(Some(e)) => e,
                Ok(None) => {
                    return ToolCallResponse::error(format!(
                        "Entity not found: {}",
                        input.entity_name
                    ))
                }
                Err(e) => return ToolCallResponse::error(format!("Error: {}", e)),
            };

            for obs in &input.observations {
                entity.add_observation(obs);
            }

            if let Err(e) = self.storage.save_entity(&entity).await {
                return ToolCallResponse::error(format!("Failed to update entity: {}", e));
            }
            updated.push(input.entity_name);
        }

        ToolCallResponse::json(&serde_json::json!({
            "updated": updated
        }))
    }

    async fn create_relations(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            project_id: Option<String>,
            relations: Vec<RelationInput>,
        }

        #[derive(Deserialize)]
        struct RelationInput {
            from: String,
            #[serde(rename = "fromProjectId")]
            from_project_id: Option<String>,
            to: String,
            #[serde(rename = "toProjectId")]
            to_project_id: Option<String>,
            #[serde(rename = "relationType")]
            relation_type: String,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project = match self.get_or_create_project(args.project_id.as_deref()).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(e),
        };

        let mut created = Vec::new();
        for input in args.relations {
            // Resolve from entity - try specified project, then current project, then global search
            let from_entity = match self
                .find_entity_for_relation(&input.from, input.from_project_id.as_deref(), &project)
                .await
            {
                Ok(e) => e,
                Err(msg) => return ToolCallResponse::error(msg),
            };

            // Resolve to entity - try specified project, then current project, then global search
            let to_entity = match self
                .find_entity_for_relation(&input.to, input.to_project_id.as_deref(), &project)
                .await
            {
                Ok(e) => e,
                Err(msg) => return ToolCallResponse::error(msg),
            };

            // Check if this is a cross-project relation
            let is_cross_project = from_entity.project_id != to_entity.project_id;

            // Create relation with real entity IDs
            let relation = if is_cross_project {
                Relation::new_cross_project(
                    project.id.clone(),
                    from_entity.id.clone(),
                    &from_entity.name,
                    from_entity.project_id.clone(),
                    to_entity.id.clone(),
                    &to_entity.name,
                    to_entity.project_id.clone(),
                    &input.relation_type,
                )
            } else {
                Relation::new(
                    project.id.clone(),
                    from_entity.id.clone(),
                    &from_entity.name,
                    to_entity.id.clone(),
                    &to_entity.name,
                    &input.relation_type,
                )
            };

            if let Err(e) = self.storage.save_relation(&relation).await {
                return ToolCallResponse::error(format!("Failed to create relation: {}", e));
            }
            created.push(format!(
                "{} -[{}]-> {}",
                input.from, input.relation_type, input.to
            ));
        }

        ToolCallResponse::json(&serde_json::json!({
            "created": created
        }))
    }

    /// Find an entity for relation creation
    /// Priority: specified project > current project > global search
    async fn find_entity_for_relation(
        &self,
        name: &str,
        project_id: Option<&str>,
        current_project: &Project,
    ) -> Result<Entity, String> {
        // If project specified, look up in that project
        if let Some(pid) = project_id {
            let proj = self.get_or_create_project(Some(pid)).await?;
            match self.storage.get_entity(name, &proj.id).await {
                Ok(Some(e)) => return Ok(e),
                Ok(None) => {
                    return Err(format!("Entity '{}' not found in project '{}'", name, pid))
                }
                Err(e) => return Err(format!("Storage error: {}", e)),
            }
        }

        // Try current project first
        if let Ok(Some(e)) = self.storage.get_entity(name, &current_project.id).await {
            return Ok(e);
        }

        // Search globally
        let all_entities = self
            .storage
            .get_all_entities_all_projects()
            .await
            .map_err(|e| format!("Storage error: {}", e))?;

        let matches: Vec<_> = all_entities
            .into_iter()
            .filter(|e| e.name == name)
            .collect();

        match matches.len() {
            0 => Err(format!("Entity not found: {}", name)),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let projects: Vec<_> = matches.iter().map(|e| e.project_id.to_string()).collect();
                Err(format!(
                    "Entity '{}' found in multiple projects: {}. Specify fromProjectId/toProjectId.",
                    name,
                    projects.join(", ")
                ))
            }
        }
    }

    async fn delete_entities(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            project_id: Option<String>,
            #[serde(rename = "entityNames")]
            entity_names: Vec<String>,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project = match self.get_or_create_project(args.project_id.as_deref()).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(e),
        };

        let mut deleted = Vec::new();
        for name in args.entity_names {
            // Delete relations first
            if let Err(e) = self
                .storage
                .delete_relations_for_entity(&name, &project.id)
                .await
            {
                tracing::warn!("Failed to delete relations for {}: {}", name, e);
            }

            if let Err(e) = self.storage.delete_entity(&name, &project.id).await {
                return ToolCallResponse::error(format!("Failed to delete {}: {}", name, e));
            }
            deleted.push(name);
        }

        ToolCallResponse::json(&serde_json::json!({
            "deleted": deleted
        }))
    }

    async fn delete_relations(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            project_id: Option<String>,
            relations: Vec<RelationInput>,
        }

        #[derive(Deserialize)]
        struct RelationInput {
            from: String,
            to: String,
            #[serde(rename = "relationType")]
            relation_type: String,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project = match self.get_or_create_project(args.project_id.as_deref()).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(e),
        };

        let mut deleted = Vec::new();
        for input in args.relations {
            if let Err(e) = self
                .storage
                .delete_relation(&input.from, &input.to, &input.relation_type, &project.id)
                .await
            {
                return ToolCallResponse::error(format!("Failed to delete relation: {}", e));
            }
            deleted.push(format!(
                "{} -[{}]-> {}",
                input.from, input.relation_type, input.to
            ));
        }

        ToolCallResponse::json(&serde_json::json!({
            "deleted": deleted
        }))
    }

    async fn read_graph(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            project_id: Option<String>,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project = match self.get_or_create_project(args.project_id.as_deref()).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(e),
        };

        let graph = match self.storage.load_graph(&project.id).await {
            Ok(g) => g,
            Err(e) => return ToolCallResponse::error(format!("Failed to load graph: {}", e)),
        };

        #[derive(Serialize)]
        struct GraphOutput {
            entities: Vec<EntityOutput>,
            relations: Vec<RelationOutput>,
        }

        #[derive(Serialize)]
        struct EntityOutput {
            name: String,
            #[serde(rename = "entityType")]
            entity_type: String,
            observations: Vec<String>,
            tags: Vec<String>,
        }

        #[derive(Serialize)]
        struct RelationOutput {
            from: String,
            to: String,
            #[serde(rename = "relationType")]
            relation_type: String,
        }

        let output = GraphOutput {
            entities: graph
                .entities
                .iter()
                .map(|e| EntityOutput {
                    name: e.name.clone(),
                    entity_type: e.entity_type.0.clone(),
                    observations: e.observations.iter().map(|o| o.content.clone()).collect(),
                    tags: e.tags.clone(),
                })
                .collect(),
            relations: graph
                .relations
                .iter()
                .map(|r| RelationOutput {
                    from: r.from_name.clone(),
                    to: r.to_name.clone(),
                    relation_type: r.relation_type.clone(),
                })
                .collect(),
        };

        ToolCallResponse::json(&output)
    }

    async fn open_nodes(&self, args: serde_json::Value) -> ToolCallResponse {
        #[derive(Deserialize)]
        struct Args {
            project_id: Option<String>,
            names: Vec<String>,
        }

        let args: Args = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResponse::error(format!("Invalid arguments: {}", e)),
        };

        let project = match self.get_or_create_project(args.project_id.as_deref()).await {
            Ok(p) => p,
            Err(e) => return ToolCallResponse::error(e),
        };

        #[derive(Serialize)]
        struct NodeOutput {
            name: String,
            #[serde(rename = "entityType")]
            entity_type: String,
            observations: Vec<String>,
            tags: Vec<String>,
            relations: Vec<RelationOutput>,
        }

        #[derive(Serialize)]
        struct RelationOutput {
            from: String,
            to: String,
            #[serde(rename = "relationType")]
            relation_type: String,
        }

        let mut nodes = Vec::new();
        for name in args.names {
            let entity = match self.storage.get_entity(&name, &project.id).await {
                Ok(Some(e)) => e,
                Ok(None) => continue,
                Err(e) => return ToolCallResponse::error(format!("Error: {}", e)),
            };

            let relations = match self
                .storage
                .get_relations_for_entity(&name, &project.id)
                .await
            {
                Ok(r) => r,
                Err(e) => return ToolCallResponse::error(format!("Error: {}", e)),
            };

            nodes.push(NodeOutput {
                name: entity.name.clone(),
                entity_type: entity.entity_type.0.clone(),
                observations: entity
                    .observations
                    .iter()
                    .map(|o| o.content.clone())
                    .collect(),
                tags: entity.tags.clone(),
                relations: relations
                    .iter()
                    .map(|r| RelationOutput {
                        from: r.from_name.clone(),
                        to: r.to_name.clone(),
                        relation_type: r.relation_type.clone(),
                    })
                    .collect(),
            });
        }

        ToolCallResponse::json(&serde_json::json!({
            "nodes": nodes
        }))
    }
}
