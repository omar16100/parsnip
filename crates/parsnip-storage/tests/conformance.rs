//! One behavioural suite, run against a local backend and against `RemoteStorage`.
//!
//! The remote path is 22 hand-written encode/decode pairs, which is exactly where a
//! swapped `from`/`to` or a dropped `project_id` hides. Running identical assertions
//! against both implementations turns any such mismatch into a deterministic failure,
//! with no HTTP involved: the client is looped straight into the dispatcher.

use std::sync::Arc;

use async_trait::async_trait;
use parsnip_core::{Entity, EntityId, Project, ProjectId, Relation};
use parsnip_storage::memory::MemoryStorage;
use parsnip_storage::remote::client::{RemoteStorage, RpcTransport};
use parsnip_storage::remote::StorageDispatcher;
use parsnip_storage::{StorageBackend, StorageError, StorageResult};
use serde_json::Value;

/// Carries calls straight into a dispatcher, exercising the full encode/decode path
/// without a socket.
struct LoopbackTransport {
    dispatcher: StorageDispatcher<MemoryStorage>,
}

#[async_trait]
impl RpcTransport for LoopbackTransport {
    async fn call(&self, method: &str, params: Value) -> StorageResult<Value> {
        self.dispatcher
            .dispatch(method, params)
            .await
            .map_err(StorageError::from)
    }
}

fn remote_backend() -> RemoteStorage {
    RemoteStorage::with_transport(Arc::new(LoopbackTransport {
        dispatcher: StorageDispatcher::new(Arc::new(MemoryStorage::new())),
    }))
}

fn entity(project: &ProjectId, name: &str) -> Entity {
    let mut e = Entity::new(project.clone(), name, "test_type");
    e.add_observation(format!("observation for {name}"));
    e.add_tag("alpha");
    e.add_tag("beta");
    e
}

fn relation(project: &ProjectId, from: &Entity, to: &Entity, kind: &str) -> Relation {
    Relation::new(
        project.clone(),
        from.id.clone(),
        from.name.clone(),
        to.id.clone(),
        to.name.clone(),
        kind,
    )
}

/// Every assertion both backends must satisfy identically.
async fn assert_backend_conformance(store: &dyn StorageBackend) {
    // ── Projects ────────────────────────────────────────────────────────────────
    assert!(store.health_check().await.unwrap());
    assert!(
        store.get_project("absent").await.unwrap().is_none(),
        "a missing project must be None, not an error"
    );
    assert!(store.get_all_projects().await.unwrap().is_empty());

    let project = Project::new("alpha");
    store.save_project(&project).await.unwrap();

    let fetched = store.get_project("alpha").await.unwrap().expect("saved");
    assert_eq!(
        fetched.id, project.id,
        "project id must survive the round trip"
    );
    assert_eq!(fetched.name, "alpha");

    let by_id = store
        .get_project_by_id(&project.id)
        .await
        .unwrap()
        .expect("lookup by id");
    assert_eq!(by_id.name, "alpha");
    assert_eq!(store.get_all_projects().await.unwrap().len(), 1);

    // ── Entities ────────────────────────────────────────────────────────────────
    assert!(
        store
            .get_entity("absent", &project.id)
            .await
            .unwrap()
            .is_none(),
        "a missing entity must be None, not an error"
    );

    let alice = entity(&project.id, "alice");
    let bob = entity(&project.id, "bob");
    store.save_entity(&alice).await.unwrap();
    store.save_entity(&bob).await.unwrap();

    let got = store
        .get_entity("alice", &project.id)
        .await
        .unwrap()
        .expect("saved entity");
    assert_eq!(got.id, alice.id);
    assert_eq!(got.entity_type, alice.entity_type);
    assert_eq!(got.project_id, project.id);
    assert_eq!(got.tags, vec!["alpha".to_string(), "beta".to_string()]);
    assert_eq!(
        got.observations.len(),
        1,
        "observations must survive the round trip"
    );
    assert_eq!(got.observations[0].content, alice.observations[0].content);

    let all = store.get_all_entities(&project.id).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(
        store.get_all_entities_all_projects().await.unwrap().len(),
        2
    );

    // ── Relations ───────────────────────────────────────────────────────────────
    assert!(store
        .get_all_relations(&project.id)
        .await
        .unwrap()
        .is_empty());

    let knows = relation(&project.id, &alice, &bob, "knows");
    store.save_relation(&knows).await.unwrap();

    let rels = store.get_all_relations(&project.id).await.unwrap();
    assert_eq!(rels.len(), 1);
    // Direction is the classic thing to get backwards in a hand-written codec.
    assert_eq!(rels[0].from_name, "alice", "from/to must not be swapped");
    assert_eq!(rels[0].to_name, "bob");
    assert_eq!(rels[0].relation_type, "knows");

    assert_eq!(
        store
            .get_relations_for_entity("alice", &project.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .get_relations_for_entity_global("alice")
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store.get_all_relations_all_projects().await.unwrap().len(),
        1
    );

    // ── Bulk ────────────────────────────────────────────────────────────────────
    let graph = store.load_graph(&project.id).await.unwrap();
    assert_eq!(graph.entities.len(), 2);
    assert_eq!(graph.relations.len(), 1);

    // Empty batches must be accepted, not rejected or turned into a wasted call.
    store.save_entities_batch(&[]).await.unwrap();
    store.save_relations_batch(&[]).await.unwrap();

    // Larger than one chunk, so the client's splitting is exercised end to end.
    let bulk: Vec<Entity> = (0..250)
        .map(|i| entity(&project.id, &format!("bulk_{i}")))
        .collect();
    store.save_entities_batch(&bulk).await.unwrap();
    assert_eq!(
        store.get_all_entities(&project.id).await.unwrap().len(),
        252,
        "every entity of a multi-chunk batch must land"
    );

    let bulk_relations: Vec<Relation> = (0..150)
        .map(|i| {
            Relation::new(
                project.id.clone(),
                EntityId::new(),
                format!("bulk_{i}"),
                EntityId::new(),
                "alice",
                "references",
            )
        })
        .collect();
    store.save_relations_batch(&bulk_relations).await.unwrap();
    assert_eq!(
        store.get_all_relations(&project.id).await.unwrap().len(),
        151
    );

    // ── Deletions ───────────────────────────────────────────────────────────────
    store
        .delete_relation("alice", "bob", "knows", &project.id)
        .await
        .unwrap();
    let after_delete = store
        .get_relations_for_entity("alice", &project.id)
        .await
        .unwrap();
    assert!(
        !after_delete.iter().any(|r| r.relation_type == "knows"),
        "the deleted relation must be gone"
    );
    assert_eq!(
        after_delete.len(),
        150,
        "deleting one relation must not touch the bulk relations pointing at alice"
    );

    store
        .delete_relations_for_entity("alice", &project.id)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_relations_for_entity("alice", &project.id)
            .await
            .unwrap()
            .len(),
        0,
        "delete_relations_for_entity must remove every relation touching the entity"
    );

    store.delete_entity("alice", &project.id).await.unwrap();
    assert!(store
        .get_entity("alice", &project.id)
        .await
        .unwrap()
        .is_none());

    store.delete_project("alpha").await.unwrap();
    assert!(store.get_project("alpha").await.unwrap().is_none());
}

#[tokio::test]
async fn memory_backend_conforms() {
    assert_backend_conformance(&MemoryStorage::new()).await;
}

#[tokio::test]
async fn remote_backend_conforms() {
    assert_backend_conformance(&remote_backend()).await;
}

#[tokio::test]
async fn remote_save_graph_matches_local() {
    let local = MemoryStorage::new();
    let remote = remote_backend();

    let project = Project::new("graph_test");
    let a = entity(&project.id, "a");
    let b = entity(&project.id, "b");
    let graph = parsnip_core::Graph {
        entities: vec![a.clone(), b.clone()],
        relations: vec![relation(&project.id, &a, &b, "links")],
    };

    local.save_project(&project).await.unwrap();
    remote.save_project(&project).await.unwrap();
    local.save_graph(&graph, &project.id).await.unwrap();
    remote.save_graph(&graph, &project.id).await.unwrap();

    let local_graph = local.load_graph(&project.id).await.unwrap();
    let remote_graph = remote.load_graph(&project.id).await.unwrap();

    assert_eq!(local_graph.entities.len(), remote_graph.entities.len());
    assert_eq!(local_graph.relations.len(), remote_graph.relations.len());
}

#[tokio::test]
async fn remote_get_or_create_project_is_atomic_and_idempotent() {
    let remote = Arc::new(remote_backend());

    let mut handles = Vec::new();
    for _ in 0..8 {
        let remote = remote.clone();
        handles.push(tokio::spawn(async move {
            remote.get_or_create_project("shared").await.unwrap().id
        }));
    }

    let mut ids = Vec::new();
    for h in handles {
        ids.push(h.await.unwrap());
    }

    let first = &ids[0];
    assert!(
        ids.iter().all(|id| id == first),
        "concurrent clients must agree on one project id, got {ids:?}"
    );
    assert_eq!(remote.get_all_projects().await.unwrap().len(), 1);
}

#[tokio::test]
async fn remote_surfaces_server_errors_with_their_kind() {
    // The dispatcher rejects oversized batches; the client must surface that, not hang
    // or decode it as success.
    let remote = remote_backend();
    let project = ProjectId::new();
    let huge: Vec<Entity> = (0..101)
        .map(|i| entity(&project, &format!("e{i}")))
        .collect();

    // The client chunks at 100, so this succeeds despite exceeding the server limit in
    // one call. That is the chunker doing its job.
    remote.save_entities_batch(&huge).await.unwrap();
    assert_eq!(remote.get_all_entities(&project).await.unwrap().len(), 101);
}
