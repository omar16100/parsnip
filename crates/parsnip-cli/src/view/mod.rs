//! View models and rendering.
//!
//! Command handlers build a view and hand it to [`emit`], instead of printing as they go.
//! That is what makes `--format json` and `--format csv` possible: previously every handler
//! wrote `println!` inline, so the flag was parsed and then ignored everywhere.
//!
//! It also keeps the local and remote paths honest. Both build the same view type, so a
//! field the remote path fails to populate shows up as a rendering difference rather than
//! silently disappearing.

pub mod render;

use serde::Serialize;

#[allow(unused_imports)]
pub use render::{emit, OutputFormat, Render};

/// One entity in a list.
#[derive(Debug, Serialize)]
pub struct EntityRow {
    pub name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub tags: Vec<String>,
}

/// One observation.
#[derive(Debug, Serialize)]
pub struct ObservationRow {
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// `entity list`
#[derive(Debug, Serialize)]
pub struct EntityListView {
    pub project: String,
    pub entities: Vec<EntityRow>,
}

/// `entity get`
#[derive(Debug, Serialize)]
pub struct EntityDetailView {
    pub name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub project: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub tags: Vec<String>,
    pub observations: Vec<ObservationRow>,
}

/// One relation in a list.
#[derive(Debug, Serialize)]
pub struct RelationRow {
    pub from: String,
    pub to: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

/// `relation list`
#[derive(Debug, Serialize)]
pub struct RelationListView {
    pub project: String,
    pub relations: Vec<RelationRow>,
}

/// `relation traverse`
#[derive(Debug, Serialize)]
pub struct TraversalView {
    pub start: String,
    pub depth: u32,
    pub direction: String,
    #[serde(rename = "nodesVisited")]
    pub nodes_visited: usize,
    #[serde(rename = "edgesTraversed")]
    pub edges_traversed: usize,
    pub entities: Vec<String>,
    pub relations: Vec<RelationRow>,
}

/// One path between two entities.
#[derive(Debug, Serialize)]
pub struct PathView {
    pub nodes: Vec<String>,
    pub edges: Vec<RelationRow>,
    #[serde(rename = "totalWeight")]
    pub total_weight: f64,
    pub length: usize,
}

/// `relation find-path`
#[derive(Debug, Serialize)]
pub struct FindPathView {
    pub from: String,
    pub to: String,
    /// "Dijkstra" when weighted, "BFS" otherwise.
    pub algorithm: &'static str,
    #[serde(rename = "nodesVisited")]
    pub nodes_visited: usize,
    #[serde(rename = "edgesTraversed")]
    pub edges_traversed: usize,
    pub paths: Vec<PathView>,
}

/// One project in a list.
///
/// No entity or relation counts: producing them would mean a query per project, which
/// `project list` has never done.
#[derive(Debug, Serialize)]
pub struct ProjectRow {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// True for the project this invocation is scoped to.
    pub current: bool,
}

/// `project list`
#[derive(Debug, Serialize)]
pub struct ProjectListView {
    pub projects: Vec<ProjectRow>,
}

/// A `type: count` breakdown line.
#[derive(Debug, Serialize)]
pub struct CountRow {
    pub name: String,
    pub count: usize,
}

/// `project stats`
#[derive(Debug, Serialize)]
pub struct ProjectStatsView {
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "entityCount")]
    pub entity_count: usize,
    #[serde(rename = "entitiesByType")]
    pub entities_by_type: Vec<CountRow>,
    #[serde(rename = "observationCount")]
    pub observation_count: usize,
    #[serde(rename = "tagCount")]
    pub tag_count: usize,
    #[serde(rename = "relationCount")]
    pub relation_count: usize,
    #[serde(rename = "relationsByType")]
    pub relations_by_type: Vec<CountRow>,
}

/// One search hit, with the relations touching it when `--include-relations` is set.
#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<SearchHitRelation>,
}

/// A relation shown under a search hit, with the direction it points relative to the hit.
#[derive(Debug, Serialize)]
pub struct SearchHitRelation {
    pub direction: &'static str,
    pub other: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
}

/// `search`
#[derive(Debug, Serialize)]
pub struct SearchView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub scope: String,
    pub results: Vec<SearchHit>,
}

/// The result of a write: created, deleted, updated.
///
/// Deliberately not list-shaped, so it renders as JSON but not CSV.
#[derive(Debug, Serialize)]
pub struct MutationView {
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

impl MutationView {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn with_details(message: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            message: message.into(),
            details,
        }
    }
}
