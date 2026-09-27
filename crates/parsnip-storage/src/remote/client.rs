//! Client side of the storage RPC: a [`StorageBackend`] that forwards to a daemon.
//!
//! Because it implements the same trait the CLI command handlers already call through,
//! pointing the CLI at a daemon needs no changes in those handlers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parsnip_core::{Entity, Graph, Project, ProjectId, Relation, SearchQuery};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::protocol::{method, GraphPayload, WireError};
use crate::error::{StorageError, StorageResult};
use crate::traits::StorageBackend;

/// Largest number of items sent in one batch request.
///
/// Chosen to sit inside the daemon's request body limit, not to enforce a semantic rule.
/// It matches `parsnip_core::limits::MAX_BATCH_ENTITIES`, which the daemon does enforce.
pub const MAX_BATCH_ITEMS: usize = 100;

/// Approximate serialized size at which a batch is split, even if under the item count.
pub const MAX_BATCH_BYTES: usize = 512 * 1024;

/// Server-side search. Not a `storage/` method: it needs the search engines, which the
/// storage crate deliberately does not depend on.
const SEARCH_METHOD: &str = "search/query";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Carries one JSON-RPC call to a daemon.
///
/// An abstraction rather than a hard-wired HTTP client so the whole protocol can be
/// exercised in tests by looping straight into the dispatcher.
#[async_trait]
pub trait RpcTransport: Send + Sync {
    async fn call(&self, method: &str, params: Value) -> StorageResult<Value>;
}

// The JSON-RPC envelope is redeclared here rather than imported: `parsnip-mcp` depends on
// this crate, so depending on it back would be circular. The contract test drives a real
// router, which is what catches drift between the two declarations.
#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    #[allow(dead_code)]
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

/// JSON-RPC over HTTP against a `parsnip serve --transport sse` daemon.
pub struct HttpTransport {
    client: reqwest::Client,
    message_url: String,
    health_url: String,
    auth_token: Option<String>,
    next_id: AtomicU64,
}

impl HttpTransport {
    /// Build a transport for a daemon base URL such as `http://100.x.y.z:8787`.
    pub fn new(base_url: &str, auth_token: Option<String>) -> StorageResult<Self> {
        let base = base_url.trim_end_matches('/');

        if base.starts_with("https://") && !cfg!(feature = "remote-tls") {
            return Err(StorageError::Remote(format!(
                "https URLs need the `remote-tls` feature; got {base}. \
                 Over Tailscale, plain http is already encrypted by WireGuard."
            )));
        }
        if !base.starts_with("http://") && !base.starts_with("https://") {
            return Err(StorageError::Remote(format!(
                "server URL must start with http:// or https://; got {base}"
            )));
        }

        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| StorageError::Remote(format!("could not build HTTP client: {e}")))?;

        Ok(Self {
            client,
            message_url: format!("{base}/message"),
            health_url: format!("{base}/health"),
            auth_token,
            next_id: AtomicU64::new(1),
        })
    }

    /// Ask the daemon what it supports.
    ///
    /// Called once when connecting so version skew is reported as one clear message
    /// rather than as "method not found" on whichever call happens to run first.
    pub async fn fetch_capabilities(&self) -> StorageResult<Vec<String>> {
        let response = self
            .client
            .get(&self.health_url)
            .send()
            .await
            .map_err(|e| StorageError::Remote(format!("cannot reach parsnip daemon: {e}")))?;

        let body: Value = response.json().await.map_err(|e| {
            StorageError::Remote(format!("daemon returned invalid health JSON: {e}"))
        })?;

        Ok(body
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default())
    }
}

#[async_trait]
impl RpcTransport for HttpTransport {
    async fn call(&self, method: &str, params: Value) -> StorageResult<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = RpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let mut builder = self.client.post(&self.message_url).json(&request);
        if let Some(token) = &self.auth_token {
            // Never logged: tracing below records the method only.
            builder = builder.bearer_auth(token);
        }

        tracing::debug!(method, id, "storage RPC request");

        let response = builder.send().await.map_err(|e| {
            if e.is_timeout() {
                StorageError::Remote(format!("parsnip daemon timed out on {method}"))
            } else if e.is_connect() {
                StorageError::Remote(format!(
                    "cannot reach parsnip daemon: {e}. Is `parsnip serve` running?"
                ))
            } else {
                StorageError::Remote(format!("request to parsnip daemon failed: {e}"))
            }
        })?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(StorageError::Remote(
                "parsnip daemon rejected the token; check --auth-token or PARSNIP_AUTH_TOKEN"
                    .to_string(),
            ));
        }
        if status == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
            return Err(StorageError::Remote(format!(
                "request too large for the daemon's body limit (method {method})"
            )));
        }
        if !status.is_success() {
            // Body may be an HTML error page from a proxy, so truncate it.
            let body = response.text().await.unwrap_or_default();
            let snippet: String = body.chars().take(200).collect();
            return Err(StorageError::Remote(format!(
                "parsnip daemon returned HTTP {status}: {snippet}"
            )));
        }

        let envelope: RpcResponse = response.json().await.map_err(|e| {
            StorageError::Remote(format!("daemon returned a non JSON-RPC body: {e}"))
        })?;

        if let Some(err) = envelope.error {
            // Prefer the structured payload; it maps back to a real StorageError variant.
            if let Some(data) = err.data {
                if let Ok(wire) = serde_json::from_value::<WireError>(data) {
                    if wire.kind == "MethodNotFound" {
                        return Err(StorageError::Remote(format!(
                            "the parsnip daemon does not support {method}. \
                             It is likely older than this client; upgrade parsnip on the daemon host."
                        )));
                    }
                    return Err(StorageError::from(wire));
                }
            }
            return Err(StorageError::Remote(err.message));
        }

        Ok(envelope.result.unwrap_or(Value::Null))
    }
}

/// A [`StorageBackend`] backed by a remote daemon.
pub struct RemoteStorage {
    transport: Arc<dyn RpcTransport>,
}

impl RemoteStorage {
    /// Connect to a daemon and verify it speaks this protocol.
    pub async fn connect(base_url: &str, auth_token: Option<String>) -> StorageResult<Self> {
        let transport = HttpTransport::new(base_url, auth_token)?;

        let capabilities = transport.fetch_capabilities().await?;
        if !capabilities.iter().any(|c| c == "storage/v1") {
            return Err(StorageError::Remote(format!(
                "the parsnip daemon at {base_url} does not advertise storage/v1 \
                 (it reports: {}). Upgrade parsnip on the daemon host.",
                if capabilities.is_empty() {
                    "nothing".to_string()
                } else {
                    capabilities.join(", ")
                }
            )));
        }

        tracing::debug!(url = base_url, ?capabilities, "connected to parsnip daemon");
        Ok(Self {
            transport: Arc::new(transport),
        })
    }

    /// Build over an arbitrary transport. Used by tests to bypass HTTP.
    pub fn with_transport(transport: Arc<dyn RpcTransport>) -> Self {
        Self { transport }
    }

    /// Run a search on the daemon rather than pulling the corpus to search it locally.
    ///
    /// `SearchQuery` already carries mode, filters, project scope and thresholds, so no
    /// new types are needed. Full-text and hybrid only work this way in remote mode: the
    /// index belongs to whoever owns the data.
    pub async fn search(
        &self,
        query: &SearchQuery,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Entity>> {
        #[derive(Deserialize)]
        struct SearchResults {
            entities: Vec<Entity>,
        }

        let value = self
            .transport
            .call(
                SEARCH_METHOD,
                serde_json::json!({ "query": query, "limit": limit }),
            )
            .await?;
        let results: SearchResults = decode(value)?;
        Ok(results.entities)
    }

    async fn call(&self, method: &str, params: Value) -> StorageResult<Value> {
        self.transport.call(method, params).await
    }

    /// Split a slice into request-sized chunks.
    ///
    /// Bounded by item count and by serialized size, because a handful of entities with
    /// large observations can exceed the body limit well before the item count does.
    fn chunks<T: Serialize>(items: &[T]) -> Vec<&[T]> {
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut bytes = 0;

        for i in 0..items.len() {
            let item_bytes = serde_json::to_vec(&items[i]).map(|v| v.len()).unwrap_or(0);

            let too_many = i - start >= MAX_BATCH_ITEMS;
            let too_big = i > start && bytes + item_bytes > MAX_BATCH_BYTES;
            if too_many || too_big {
                chunks.push(&items[start..i]);
                start = i;
                bytes = 0;
            }
            bytes += item_bytes;
        }

        if start < items.len() {
            chunks.push(&items[start..]);
        }
        chunks
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> StorageResult<T> {
    serde_json::from_value(value)
        .map_err(|e| StorageError::Remote(format!("could not decode daemon response: {e}")))
}

#[async_trait]
impl StorageBackend for RemoteStorage {
    /// No-op: the daemon owns the database and initialized it at startup.
    async fn initialize(&self) -> StorageResult<()> {
        Ok(())
    }

    /// No-op by design. Forwarding this would let any client drop the daemon's handle.
    async fn close(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn health_check(&self) -> StorageResult<bool> {
        decode(self.call(method::HEALTH_CHECK, Value::Null).await?)
    }

    async fn save_entity(&self, entity: &Entity) -> StorageResult<()> {
        self.call(method::SAVE_ENTITY, serde_json::json!({ "entity": entity }))
            .await?;
        Ok(())
    }

    async fn get_entity(
        &self,
        name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Option<Entity>> {
        decode(
            self.call(
                method::GET_ENTITY,
                serde_json::json!({"name": name, "project_id": project_id}),
            )
            .await?,
        )
    }

    async fn get_all_entities(&self, project_id: &ProjectId) -> StorageResult<Vec<Entity>> {
        decode(
            self.call(
                method::GET_ALL_ENTITIES,
                serde_json::json!({ "project_id": project_id }),
            )
            .await?,
        )
    }

    async fn get_all_entities_all_projects(&self) -> StorageResult<Vec<Entity>> {
        decode(
            self.call(method::GET_ALL_ENTITIES_ALL_PROJECTS, Value::Null)
                .await?,
        )
    }

    async fn delete_entity(&self, name: &str, project_id: &ProjectId) -> StorageResult<()> {
        self.call(
            method::DELETE_ENTITY,
            serde_json::json!({"name": name, "project_id": project_id}),
        )
        .await?;
        Ok(())
    }

    async fn save_relation(&self, relation: &Relation) -> StorageResult<()> {
        self.call(
            method::SAVE_RELATION,
            serde_json::json!({ "relation": relation }),
        )
        .await?;
        Ok(())
    }

    async fn get_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<Vec<Relation>> {
        decode(
            self.call(
                method::GET_RELATIONS_FOR_ENTITY,
                serde_json::json!({"entity_name": entity_name, "project_id": project_id}),
            )
            .await?,
        )
    }

    async fn get_all_relations(&self, project_id: &ProjectId) -> StorageResult<Vec<Relation>> {
        decode(
            self.call(
                method::GET_ALL_RELATIONS,
                serde_json::json!({ "project_id": project_id }),
            )
            .await?,
        )
    }

    async fn get_all_relations_all_projects(&self) -> StorageResult<Vec<Relation>> {
        decode(
            self.call(method::GET_ALL_RELATIONS_ALL_PROJECTS, Value::Null)
                .await?,
        )
    }

    async fn get_relations_for_entity_global(
        &self,
        entity_name: &str,
    ) -> StorageResult<Vec<Relation>> {
        decode(
            self.call(
                method::GET_RELATIONS_FOR_ENTITY_GLOBAL,
                serde_json::json!({ "entity_name": entity_name }),
            )
            .await?,
        )
    }

    async fn delete_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        self.call(
            method::DELETE_RELATION,
            serde_json::json!({
                "from": from,
                "to": to,
                "relation_type": relation_type,
                "project_id": project_id,
            }),
        )
        .await?;
        Ok(())
    }

    async fn delete_relations_for_entity(
        &self,
        entity_name: &str,
        project_id: &ProjectId,
    ) -> StorageResult<()> {
        self.call(
            method::DELETE_RELATIONS_FOR_ENTITY,
            serde_json::json!({"entity_name": entity_name, "project_id": project_id}),
        )
        .await?;
        Ok(())
    }

    async fn save_project(&self, project: &Project) -> StorageResult<()> {
        self.call(
            method::SAVE_PROJECT,
            serde_json::json!({ "project": project }),
        )
        .await?;
        Ok(())
    }

    async fn get_project(&self, name: &str) -> StorageResult<Option<Project>> {
        decode(
            self.call(method::GET_PROJECT, serde_json::json!({ "name": name }))
                .await?,
        )
    }

    async fn get_project_by_id(&self, id: &ProjectId) -> StorageResult<Option<Project>> {
        decode(
            self.call(method::GET_PROJECT_BY_ID, serde_json::json!({ "id": id }))
                .await?,
        )
    }

    async fn get_all_projects(&self) -> StorageResult<Vec<Project>> {
        decode(self.call(method::GET_ALL_PROJECTS, Value::Null).await?)
    }

    async fn delete_project(&self, name: &str) -> StorageResult<()> {
        self.call(method::DELETE_PROJECT, serde_json::json!({ "name": name }))
            .await?;
        Ok(())
    }

    /// One atomic server-side call instead of the default get-then-create, which races
    /// between clients sharing a daemon (see the trait method).
    async fn get_or_create_project(&self, name: &str) -> StorageResult<Project> {
        decode(
            self.call(
                method::GET_OR_CREATE_PROJECT,
                serde_json::json!({ "name": name }),
            )
            .await?,
        )
    }

    /// Overridden rather than inherited: the default implementation issues two calls.
    async fn load_graph(&self, project_id: &ProjectId) -> StorageResult<Graph> {
        let payload: GraphPayload = decode(
            self.call(
                method::LOAD_GRAPH,
                serde_json::json!({ "project_id": project_id }),
            )
            .await?,
        )?;
        Ok(Graph {
            entities: payload.entities,
            relations: payload.relations,
        })
    }

    /// `project_id` is ignored, matching every local backend: each entity and relation
    /// already carries its own project id, and the trait's "replaces existing" doc is
    /// aspirational, since all backends upsert rather than replace.
    async fn save_graph(&self, graph: &Graph, _project_id: &ProjectId) -> StorageResult<()> {
        // Sent as batches so a large graph stays inside the body limit. Note this is not
        // one transaction on the daemon; see save_entities_batch.
        self.save_entities_batch(&graph.entities).await?;
        self.save_relations_batch(&graph.relations).await?;
        Ok(())
    }

    /// Chunked to fit the daemon's body limit.
    ///
    /// The local redb backend commits a whole batch in a single write transaction; over
    /// the wire a large batch becomes several, so an interrupted call can leave part of
    /// the batch applied. Callers that need all-or-nothing must handle that themselves.
    async fn save_entities_batch(&self, entities: &[Entity]) -> StorageResult<()> {
        if entities.is_empty() {
            return Ok(());
        }
        for chunk in Self::chunks(entities) {
            self.call(
                method::SAVE_ENTITIES_BATCH,
                serde_json::json!({ "entities": chunk }),
            )
            .await?;
        }
        Ok(())
    }

    /// Chunked; same non-atomicity note as [`Self::save_entities_batch`].
    async fn save_relations_batch(&self, relations: &[Relation]) -> StorageResult<()> {
        if relations.is_empty() {
            return Ok(());
        }
        for chunk in Self::chunks(relations) {
            self.call(
                method::SAVE_RELATIONS_BATCH,
                serde_json::json!({ "relations": chunk }),
            )
            .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_without_tls_feature_is_refused_clearly() {
        match HttpTransport::new("https://example.com", None) {
            Ok(_) => panic!("https must be refused without the remote-tls feature"),
            Err(e) => assert!(
                e.to_string().contains("remote-tls"),
                "error should name the feature, got: {e}"
            ),
        }
    }

    #[test]
    fn non_http_scheme_is_refused() {
        assert!(HttpTransport::new("example.com:8787", None).is_err());
        assert!(HttpTransport::new("ftp://example.com", None).is_err());
    }

    #[test]
    fn trailing_slash_does_not_double_up_in_paths() {
        let t = HttpTransport::new("http://host:8787/", None).unwrap();
        assert_eq!(t.message_url, "http://host:8787/message");
        assert_eq!(t.health_url, "http://host:8787/health");
    }

    #[test]
    fn chunking_splits_on_item_count() {
        let items: Vec<u32> = (0..250).collect();
        let chunks = RemoteStorage::chunks(&items);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), MAX_BATCH_ITEMS);
        assert_eq!(chunks[1].len(), MAX_BATCH_ITEMS);
        assert_eq!(chunks[2].len(), 50);
        assert_eq!(
            chunks.iter().map(|c| c.len()).sum::<usize>(),
            items.len(),
            "chunking must not drop items"
        );
    }

    #[test]
    fn chunking_splits_on_byte_size_before_item_count() {
        // Ten items that are each ~200KB: the item count never trips, the byte cap must.
        let big = "x".repeat(200 * 1024);
        let items: Vec<String> = (0..10).map(|_| big.clone()).collect();
        let chunks = RemoteStorage::chunks(&items);
        assert!(
            chunks.len() > 1,
            "oversized items must split despite being under the item count"
        );
        assert_eq!(chunks.iter().map(|c| c.len()).sum::<usize>(), items.len());
    }

    #[test]
    fn chunking_handles_empty_and_single() {
        let empty: Vec<u32> = vec![];
        assert!(RemoteStorage::chunks(&empty).is_empty());
        let one = vec![1u32];
        assert_eq!(RemoteStorage::chunks(&one).len(), 1);
    }
}
