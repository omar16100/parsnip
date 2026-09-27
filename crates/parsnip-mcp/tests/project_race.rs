//! Project creation races between the MCP tool surface and storage RPC clients.
//!
//! Entity keys embed the project id, so if two callers each mint an id for the same new
//! project, whichever loses the name leaves its entities unreachable. The storage RPC
//! resolves projects under a lock; these tests pin that the MCP tool surface in the same
//! daemon, and MCP proxies that forward to a daemon, use that same atomic path instead of
//! a private get-then-create.
//!
//! `get_project` is slowed down so the get-then-create window is wide enough to lose
//! reliably when the lock is bypassed, rather than only on an unlucky schedule.

#![cfg(feature = "sse")]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parsnip_core::{Entity, Graph, Project, ProjectId, Relation};
use parsnip_mcp::sse::create_sse_router;
use parsnip_mcp::transport::JsonRpcRequest;
use parsnip_mcp::McpServer;
use parsnip_storage::memory::MemoryStorage;
use parsnip_storage::remote::client::RemoteStorage;
use parsnip_storage::{StorageBackend, StorageResult};

/// MemoryStorage with a slow `get_project`, to widen the get-then-create window.
struct SlowProjectLookup(MemoryStorage);

#[async_trait]
impl StorageBackend for SlowProjectLookup {
    async fn initialize(&self) -> StorageResult<()> {
        self.0.initialize().await
    }
    async fn close(&self) -> StorageResult<()> {
        self.0.close().await
    }
    async fn health_check(&self) -> StorageResult<bool> {
        self.0.health_check().await
    }
    async fn save_entity(&self, entity: &Entity) -> StorageResult<()> {
        self.0.save_entity(entity).await
    }
    async fn get_entity(
        &self,
        name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Option<Entity>> {
        self.0.get_entity(name, project_id).await
    }
    async fn get_all_entities(&self, project_id: &ProjectId) -> StorageResult<Vec<Entity>> {
        self.0.get_all_entities(project_id).await
    }
    async fn get_all_entities_all_projects(&self) -> StorageResult<Vec<Entity>> {
        self.0.get_all_entities_all_projects().await
    }
    async fn delete_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<()> {
        self.0.delete_entity(name, project_id).await
    }
    async fn save_relation(&self, relation: &Relation) -> StorageResult<()> {
        self.0.save_relation(relation).await
    }
    async fn get_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Vec<Relation>> {
        self.0
            .get_relations_for_entity(entity_name, project_id)
            .await
    }
    async fn get_all_relations(&self, project_id: &ProjectId) -> StorageResult<Vec<Relation>> {
        self.0.get_all_relations(project_id).await
    }
    async fn get_all_relations_all_projects(&self) -> StorageResult<Vec<Relation>> {
        self.0.get_all_relations_all_projects().await
    }
    async fn get_relations_for_entity_global(
        &self,
        entity_name: &str,
    ) -> StorageResult<Vec<Relation>> {
        self.0.get_relations_for_entity_global(entity_name).await
    }
    async fn delete_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        self.0
            .delete_relation(from, to, relation_type, project_id)
            .await
    }
    async fn delete_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        self.0
            .delete_relations_for_entity(entity_name, project_id)
            .await
    }
    async fn save_project(&self, project: &Project) -> StorageResult<()> {
        self.0.save_project(project).await
    }
    async fn get_project(&self, name: &str) -> StorageResult<Option<Project>> {
        let found = self.0.get_project(name).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        found
    }
    async fn get_project_by_id(&self, id: &ProjectId) -> StorageResult<Option<Project>> {
        self.0.get_project_by_id(id).await
    }
    async fn get_all_projects(&self) -> StorageResult<Vec<Project>> {
        self.0.get_all_projects().await
    }
    async fn delete_project(&self, name: &str) -> StorageResult<()> {
        self.0.delete_project(name).await
    }
    async fn save_graph(&self, graph: &Graph, project_id: &ProjectId) -> StorageResult<()> {
        self.0.save_graph(graph, project_id).await
    }
}

fn slow_backend() -> Arc<dyn StorageBackend> {
    Arc::new(SlowProjectLookup(MemoryStorage::new()))
}

fn create_entity_call(project: &str, entity: &str) -> JsonRpcRequest {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "create_entities",
            "arguments": {
                "projectId": project,
                "entities": [
                    {"name": entity, "entityType": "thing", "observations": ["x"]}
                ]
            }
        }
    }))
    .expect("valid JSON-RPC request")
}

fn get_or_create_call(project: &str) -> JsonRpcRequest {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "storage/get_or_create_project",
        "params": {"name": project}
    }))
    .expect("valid JSON-RPC request")
}

/// Every entity created must be reachable under the one id the project name resolves to.
async fn assert_all_reachable(backend: &Arc<dyn StorageBackend>, project: &str, count: usize) {
    let resolved = backend
        .get_project(project)
        .await
        .unwrap()
        .expect("project exists");
    let reachable = backend.get_all_entities(&resolved.id).await.unwrap().len();
    assert_eq!(
        reachable, count,
        "only {reachable} of {count} entities are reachable under project '{project}': \
         concurrent callers minted different project ids"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_tools_and_storage_rpc_share_one_project_lock() {
    let backend = slow_backend();
    let server = Arc::new(McpServer::new(backend.clone()));

    let mut handles = Vec::new();
    for i in 0..6 {
        let s = server.clone();
        handles.push(tokio::spawn(async move {
            let response = s
                .handle_request_public(create_entity_call("race", &format!("tool_{i}")))
                .await;
            assert!(response.error.is_none(), "tool call failed: {response:?}");
        }));
        let s = server.clone();
        handles.push(tokio::spawn(async move {
            let response = s.handle_request_public(get_or_create_call("race")).await;
            assert!(
                response.error.is_none(),
                "storage call failed: {response:?}"
            );
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_all_reachable(&backend, "race", 6).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_proxies_resolve_projects_on_the_daemon() {
    // The daemon: sole owner of the data.
    let backend = slow_backend();
    let daemon = Arc::new(McpServer::new(backend.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = create_sse_router(daemon, None);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    // Two MCP proxies, as `parsnip serve --transport stdio` with PARSNIP_SERVER set would
    // build: each one's storage is RemoteStorage pointing at the daemon.
    let mut proxies = Vec::new();
    for _ in 0..2 {
        let mut remote = None;
        for _ in 0..50 {
            if let Ok(r) = RemoteStorage::connect(&base, None).await {
                remote = Some(r);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let storage: Arc<dyn StorageBackend> = Arc::new(remote.expect("daemon became healthy"));
        proxies.push(Arc::new(McpServer::new(storage)));
    }

    let mut handles = Vec::new();
    for i in 0..8 {
        let proxy = proxies[i % 2].clone();
        handles.push(tokio::spawn(async move {
            let response = proxy
                .handle_request_public(create_entity_call("proxied", &format!("p_{i}")))
                .await;
            assert!(
                response.error.is_none(),
                "proxied tool call failed: {response:?}"
            );
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_all_reachable(&backend, "proxied", 8).await;
}
