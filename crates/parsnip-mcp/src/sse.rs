//! SSE transport for MCP server
//!
//! Implements MCP over HTTP with SSE for server-to-client events.

#[cfg(feature = "sse")]
use std::sync::Arc;

#[cfg(feature = "sse")]
use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};

#[cfg(feature = "sse")]
use futures::stream::Stream;

#[cfg(feature = "sse")]
use parsnip_storage::StorageBackend;

#[cfg(feature = "sse")]
use tokio::sync::broadcast;

#[cfg(feature = "sse")]
use tower_http::cors::{Any, CorsLayer};

#[cfg(feature = "sse")]
use crate::transport::JsonRpcRequest;

#[cfg(feature = "sse")]
use crate::McpServer;

/// SSE transport state
#[cfg(feature = "sse")]
pub struct SseState<S: StorageBackend> {
    server: Arc<McpServer<S>>,
    event_tx: broadcast::Sender<String>,
}

#[cfg(feature = "sse")]
impl<S: StorageBackend + Send + Sync + 'static> SseState<S> {
    pub fn new(server: Arc<McpServer<S>>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self { server, event_tx }
    }
}

/// Create the SSE router
#[cfg(feature = "sse")]
pub fn create_sse_router<S: StorageBackend + Send + Sync + 'static>(
    server: Arc<McpServer<S>>,
) -> Router {
    let state = Arc::new(SseState::new(server));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/sse", get(sse_handler::<S>))
        .route("/message", post(message_handler::<S>))
        .route("/health", get(health_handler))
        .with_state(state)
        .layer(cors)
}

/// Health check endpoint
#[cfg(feature = "sse")]
async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "server": "parsnip-mcp",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// SSE endpoint for server-to-client events
#[cfg(feature = "sse")]
async fn sse_handler<S: StorageBackend + Send + Sync + 'static>(
    State(state): State<Arc<SseState<S>>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut rx = state.event_tx.subscribe();

    // Send initial endpoint message
    let endpoint_url = "/message";
    let initial_event = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "endpoint",
        "params": {
            "endpoint": endpoint_url
        }
    });

    let initial_msg = serde_json::to_string(&initial_event).unwrap();

    let stream = async_stream::stream! {
        // Send endpoint info first
        yield Ok(Event::default().event("endpoint").data(initial_msg));

        // Then stream events from broadcast channel
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    yield Ok(Event::default().event("message").data(msg));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    tracing::warn!("SSE client lagged behind");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Sse::new(stream)
}

/// Message endpoint for client requests
#[cfg(feature = "sse")]
async fn message_handler<S: StorageBackend + Send + Sync + 'static>(
    State(state): State<Arc<SseState<S>>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    tracing::debug!("Received SSE request: {:?}", request.method);

    let response = state.server.handle_request_public(request).await;

    // Also broadcast to SSE clients if they want to see responses
    if let Ok(json) = serde_json::to_string(&response) {
        let _ = state.event_tx.send(json);
    }

    Json(response)
}

/// Run the SSE server
#[cfg(feature = "sse")]
pub async fn run_sse_server<S: StorageBackend + Send + Sync + 'static>(
    server: Arc<McpServer<S>>,
    addr: &str,
) -> anyhow::Result<()> {
    let router = create_sse_router(server);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("MCP SSE server listening on {}", addr);
    tracing::info!("  SSE endpoint: http://{}/sse", addr);
    tracing::info!("  Message endpoint: http://{}/message", addr);
    tracing::info!("  Health check: http://{}/health", addr);

    axum::serve(listener, router).await?;

    Ok(())
}
