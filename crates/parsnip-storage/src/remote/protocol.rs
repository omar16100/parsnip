//! Wire protocol for remoting [`StorageBackend`](crate::traits::StorageBackend).
//!
//! Every method of the trait maps to one JSON-RPC method named `storage/<trait_method>`,
//! and every parameter struct field is named exactly like the corresponding trait
//! parameter. That correspondence is deliberate: it makes the dispatch table in
//! [`super::dispatch`] mechanically checkable against `traits.rs` by eye.
//!
//! Encoding and decoding live in the same directory as the dispatcher on purpose, so a
//! change to one cannot silently drift from the other.

use parsnip_core::{Entity, Project, ProjectId, Relation};
use serde::{Deserialize, Serialize};

use crate::error::StorageError;

/// Method names. `storage/` prefixed so they cannot collide with the MCP `tools/*`
/// surface sharing the same endpoint.
pub mod method {
    pub const HEALTH_CHECK: &str = "storage/health_check";

    pub const SAVE_ENTITY: &str = "storage/save_entity";
    pub const GET_ENTITY: &str = "storage/get_entity";
    pub const GET_ALL_ENTITIES: &str = "storage/get_all_entities";
    pub const GET_ALL_ENTITIES_ALL_PROJECTS: &str = "storage/get_all_entities_all_projects";
    pub const DELETE_ENTITY: &str = "storage/delete_entity";

    pub const SAVE_RELATION: &str = "storage/save_relation";
    pub const GET_RELATIONS_FOR_ENTITY: &str = "storage/get_relations_for_entity";
    pub const GET_ALL_RELATIONS: &str = "storage/get_all_relations";
    pub const GET_ALL_RELATIONS_ALL_PROJECTS: &str = "storage/get_all_relations_all_projects";
    pub const GET_RELATIONS_FOR_ENTITY_GLOBAL: &str = "storage/get_relations_for_entity_global";
    pub const DELETE_RELATION: &str = "storage/delete_relation";
    pub const DELETE_RELATIONS_FOR_ENTITY: &str = "storage/delete_relations_for_entity";

    pub const SAVE_PROJECT: &str = "storage/save_project";
    pub const GET_PROJECT: &str = "storage/get_project";
    pub const GET_PROJECT_BY_ID: &str = "storage/get_project_by_id";
    pub const GET_ALL_PROJECTS: &str = "storage/get_all_projects";
    pub const DELETE_PROJECT: &str = "storage/delete_project";

    pub const LOAD_GRAPH: &str = "storage/load_graph";
    pub const SAVE_GRAPH: &str = "storage/save_graph";
    pub const SAVE_ENTITIES_BATCH: &str = "storage/save_entities_batch";
    pub const SAVE_RELATIONS_BATCH: &str = "storage/save_relations_batch";

    /// Not a trait method. Makes the CLI's get-then-create project resolution atomic
    /// on the server instead of racing between clients.
    pub const GET_OR_CREATE_PROJECT: &str = "storage/get_or_create_project";
}

/// JSON-RPC error code used for storage-level failures.
pub const STORAGE_ERROR_CODE: i32 = -32001;

/// A [`StorageError`] rendered for the wire.
///
/// `StorageError` cannot be serialized directly: it wraps `std::io::Error`,
/// `serde_json::Error` and six redb types. `kind` is a stable string the client maps
/// back to a variant; `detail` is the `Display` text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireError {
    pub kind: String,
    pub detail: String,
}

impl WireError {
    pub fn new(kind: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl From<&StorageError> for WireError {
    fn from(e: &StorageError) -> Self {
        // `detail` is the variant's *payload*, not its Display output. Using Display here
        // would re-apply the "Database error: " prefix when the client rebuilds the
        // variant, producing "Database error: Database error: boom".
        //
        // Backend-specific variants collapse to "Backend": their concrete types are not
        // reconstructable client-side and no caller matches on them, so they keep the
        // full Display text, which the client surfaces via `Remote`.
        match e {
            StorageError::Database(m) => Self::new("Database", m),
            StorageError::EntityNotFound(m) => Self::new("EntityNotFound", m),
            StorageError::ProjectNotFound(m) => Self::new("ProjectNotFound", m),
            StorageError::DuplicateEntity(m) => Self::new("DuplicateEntity", m),
            StorageError::DuplicateProject(m) => Self::new("DuplicateProject", m),
            StorageError::Migration(m) => Self::new("Migration", m),
            StorageError::Connection(m) => Self::new("Connection", m),
            StorageError::Transaction(m) => Self::new("Transaction", m),
            StorageError::Remote(m) => Self::new("Remote", m),
            StorageError::Serialization(_) => Self::new("Serialization", e.to_string()),
            StorageError::Io(_) => Self::new("Io", e.to_string()),
            #[allow(unreachable_patterns)]
            _ => Self::new("Backend", e.to_string()),
        }
    }
}

impl From<WireError> for StorageError {
    fn from(w: WireError) -> Self {
        // Only variants carrying a plain String can be reconstructed faithfully. The rest
        // become Remote, which keeps the server's message.
        match w.kind.as_str() {
            "Database" => StorageError::Database(w.detail),
            "EntityNotFound" => StorageError::EntityNotFound(w.detail),
            "ProjectNotFound" => StorageError::ProjectNotFound(w.detail),
            "DuplicateEntity" => StorageError::DuplicateEntity(w.detail),
            "DuplicateProject" => StorageError::DuplicateProject(w.detail),
            "Migration" => StorageError::Migration(w.detail),
            "Connection" => StorageError::Connection(w.detail),
            "Transaction" => StorageError::Transaction(w.detail),
            _ => StorageError::Remote(w.detail),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter types. Field names mirror the trait's parameter names exactly.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveEntityParams {
    pub entity: Entity,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetEntityParams {
    pub name: String,
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectIdParams {
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteEntityParams {
    pub name: String,
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveRelationParams {
    pub relation: Relation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRelationsForEntityParams {
    pub entity_name: String,
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityNameParams {
    pub entity_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRelationParams {
    pub from: String,
    pub to: String,
    pub relation_type: String,
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRelationsForEntityParams {
    pub entity_name: String,
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveProjectParams {
    pub project: Project,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NameParams {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectIdOnlyParams {
    pub id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntitiesParams {
    pub entities: Vec<Entity>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelationsParams {
    pub relations: Vec<Relation>,
}

/// `parsnip_core::Graph` derives neither `Serialize` nor `Deserialize`, so graph
/// transfers use this explicit envelope rather than adding derives to a core type.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GraphPayload {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveGraphParams {
    pub project_id: ProjectId,
    #[serde(flatten)]
    pub graph: GraphPayload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_error_roundtrips_known_kinds() {
        let cases = vec![
            StorageError::Database("boom".into()),
            StorageError::EntityNotFound("thing".into()),
            StorageError::ProjectNotFound("proj".into()),
            StorageError::DuplicateEntity("dup".into()),
            StorageError::DuplicateProject("dup".into()),
            StorageError::Migration("mig".into()),
            StorageError::Connection("conn".into()),
            StorageError::Transaction("txn".into()),
        ];

        for original in cases {
            let wire = WireError::from(&original);
            let restored = StorageError::from(wire.clone());
            assert_eq!(
                std::mem::discriminant(&original),
                std::mem::discriminant(&restored),
                "kind {} did not round-trip to the same variant",
                wire.kind
            );
            assert_eq!(original.to_string(), restored.to_string());
        }
    }

    #[test]
    fn unmappable_kinds_become_remote() {
        let wire = WireError::new("Backend", "redb exploded");
        let restored = StorageError::from(wire);
        assert!(matches!(restored, StorageError::Remote(m) if m == "redb exploded"));
    }

    #[test]
    fn save_graph_params_flattens_graph_fields() {
        let params = SaveGraphParams {
            project_id: ProjectId::new(),
            graph: GraphPayload::default(),
        };
        let json = serde_json::to_value(&params).unwrap();
        // Flattened, so entities/relations sit next to project_id rather than nested.
        assert!(json.get("entities").is_some());
        assert!(json.get("relations").is_some());
        assert!(json.get("project_id").is_some());
    }
}
