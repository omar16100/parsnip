//! SSE transport for MCP server
//!
//! Implements MCP over HTTP with SSE for server-to-client events.

#[cfg(feature = "sse")]
use std::sync::Arc;

#[cfg(feature = "sse")]
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
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
use tower_http::cors::CorsLayer;

#[cfg(feature = "sse")]
use tower_http::limit::RequestBodyLimitLayer;

#[cfg(feature = "sse")]
use crate::transport::JsonRpcRequest;

#[cfg(feature = "sse")]
use crate::McpServer;

/// Maximum request body size.
///
/// Raised from 1MB for the storage RPC: `parsnip import` sends entity batches, and a
/// batch of entities carrying large observations exceeds 1MB easily. Clients still chunk
/// their batches, so this is a ceiling rather than the mechanism.
#[cfg(feature = "sse")]
const MAX_BODY_SIZE: usize = 32 * 1024 * 1024;

/// SSE transport state
#[cfg(feature = "sse")]
pub struct SseState<S: StorageBackend + ?Sized> {
    server: Arc<McpServer<S>>,
    event_tx: broadcast::Sender<String>,
    auth_token: Option<String>,
}

#[cfg(feature = "sse")]
impl<S: StorageBackend + ?Sized + Send + Sync + 'static> SseState<S> {
    pub fn new(server: Arc<McpServer<S>>, auth_token: Option<String>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            server,
            event_tx,
            auth_token,
        }
    }
}

/// Auth middleware - validates Bearer token if configured
#[cfg(feature = "sse")]
async fn auth_middleware<S: StorageBackend + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<SseState<S>>>,
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Skip auth for health endpoint
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    // No token configured: only reachable on loopback (`serve` refuses anything else), but
    // a web page can still reach a loopback port through DNS rebinding, under its own
    // hostname. Browsers always send that hostname in Host, so insist on a loopback one.
    let Some(expected_token) = &state.auth_token else {
        if !host_is_loopback(&headers) {
            tracing::warn!(
                host = ?headers.get(header::HOST),
                "rejected tokenless request with a non-loopback Host header"
            );
            return (
                StatusCode::FORBIDDEN,
                "Host not allowed: this daemon has no auth token, so it only accepts \
                 loopback hostnames. Set --auth-token to serve other names.",
            )
                .into_response();
        }
        return next.run(request).await;
    };

    // Check Authorization header
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(auth) if auth.starts_with("Bearer ") => {
            let token = &auth[7..];
            if tokens_match(token, expected_token) {
                next.run(request).await
            } else {
                tracing::warn!(path = %request.uri().path(), "rejected request with an invalid token");
                (StatusCode::UNAUTHORIZED, "Invalid token").into_response()
            }
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid Authorization header",
        )
            .into_response(),
    }
}

/// True if the Host header names a loopback address (`localhost`, `127.x.y.z`, `[::1]`),
/// with or without a port. A missing header is allowed: browsers always send one, so its
/// absence cannot be a rebinding attack.
#[cfg(feature = "sse")]
fn host_is_loopback(headers: &HeaderMap) -> bool {
    let Some(value) = headers.get(header::HOST) else {
        return true;
    };
    let Ok(host) = value.to_str() else {
        return false;
    };

    // Strip the port: "[::1]:8787" -> "::1", "localhost:8787" -> "localhost". Anything
    // after the host other than ":<digits>" is malformed and rejected.
    let valid_port = |port: &str| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit());
    let name = if let Some(rest) = host.strip_prefix('[') {
        let Some((inner, after)) = rest.split_once(']') else {
            return false;
        };
        match after.strip_prefix(':') {
            None if after.is_empty() => inner,
            Some(port) if valid_port(port) => inner,
            _ => return false,
        }
    } else {
        match host.rsplit_once(':') {
            Some((name, port)) if valid_port(port) => name,
            Some(_) => return false,
            None => host,
        }
    };

    name.eq_ignore_ascii_case("localhost")
        || name
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Compare a presented token with the expected one without an early exit on the first
/// differing byte, so response timing does not reveal how much of a guess was right.
/// Only the length can leak, which says nothing useful about a random token.
#[cfg(feature = "sse")]
fn tokens_match(presented: &str, expected: &str) -> bool {
    let (a, b) = (presented.as_bytes(), expected.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Create the SSE router
#[cfg(feature = "sse")]
pub fn create_sse_router<S: StorageBackend + ?Sized + Send + Sync + 'static>(
    server: Arc<McpServer<S>>,
    auth_token: Option<String>,
) -> Router {
    let state = Arc::new(SseState::new(server, auth_token));

    // Restrictive CORS: only allow localhost origins
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:3000".parse().unwrap(),
            "http://127.0.0.1:3000".parse().unwrap(),
            "http://localhost:8080".parse().unwrap(),
            "http://127.0.0.1:8080".parse().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    Router::new()
        .route("/sse", get(sse_handler::<S>))
        .route("/message", post(message_handler::<S>))
        .route("/health", get(health_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware::<S>,
        ))
        .with_state(state)
        .layer(cors)
        // Two independent limits: axum's Json extractor enforces DefaultBodyLimit (2MB)
        // regardless of the tower layer, so raising only one still rejects large bodies.
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
}

/// Health check endpoint
#[cfg(feature = "sse")]
async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "server": "parsnip-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        // Advertised so remote clients can detect version skew up front and report it
        // clearly, instead of hitting "method not found" per call.
        "capabilities": crate::CAPABILITIES,
        "maxBodySize": MAX_BODY_SIZE
    }))
}

/// SSE endpoint for server-to-client events
#[cfg(feature = "sse")]
async fn sse_handler<S: StorageBackend + ?Sized + Send + Sync + 'static>(
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
async fn message_handler<S: StorageBackend + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<SseState<S>>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    tracing::debug!("Received SSE request: {:?}", request.method);

    // Storage RPCs are point-to-point CLI traffic. Broadcasting their responses would push
    // whole-graph payloads at every SSE subscriber, so only MCP responses go on the stream.
    // The broadcast is the sole delivery path to /sse subscribers, so it must stay for MCP.
    // `search/query` is CLI traffic too, and its responses carry full entities.
    let broadcastable = !request.method.starts_with(crate::STORAGE_METHOD_PREFIX)
        && request.method != crate::SEARCH_METHOD;

    let response = state.server.handle_request_public(request).await;

    if broadcastable {
        if let Ok(json) = serde_json::to_string(&response) {
            let _ = state.event_tx.send(json);
        }
    }

    Json(response)
}

/// Run the SSE server
#[cfg(feature = "sse")]
pub async fn run_sse_server<S: StorageBackend + ?Sized + Send + Sync + 'static>(
    server: Arc<McpServer<S>>,
    addr: &str,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    if auth_token.is_none() {
        tracing::warn!(
            "no auth token set: any process that can reach {addr} can read and write the \
             whole graph. Set --auth-token or PARSNIP_AUTH_TOKEN."
        );
    }

    let router = create_sse_router(server, auth_token);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Log the address actually bound, not the one requested, so `--port 0` is usable and
    // the log is truthful when the two differ.
    let bound = listener.local_addr()?;
    tracing::info!("MCP SSE server listening on {}", bound);
    tracing::info!("  SSE endpoint: http://{}/sse", bound);
    tracing::info!("  Message endpoint: http://{}/message", bound);
    tracing::info!("  Health check: http://{}/health", bound);

    axum::serve(listener, router).await?;

    Ok(())
}

#[cfg(all(test, feature = "sse"))]
mod tests {
    use super::{host_is_loopback, tokens_match};
    use axum::http::{header, HeaderMap, HeaderValue};

    fn with_host(host: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_str(host).unwrap());
        headers
    }

    #[test]
    fn loopback_hosts_are_accepted() {
        for host in [
            "localhost",
            "LOCALHOST:8787",
            "127.0.0.1",
            "127.0.0.1:8787",
            "127.0.0.2:3000",
            "[::1]",
            "[::1]:8787",
        ] {
            assert!(
                host_is_loopback(&with_host(host)),
                "{host} should be accepted"
            );
        }
        assert!(
            host_is_loopback(&HeaderMap::new()),
            "missing Host is not a browser"
        );
    }

    #[test]
    fn other_hosts_are_rejected() {
        for host in [
            "evil.example",
            "evil.example:8787",
            "localhost.evil.example",
            "192.168.1.10:8787",
            "[fe80::1]:8787",
            "0.0.0.0:8787",
            "[::1]junk",
            "[::1]:80x",
            "localhost:",
            "127.0.0.1:abc",
        ] {
            assert!(
                !host_is_loopback(&with_host(host)),
                "{host} should be rejected"
            );
        }
    }

    #[test]
    fn tokens_match_only_on_exact_equality() {
        assert!(tokens_match("s3cret-token", "s3cret-token"));
        assert!(!tokens_match("s3cret-tokeN", "s3cret-token"));
        assert!(!tokens_match("s3cret", "s3cret-token"));
        assert!(!tokens_match("", "s3cret-token"));
        assert!(!tokens_match("s3cret-token-and-more", "s3cret-token"));
    }
}
