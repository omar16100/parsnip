//! Contract tests: a real axum router, real HTTP, and the real `RemoteStorage` client.
//!
//! The loopback conformance suite in `parsnip-storage` proves the codec is symmetric, but
//! it never touches the HTTP envelope. The client redeclares that envelope (depending on
//! `parsnip-mcp` from `parsnip-storage` would be circular), so these tests are what catch
//! the two declarations drifting apart, along with auth, status-code and body-limit
//! behaviour that only exists over a socket.

#![cfg(feature = "sse")]

use std::sync::Arc;

use parsnip_core::{Entity, Project, ProjectId};
use parsnip_mcp::sse::create_sse_router;
use parsnip_mcp::McpServer;
use parsnip_storage::memory::MemoryStorage;
use parsnip_storage::remote::client::RemoteStorage;
use parsnip_storage::StorageBackend;

/// Boot a daemon on an ephemeral port and return its base URL.
///
/// Built over `Arc<dyn StorageBackend>` on purpose: that is the shape the CLI uses once
/// storage is chosen at runtime, and it is what the `?Sized` bounds exist for.
async fn spawn_daemon(auth_token: Option<String>) -> (String, Arc<dyn StorageBackend>) {
    let backend: Arc<dyn StorageBackend> = Arc::new(MemoryStorage::new());
    let server = Arc::new(McpServer::new(backend.clone()));
    let router = create_sse_router(server, auth_token);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    // Wait for the listener to start answering before handing the URL out.
    let base = format!("http://{addr}");
    let health = format!("{base}/health");
    for _ in 0..50 {
        if reqwest::get(&health).await.is_ok() {
            return (base, backend);
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("daemon did not become healthy");
}

#[tokio::test]
async fn health_reports_capabilities_without_auth() {
    let (base, _) = spawn_daemon(Some("secret".into())).await;

    let body: serde_json::Value = reqwest::get(format!("{base}/health"))
        .await
        .expect("health is reachable without a token")
        .json()
        .await
        .expect("health returns JSON");

    assert_eq!(body["status"], "ok");
    let caps: Vec<&str> = body["capabilities"]
        .as_array()
        .expect("capabilities array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(caps.contains(&"storage/v1"), "got {caps:?}");
}

#[tokio::test]
async fn round_trip_over_real_http() {
    let (base, _) = spawn_daemon(None).await;
    let remote = RemoteStorage::connect(&base, None)
        .await
        .expect("connect to daemon");

    assert!(remote.health_check().await.unwrap());

    let project = remote.get_or_create_project("contract").await.unwrap();
    let mut entity = Entity::new(project.id.clone(), "widget", "thing");
    entity.add_observation("made of atoms");
    entity.add_tag("physical");
    remote.save_entity(&entity).await.unwrap();

    let fetched = remote
        .get_entity("widget", &project.id)
        .await
        .unwrap()
        .expect("entity round-trips over HTTP");
    assert_eq!(fetched.name, "widget");
    assert_eq!(fetched.observations.len(), 1);
    assert_eq!(fetched.tags, vec!["physical".to_string()]);

    // A miss must decode as None rather than as an error.
    assert!(remote
        .get_entity("absent", &project.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn wrong_token_is_rejected_with_a_useful_message() {
    let (base, _) = spawn_daemon(Some("correct-token".into())).await;

    let remote = RemoteStorage::connect(&base, Some("wrong-token".into()))
        .await
        .expect("connect succeeds: /health is deliberately unauthenticated");

    let err = remote
        .health_check()
        .await
        .expect_err("an authenticated call must fail with the wrong token");
    let msg = err.to_string();
    assert!(
        msg.contains("token"),
        "error should point at the token, got: {msg}"
    );
}

#[tokio::test]
async fn correct_token_is_accepted() {
    let (base, _) = spawn_daemon(Some("correct-token".into())).await;
    let remote = RemoteStorage::connect(&base, Some("correct-token".into()))
        .await
        .unwrap();
    assert!(remote.health_check().await.unwrap());
}

#[tokio::test]
async fn storage_errors_keep_their_kind_across_the_wire() {
    let (base, _) = spawn_daemon(None).await;

    // Posted raw so it bypasses the client's chunker and trips the server-side batch
    // limit, which is the point: the failure must come back with a structured kind, not
    // just prose, or the client cannot rebuild a real StorageError.
    let project = ProjectId::new();
    let entities: Vec<Entity> = (0..101)
        .map(|i| Entity::new(project.clone(), format!("e{i}"), "thing"))
        .collect();

    let response: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/message"))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "storage/save_entities_batch",
            "params": { "entities": entities }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(response["error"]["data"]["kind"], "InvalidParams");
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("too many"),
        "message should say what was wrong, got {}",
        response["error"]["message"]
    );
}

#[tokio::test]
async fn missing_rows_decode_as_none_not_errors() {
    let (base, _) = spawn_daemon(None).await;
    let remote = RemoteStorage::connect(&base, None).await.unwrap();

    assert!(remote
        .get_project_by_id(&ProjectId::new())
        .await
        .expect("a lookup miss is not an error")
        .is_none());
    assert!(remote.get_project("nope").await.unwrap().is_none());
}

#[tokio::test]
async fn large_batch_passes_the_raised_body_limit() {
    let (base, _) = spawn_daemon(None).await;
    let remote = RemoteStorage::connect(&base, None).await.unwrap();

    let project = remote.get_or_create_project("bulk").await.unwrap();

    // ~5MB of entities: over the old 1MB limit and over axum's 2MB default, so this
    // fails unless both limits were raised. The client chunks, which is also exercised.
    let blob = "x".repeat(50 * 1024);
    let entities: Vec<Entity> = (0..100)
        .map(|i| {
            let mut e = Entity::new(project.id.clone(), format!("bulk_{i}"), "thing");
            e.add_observation(blob.clone());
            e
        })
        .collect();

    remote.save_entities_batch(&entities).await.unwrap();
    assert_eq!(
        remote.get_all_entities(&project.id).await.unwrap().len(),
        100
    );
}

#[tokio::test]
async fn mcp_tools_still_work_alongside_the_storage_rpc() {
    let (base, _) = spawn_daemon(None).await;

    // The storage RPC must not have displaced the MCP surface; both share /message.
    let response: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/message"))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let tools = response["result"]["tools"]
        .as_array()
        .expect("tools/list still returns tools");
    assert!(!tools.is_empty());
}

#[tokio::test]
async fn unknown_storage_method_reports_a_version_skew_message() {
    let (base, _) = spawn_daemon(None).await;

    let response: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/message"))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "storage/does_not_exist",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(
        response["error"]["data"]["kind"], "MethodNotFound",
        "the structured kind is what lets the client explain version skew"
    );
}

#[tokio::test]
async fn local_and_remote_agree_on_the_same_backend() {
    // One backend, reached directly and through HTTP: the two views must not diverge.
    let (base, backend) = spawn_daemon(None).await;
    let remote = RemoteStorage::connect(&base, None).await.unwrap();

    let project = Project::new("shared");
    backend.save_project(&project).await.unwrap();

    let via_remote = remote
        .get_project("shared")
        .await
        .unwrap()
        .expect("a project written locally is visible remotely");
    assert_eq!(via_remote.id, project.id);

    let mut entity = Entity::new(project.id.clone(), "written_remotely", "thing");
    entity.add_observation("via HTTP");
    remote.save_entity(&entity).await.unwrap();

    let via_local = backend
        .get_entity("written_remotely", &project.id)
        .await
        .unwrap()
        .expect("an entity written remotely is visible locally");
    assert_eq!(via_local.observations.len(), 1);
}

/// A tokenless daemon is reachable by a web page through DNS rebinding, under the page's
/// own hostname. It must only answer loopback hostnames.
#[tokio::test]
async fn tokenless_daemon_rejects_foreign_host_headers() {
    let (base, _) = spawn_daemon(None).await;
    let ping = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
    let client = reqwest::Client::new();

    let foreign = client
        .post(format!("{base}/message"))
        .header(reqwest::header::HOST, "rebind.example:8787")
        .json(&ping)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), reqwest::StatusCode::FORBIDDEN);

    let loopback = client
        .post(format!("{base}/message"))
        .json(&ping)
        .send()
        .await
        .unwrap();
    assert!(loopback.status().is_success(), "{}", loopback.status());

    // With a token the Host check is not needed: a rebinding page does not have it.
    let (base, _) = spawn_daemon(Some("t0ken".into())).await;
    let with_token = client
        .post(format!("{base}/message"))
        .header(reqwest::header::HOST, "daemon.example:8787")
        .bearer_auth("t0ken")
        .json(&ping)
        .send()
        .await
        .unwrap();
    assert!(with_token.status().is_success(), "{}", with_token.status());
}

/// `search/query` with a multi-project scope must not widen to every project.
#[tokio::test]
async fn search_respects_a_multi_project_scope() {
    let (base, backend) = spawn_daemon(None).await;

    let mut ids = Vec::new();
    for name in ["alpha", "beta", "gamma"] {
        let project = Project::new(name);
        backend.save_project(&project).await.unwrap();
        let mut entity = Entity::new(project.id.clone(), format!("{name}_widget"), "thing");
        entity.add_observation("shared term");
        backend.save_entity(&entity).await.unwrap();
        ids.push(project.id);
    }

    let mut query = parsnip_core::SearchQuery::new("widget");
    query.projects = parsnip_core::ProjectScope::Multiple(vec![ids[0].clone(), ids[1].clone()]);

    let response: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/message"))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "search/query",
            "params": { "query": query }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let mut names: Vec<&str> = response["result"]["entities"]
        .as_array()
        .expect("entities array")
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, vec!["alpha_widget", "beta_widget"]);
}
