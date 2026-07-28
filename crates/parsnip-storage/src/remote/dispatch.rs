//! Server side of the storage RPC: decode a method call and run it against a local backend.
//!
//! Transport-agnostic on purpose. The HTTP envelope lives in `parsnip-mcp`; this module
//! only turns `(method, params)` into a backend call and a JSON result, which lets the
//! whole protocol be tested by looping a client straight into a dispatcher with no
//! sockets involved.

use std::sync::Arc;

use parsnip_core::limits::{validate_batch_entities, validate_batch_relations};
use parsnip_core::{Graph, Project};
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::protocol::{method, GraphPayload, WireError};
use super::protocol::{
    DeleteEntityParams, DeleteRelationParams, DeleteRelationsForEntityParams, EntitiesParams,
    EntityNameParams, GetEntityParams, GetRelationsForEntityParams, NameParams,
    ProjectIdOnlyParams, ProjectIdParams, RelationsParams, SaveEntityParams, SaveGraphParams,
    SaveProjectParams, SaveRelationParams,
};
use crate::traits::StorageBackend;

/// Dispatches storage RPC methods against a local backend.
///
/// Owns a lock used only by `get_or_create_project`, which is the one operation that has
/// to be atomic across concurrent clients (see [`Self::get_or_create_project`]).
pub struct StorageDispatcher<S: StorageBackend + ?Sized> {
    backend: Arc<S>,
    project_create_lock: tokio::sync::Mutex<()>,
}

impl<S: StorageBackend + ?Sized> StorageDispatcher<S> {
    pub fn new(backend: Arc<S>) -> Self {
        Self {
            backend,
            project_create_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// True if this method belongs to the storage RPC surface.
    pub fn handles(method: &str) -> bool {
        method.starts_with("storage/")
    }

    /// Resolve a project by name, creating it if absent, without racing.
    ///
    /// The CLI resolves project names with a get-then-create that lives in three command
    /// modules. Run concurrently by two clients against one daemon, both observe `None`,
    /// both mint a different `ProjectId`, and both save. The last write wins the name
    /// while the loser's entities stay keyed under an id no name resolves to, so they
    /// silently disappear. Holding a lock across the pair closes that window.
    async fn get_or_create_project(&self, name: &str) -> Result<Project, WireError> {
        let _guard = self.project_create_lock.lock().await;

        if let Some(existing) = self.backend.get_project(name).await.map_err(wire)? {
            return Ok(existing);
        }

        let project = Project::new(name);
        self.backend.save_project(&project).await.map_err(wire)?;
        tracing::debug!(project = name, id = %project.id, "created project via storage RPC");
        Ok(project)
    }

    /// Run one storage RPC call.
    pub async fn dispatch(&self, method: &str, params: Value) -> Result<Value, WireError> {
        let b = &self.backend;

        match method {
            method::HEALTH_CHECK => ok(b.health_check().await.map_err(wire)?),

            // ── Entities ────────────────────────────────────────────────────────────
            method::SAVE_ENTITY => {
                let p: SaveEntityParams = parse(params)?;
                b.save_entity(&p.entity).await.map_err(wire)?;
                ok(())
            }
            method::GET_ENTITY => {
                let p: GetEntityParams = parse(params)?;
                ok(b.get_entity(&p.name, &p.project_id).await.map_err(wire)?)
            }
            method::GET_ALL_ENTITIES => {
                let p: ProjectIdParams = parse(params)?;
                ok(b.get_all_entities(&p.project_id).await.map_err(wire)?)
            }
            method::GET_ALL_ENTITIES_ALL_PROJECTS => {
                ok(b.get_all_entities_all_projects().await.map_err(wire)?)
            }
            method::DELETE_ENTITY => {
                let p: DeleteEntityParams = parse(params)?;
                b.delete_entity(&p.name, &p.project_id)
                    .await
                    .map_err(wire)?;
                ok(())
            }

            // ── Relations ───────────────────────────────────────────────────────────
            method::SAVE_RELATION => {
                let p: SaveRelationParams = parse(params)?;
                b.save_relation(&p.relation).await.map_err(wire)?;
                ok(())
            }
            method::GET_RELATIONS_FOR_ENTITY => {
                let p: GetRelationsForEntityParams = parse(params)?;
                ok(b.get_relations_for_entity(&p.entity_name, &p.project_id)
                    .await
                    .map_err(wire)?)
            }
            method::GET_ALL_RELATIONS => {
                let p: ProjectIdParams = parse(params)?;
                ok(b.get_all_relations(&p.project_id).await.map_err(wire)?)
            }
            method::GET_ALL_RELATIONS_ALL_PROJECTS => {
                ok(b.get_all_relations_all_projects().await.map_err(wire)?)
            }
            method::GET_RELATIONS_FOR_ENTITY_GLOBAL => {
                let p: EntityNameParams = parse(params)?;
                ok(b.get_relations_for_entity_global(&p.entity_name)
                    .await
                    .map_err(wire)?)
            }
            method::DELETE_RELATION => {
                let p: DeleteRelationParams = parse(params)?;
                b.delete_relation(&p.from, &p.to, &p.relation_type, &p.project_id)
                    .await
                    .map_err(wire)?;
                ok(())
            }
            method::DELETE_RELATIONS_FOR_ENTITY => {
                let p: DeleteRelationsForEntityParams = parse(params)?;
                b.delete_relations_for_entity(&p.entity_name, &p.project_id)
                    .await
                    .map_err(wire)?;
                ok(())
            }

            // ── Projects ────────────────────────────────────────────────────────────
            method::SAVE_PROJECT => {
                let p: SaveProjectParams = parse(params)?;
                b.save_project(&p.project).await.map_err(wire)?;
                ok(())
            }
            method::GET_PROJECT => {
                let p: NameParams = parse(params)?;
                ok(b.get_project(&p.name).await.map_err(wire)?)
            }
            method::GET_PROJECT_BY_ID => {
                let p: ProjectIdOnlyParams = parse(params)?;
                ok(b.get_project_by_id(&p.id).await.map_err(wire)?)
            }
            method::GET_ALL_PROJECTS => ok(b.get_all_projects().await.map_err(wire)?),
            method::DELETE_PROJECT => {
                let p: NameParams = parse(params)?;
                b.delete_project(&p.name).await.map_err(wire)?;
                ok(())
            }
            method::GET_OR_CREATE_PROJECT => {
                let p: NameParams = parse(params)?;
                ok(self.get_or_create_project(&p.name).await?)
            }

            // ── Bulk ────────────────────────────────────────────────────────────────
            method::LOAD_GRAPH => {
                let p: ProjectIdParams = parse(params)?;
                let graph = b.load_graph(&p.project_id).await.map_err(wire)?;
                ok(GraphPayload {
                    entities: graph.entities,
                    relations: graph.relations,
                })
            }
            method::SAVE_GRAPH => {
                let p: SaveGraphParams = parse(params)?;
                // Bound per-call work; see the note on batch limits below.
                validate_batch(p.graph.entities.len(), p.graph.relations.len())?;
                let graph = Graph {
                    entities: p.graph.entities,
                    relations: p.graph.relations,
                };
                b.save_graph(&graph, &p.project_id).await.map_err(wire)?;
                ok(())
            }
            method::SAVE_ENTITIES_BATCH => {
                let p: EntitiesParams = parse(params)?;
                validate_batch(p.entities.len(), 0)?;
                b.save_entities_batch(&p.entities).await.map_err(wire)?;
                ok(())
            }
            method::SAVE_RELATIONS_BATCH => {
                let p: RelationsParams = parse(params)?;
                validate_batch(0, p.relations.len())?;
                b.save_relations_batch(&p.relations).await.map_err(wire)?;
                ok(())
            }

            // `initialize` and `close` are deliberately not exposed: forwarding `close`
            // would let any client drop the daemon's database handle, and neither has a
            // caller. Clients treat both as local no-ops.
            other => Err(WireError::new(
                "MethodNotFound",
                format!("Unknown storage method: {}", other),
            )),
        }
    }
}

/// Enforce the batch-size limits.
///
/// Deliberately narrower than the MCP tool layer, which also checks entity-name,
/// observation and tag lengths. Those are content rules the local CLI path does not
/// apply either (`ctx.storage.save_entity` is called directly), so enforcing them only
/// here would make remote mode reject data local mode accepts, and split the two
/// behaviours. Batch sizes are different: they bound per-request memory on a shared
/// daemon, so they belong on the server regardless.
fn validate_batch(entities: usize, relations: usize) -> Result<(), WireError> {
    if entities > 0 {
        validate_batch_entities(entities)
            .map_err(|e| WireError::new("InvalidParams", e.to_string()))?;
    }
    if relations > 0 {
        validate_batch_relations(relations)
            .map_err(|e| WireError::new("InvalidParams", e.to_string()))?;
    }
    Ok(())
}

fn parse<T: DeserializeOwned>(params: Value) -> Result<T, WireError> {
    serde_json::from_value(params)
        .map_err(|e| WireError::new("InvalidParams", format!("Invalid parameters: {}", e)))
}

fn ok<T: serde::Serialize>(value: T) -> Result<Value, WireError> {
    serde_json::to_value(value)
        .map_err(|e| WireError::new("Serialization", format!("Failed to encode result: {}", e)))
}

fn wire(e: crate::error::StorageError) -> WireError {
    WireError::from(&e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryStorage;

    fn dispatcher() -> StorageDispatcher<MemoryStorage> {
        StorageDispatcher::new(Arc::new(MemoryStorage::new()))
    }

    #[test]
    fn handles_only_storage_methods() {
        assert!(StorageDispatcher::<MemoryStorage>::handles(
            method::GET_ENTITY
        ));
        assert!(!StorageDispatcher::<MemoryStorage>::handles("tools/call"));
        assert!(!StorageDispatcher::<MemoryStorage>::handles("initialize"));
    }

    #[tokio::test]
    async fn unknown_method_is_reported_not_panicked() {
        let d = dispatcher();
        let err = d
            .dispatch("storage/nope", Value::Null)
            .await
            .expect_err("unknown method must error");
        assert_eq!(err.kind, "MethodNotFound");
    }

    #[tokio::test]
    async fn missing_entity_is_null_not_an_error() {
        let d = dispatcher();
        let project = d.get_or_create_project("default").await.unwrap();
        let result = d
            .dispatch(
                method::GET_ENTITY,
                serde_json::json!({"name": "absent", "project_id": project.id}),
            )
            .await
            .expect("get_entity of a missing entity should succeed");
        assert!(result.is_null(), "expected null, got {result}");
    }

    #[tokio::test]
    async fn get_or_create_project_is_idempotent() {
        let d = dispatcher();
        let first = d.get_or_create_project("shared").await.unwrap();
        let second = d.get_or_create_project("shared").await.unwrap();
        assert_eq!(
            first.id, second.id,
            "second call must reuse the existing project id"
        );
    }

    #[tokio::test]
    async fn concurrent_get_or_create_yields_one_project_id() {
        let d = Arc::new(dispatcher());

        let mut handles = Vec::new();
        for _ in 0..8 {
            let d = d.clone();
            handles.push(tokio::spawn(async move {
                d.get_or_create_project("racy").await.unwrap().id
            }));
        }

        let mut ids = Vec::new();
        for h in handles {
            ids.push(h.await.unwrap());
        }

        let first = &ids[0];
        assert!(
            ids.iter().all(|id| id == first),
            "concurrent callers minted different project ids: {ids:?}"
        );
    }

    #[tokio::test]
    async fn oversized_batch_is_rejected() {
        let d = dispatcher();
        let entities: Vec<parsnip_core::Entity> = (0..500)
            .map(|i| {
                parsnip_core::Entity::new(
                    parsnip_core::ProjectId::new(),
                    format!("e{i}"),
                    "test".to_string(),
                )
            })
            .collect();

        let err = d
            .dispatch(
                method::SAVE_ENTITIES_BATCH,
                serde_json::json!({ "entities": entities }),
            )
            .await
            .expect_err("batch over the limit must be rejected");
        assert_eq!(err.kind, "InvalidParams");
    }

    #[tokio::test]
    async fn bad_params_report_invalid_params() {
        let d = dispatcher();
        let err = d
            .dispatch(method::GET_ENTITY, serde_json::json!({"name": "x"}))
            .await
            .expect_err("missing project_id must error");
        assert_eq!(err.kind, "InvalidParams");
    }
}
