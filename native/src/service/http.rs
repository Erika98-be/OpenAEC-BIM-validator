use super::{tools, Error, Service};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use serde_json::{json, Value};
use std::{
    net::{SocketAddr, TcpListener},
    sync::Arc,
    thread,
};
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
struct Access {
    token: String,
    port: u16,
}
pub struct Server {
    pub address: SocketAddr,
    pub service: Service,
    pub stop: CancellationToken,
    thread: Option<thread::JoinHandle<std::result::Result<(), String>>>,
}
impl Server {
    pub fn start(port: u16, token: String, service: Service) -> std::result::Result<Self, String> {
        if token.len() < 24
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Token must contain at least 24 ASCII letters, digits, - or _".into());
        }
        let listener =
            TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let stop = CancellationToken::new();
        let cancellation = stop.clone();
        let app = router(service.clone(), token, address.port(), stop.clone());
        let thread = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let result = runtime.block_on(async {
                let listener =
                    tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?;
                axum::serve(listener, app)
                    .with_graceful_shutdown(cancellation.cancelled_owned())
                    .await
                    .map_err(|e| e.to_string())
            });
            runtime.shutdown_timeout(std::time::Duration::from_secs(2));
            result
        });
        Ok(Self {
            address,
            service,
            stop,
            thread: Some(thread),
        })
    }
    pub fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }
    pub fn join(&mut self) -> std::result::Result<(), String> {
        self.thread
            .take()
            .map(|t| {
                t.join()
                    .unwrap_or_else(|_| Err("Server thread failed".into()))
            })
            .unwrap_or(Ok(()))
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
fn failure(status: StatusCode, message: &str) -> Response {
    let mut response = (status, Json(json!({"error":message}))).into_response();
    response.headers_mut().insert(
        header::CONNECTION,
        header::HeaderValue::from_static("close"),
    );
    response
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        failure(
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            &self.message,
        )
    }
}
async fn authorize(
    State(access): State<Arc<Access>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if host != format!("127.0.0.1:{}", access.port) && host != format!("localhost:{}", access.port)
    {
        return failure(StatusCode::FORBIDDEN, "Invalid Host");
    }
    if request.headers().contains_key(header::ORIGIN) {
        return failure(
            StatusCode::FORBIDDEN,
            "Browser-origin requests are not enabled; use a native client",
        );
    }
    let expected = format!("Bearer {}", access.token);
    if request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        != Some(expected.as_str())
    {
        return failure(StatusCode::UNAUTHORIZED, "Bearer token required");
    }
    // Buffer at most one byte beyond the JSON limit. This lets a small oversize
    // upload finish before returning 413, instead of resetting a socket while
    // the client is still writing. Larger/slow uploads are bounded as well.
    const MAX_BODY: usize = 1024 * 1024;
    let (parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        axum::body::to_bytes(body, MAX_BODY + 1),
    )
    .await
    {
        Ok(Ok(bytes)) if bytes.len() <= MAX_BODY => bytes,
        Ok(_) => return failure(StatusCode::PAYLOAD_TOO_LARGE, "Request body exceeds 1 MiB"),
        Err(_) => return failure(StatusCode::REQUEST_TIMEOUT, "Request body timeout"),
    };
    let request = axum::extract::Request::from_parts(parts, axum::body::Body::from(bytes));
    let mut response = next.run(request).await;
    // A rejected request can have an unread body. Explicitly close it so clients
    // cannot return that half-consumed connection to their keep-alive pool.
    if response.status().is_client_error() || response.status().is_server_error() {
        response.headers_mut().insert(
            header::CONNECTION,
            header::HeaderValue::from_static("close"),
        );
    }

    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        header::HeaderValue::from_static("nosniff"),
    );
    response
}
fn router(service: Service, token: String, port: u16, stop: CancellationToken) -> Router {
    let mcp_service = service.clone();
    let mcp = StreamableHttpService::new(
        move || Ok(super::mcp::Mcp(mcp_service.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_cancellation_token(stop)
            .with_json_response(true)
            .with_max_request_body_bytes(1024 * 1024),
    );
    Router::new()
        .route(
            "/api/v1/health",
            get(|| async {
                Json(json!({"status":"ok","version":env!("CARGO_PKG_VERSION"),"mcp":"/mcp"}))
            }),
        )
        .route("/api/v1/models", get(models))
        .route(
            "/api/v1/tools",
            get(|| async { Json(json!({"tools":tools()})) }),
        )
        .route("/api/v1/tools/{name}", post(call))
        .route("/api/v1/artifacts/{id}", get(artifact))
        .route("/api/v1/openapi.json", get(|| async { Json(openapi()) }))
        .with_state(service)
        .nest_service("/mcp", mcp)
        .layer(middleware::from_fn_with_state(
            Arc::new(Access { token, port }),
            authorize,
        ))
}
async fn models(State(service): State<Service>) -> std::result::Result<Json<Value>, Error> {
    service
        .call("models_list".into(), json!({}))
        .await
        .map(Json)
}
async fn call(
    State(service): State<Service>,
    Path(name): Path<String>,
    Json(args): Json<Value>,
) -> std::result::Result<Json<Value>, Error> {
    service.call(name, args).await.map(Json)
}
async fn artifact(
    State(service): State<Service>,
    Path(id): Path<String>,
) -> std::result::Result<Response, Error> {
    let artifact = service.artifact(&id)?;
    Ok((
        [(header::CONTENT_TYPE, artifact.mime)],
        artifact.bytes.as_ref().clone(),
    )
        .into_response())
}
pub fn openapi() -> Value {
    let mut paths = serde_json::Map::new();
    for tool in tools() {
        let name = tool["name"].as_str().unwrap();
        paths.insert(format!("/api/v1/tools/{name}"),json!({"post":{"operationId":name,"description":tool["description"],"requestBody":{"required":true,"content":{"application/json":{"schema":tool["inputSchema"]}}},"responses":{"200":{"description":"Operation result","content":{"application/json":{"schema":{"type":"object"}}}},"400":{"description":"Invalid arguments"},"401":{"description":"Authentication required"},"404":{"description":"Model, object or tool missing"},"409":{"description":"Stale revision or unavailable desktop"},"429":{"description":"Busy; retry later"},"500":{"description":"Operation failed"}}}}));
    }
    for (path, description) in [
        ("health", "Server status"),
        ("models", "Loaded models"),
        ("tools", "Tool schemas"),
        ("openapi.json", "OpenAPI description"),
    ] {
        paths.insert(
            format!("/api/v1/{path}"),
            json!({"get":{"responses":{"200":{"description":description}}}}),
        );
    }
    paths.insert("/api/v1/artifacts/{id}".into(),json!({"get":{"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}],"responses":{"200":{"description":"Export bytes"},"404":{"description":"Expired or unknown artifact"}}}}));
    json!({"openapi":"3.1.0","info":{"title":"OpenAEC Native BIM API","version":env!("CARGO_PKG_VERSION")},"security":[{"bearerAuth":[]}],"components":{"securitySchemes":{"bearerAuth":{"type":"http","scheme":"bearer"}}},"paths":paths})
}
