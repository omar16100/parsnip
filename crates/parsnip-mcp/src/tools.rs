//! MCP tool definitions

use serde::Serialize;

/// MCP tool definition
#[derive(Debug, Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// Get all available tools
pub fn get_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "search_knowledge",
            description: "Search entities by text or tags across projects. Omit projectId to search all projects.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search text"},
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default'). Omit to search all projects."},
                    "searchMode": {"type": "string", "enum": ["exact", "fuzzy"], "default": "exact"},
                    "fuzzyThreshold": {"type": "number", "description": "Fuzzy threshold (0.0-1.0)", "default": 0.3},
                    "exactTags": {"type": "array", "items": {"type": "string"}, "description": "Tags for exact-match filtering"},
                    "page": {"type": "number", "description": "Page number (0-indexed)"},
                    "pageSize": {"type": "number", "description": "Results per page (default: 100, max: 1000)"}
                }
            }),
        },
        Tool {
            name: "create_entities",
            description: "Create new entities with observations and optional tags. Use a single call for multiple entities.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["entities"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "entities": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["name", "entityType", "observations"],
                            "properties": {
                                "name": {"type": "string", "description": "Unique entity name"},
                                "entityType": {"type": "string", "description": "Entity type (person, technology, project, company, concept, event, preference)"},
                                "observations": {"type": "array", "items": {"type": "string"}, "description": "Factual statements about the entity"},
                                "tags": {"type": "array", "items": {"type": "string"}, "description": "Optional tags for categorization"}
                            }
                        }
                    }
                }
            }),
        },
        Tool {
            name: "add_observations",
            description: "Add new observations to existing entities.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["observations"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "observations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["entityName", "observations"],
                            "properties": {
                                "entityName": {"type": "string", "description": "Exact name of existing entity"},
                                "observations": {"type": "array", "items": {"type": "string"}, "description": "New factual statements to add"}
                            }
                        }
                    }
                }
            }),
        },
        Tool {
            name: "create_relations",
            description: "Create directional relationships between entities. Supports cross-project relations by specifying fromProjectId/toProjectId.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["relations"],
                "properties": {
                    "projectId": {"type": "string", "description": "Default project for entities (default: 'default')"},
                    "relations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["from", "to", "relationType"],
                            "properties": {
                                "from": {"type": "string", "description": "Source entity name"},
                                "fromProjectId": {"type": "string", "description": "Project containing the source entity (optional, auto-detected if unique)"},
                                "to": {"type": "string", "description": "Target entity name"},
                                "toProjectId": {"type": "string", "description": "Project containing the target entity (optional, auto-detected if unique)"},
                                "relationType": {"type": "string", "description": "Relationship type (e.g., works_at, manages, depends_on)"}
                            }
                        }
                    }
                }
            }),
        },
        Tool {
            name: "delete_entities",
            description: "Permanently delete entities and all their relationships.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["entityNames"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "entityNames": {"type": "array", "items": {"type": "string"}, "description": "Entity names to delete"}
                }
            }),
        },
        Tool {
            name: "delete_observations",
            description: "Delete specific observations from entities while preserving the entity.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["deletions"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "deletions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["entityName", "observations"],
                            "properties": {
                                "entityName": {"type": "string"},
                                "observations": {"type": "array", "items": {"type": "string"}, "description": "Exact observation strings to remove"}
                            }
                        }
                    }
                }
            }),
        },
        Tool {
            name: "delete_relations",
            description: "Delete specific relationships between entities.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["relations"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "relations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["from", "to", "relationType"],
                            "properties": {
                                "from": {"type": "string"},
                                "to": {"type": "string"},
                                "relationType": {"type": "string"}
                            }
                        }
                    }
                }
            }),
        },
        Tool {
            name: "read_graph",
            description: "Retrieve the complete knowledge graph for a project.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "projectId": {"type": "string", "description": "Project identifier (default: 'default')"}
                }
            }),
        },
        Tool {
            name: "open_nodes",
            description: "Retrieve specific entities by exact names along with their relationships.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["names"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "names": {"type": "array", "items": {"type": "string"}, "description": "Exact entity names to retrieve"}
                }
            }),
        },
        Tool {
            name: "add_tags",
            description: "Add categorical tags to existing entities for filtering and organization.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["updates"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "updates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["entityName", "tags"],
                            "properties": {
                                "entityName": {"type": "string"},
                                "tags": {"type": "array", "items": {"type": "string"}}
                            }
                        }
                    }
                }
            }),
        },
        Tool {
            name: "remove_tags",
            description: "Remove specific tags from entities.",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["updates"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "updates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["entityName", "tags"],
                            "properties": {
                                "entityName": {"type": "string"},
                                "tags": {"type": "array", "items": {"type": "string"}}
                            }
                        }
                    }
                }
            }),
        },
        Tool {
            name: "traverse_graph",
            description: "Traverse the knowledge graph from a starting entity. Supports path finding between entities, filtered traversal by entity/relation types, and weighted shortest path (Dijkstra).",
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["start"],
                "properties": {
                    "projectId": {"type": "string", "description": "Project name for data isolation (default: 'default')"},
                    "start": {"type": "string", "description": "Starting entity name"},
                    "target": {"type": "string", "description": "Target entity name for path finding (optional)"},
                    "maxDepth": {"type": "number", "description": "Maximum traversal depth (default: 10)", "default": 10},
                    "direction": {"type": "string", "enum": ["outgoing", "incoming", "both"], "description": "Traversal direction (default: 'both')", "default": "both"},
                    "entityTypeFilter": {"type": "array", "items": {"type": "string"}, "description": "Filter traversal to these entity types only"},
                    "relationTypeFilter": {"type": "array", "items": {"type": "string"}, "description": "Filter traversal to these relation types only"},
                    "useWeights": {"type": "boolean", "description": "Use weighted shortest path (Dijkstra) when finding paths", "default": false}
                }
            }),
        },
        Tool {
            name: "list_projects",
            description: "List all projects with entity and relation counts.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}
