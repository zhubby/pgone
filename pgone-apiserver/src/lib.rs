pub mod grpc;
mod http;
mod proxy;

use std::net::SocketAddr;

use anyhow::Result;
use axum::http::StatusCode;
use axum::{
    Router,
    response::IntoResponse,
    routing::{get, post},
};
use pgone_util::log;
use std::str::FromStr;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::broadcast;
use tower::{Layer, ServiceBuilder};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{Level, info, warn};

use once_cell::sync::{Lazy, OnceCell};
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
use prometheus_client::registry::Registry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use axum::Json;
use axum::response::Html;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use pgone_storage::blocking::StorageBlocking;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use utoipa::OpenApi;

static OAUTH_STATE: OnceCell<Arc<Mutex<HashMap<String, String>>>> = OnceCell::new();
static STORAGE: OnceCell<Arc<StorageBlocking>> = OnceCell::new();

#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    pub log_level: String,
    pub enable_otel: bool,
    pub json_log: bool,
    pub service_name: String,
    pub http_bind: String,
    pub http_port: u16,
    pub grpc_bind: String,
    pub grpc_port: u16,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            enable_otel: false,
            json_log: false,
            service_name: "pgone-apiserver".to_string(),
            http_bind: "127.0.0.1".to_string(),
            http_port: 8765,
            grpc_bind: "127.0.0.1".to_string(),
            grpc_port: 50051,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct Labels {
    method: String,
    path: String,
    status: String,
}

#[utoipa::path(get, path = "/health", responses((status = 200, description = "OK")))]
async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

#[utoipa::path(get, path = "/metrics", responses((status = 200, description = "Prometheus metrics")))]
async fn metrics() -> impl IntoResponse {
    let mut buffer = String::new();
    if let Some(reg) = REGISTRY.get() {
        encode(&mut buffer, reg.as_ref()).unwrap_or_default();
    }
    (StatusCode::OK, buffer)
}

#[derive(OpenApi)]
#[openapi(
    paths(health, metrics, oauth_github_start, oauth_github_callback, auth_me, http::execute_proxy),
    tags(
        (name = "pgone-apiserver", description = "HTTP APIs for pgone"),
        (name = "proxy", description = "SQL proxy APIs"),
    )
)]
struct ApiDoc;

async fn docs_html() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Swagger UI</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  <style> body { margin: 0; } #swagger-ui { height: 100vh; } </style>
  </head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js" crossorigin></script>
  <script>
    window.ui = SwaggerUIBundle({
      url: '/api-docs/openapi.json',
      dom_id: '#swagger-ui',
      presets: [SwaggerUIBundle.presets.apis],
      layout: 'BaseLayout'
    });
  </script>
</body>
</html>"#,
    )
}

pub async fn serve(
    addr: SocketAddr,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    // metrics registry
    let mut registry = Registry::default();

    registry.register(
        "http_requests_total",
        "Total number of HTTP requests",
        HTTP_REQUESTS_TOTAL.clone(),
    );

    registry.register(
        "http_request_duration_seconds",
        "HTTP request latency in seconds",
        HTTP_REQUEST_DURATION.clone(),
    );

    let registry = Arc::new(registry);
    let _ = REGISTRY.set(registry);

    // middleware: tracing + simple metrics
    let trace_layer =
        TraceLayer::new_for_http().on_response(DefaultOnResponse::new().level(Level::INFO));

    let metrics_layer = MetricsLayer;

    // set storage
    let storage = StorageBlocking::open_default().await?;
    let _ = STORAGE.set(Arc::new(storage));

    // init oauth state map
    let _ = OAUTH_STATE.set(Arc::new(Mutex::new(HashMap::new())));

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route(
            "/api-docs/openapi.json",
            get(|| async { axum::Json(ApiDoc::openapi()) }),
        )
        .route("/docs", get(docs_html))
        .route("/oauth/github/start", post(oauth_github_start))
        .route("/oauth/github/callback", get(oauth_github_callback))
        .route("/auth/me", get(auth_me))
        .merge(http::router())
        .layer(
            ServiceBuilder::new()
                .layer(trace_layer)
                .layer(metrics_layer),
        );

    let listener = TcpListener::bind(addr).await?;
    info!("listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

pub async fn run(config: ApiServerConfig) -> Result<()> {
    log::init_log(log::LogConfig {
        level: log::LogLevel::from_str(&config.log_level)?,
        enable_otel: config.enable_otel,
        json_format: config.json_log,
        service_name: Some(config.service_name.clone()),
    })?;

    info!("pgone-apiserver 启动中...");

    let http_addr: SocketAddr = format!("{}:{}", config.http_bind, config.http_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("无效的 HTTP 地址: {}", e))?;

    let grpc_addr: SocketAddr = format!("{}:{}", config.grpc_bind, config.grpc_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("无效的 gRPC 地址: {}", e))?;

    info!(
        http_addr = %http_addr,
        grpc_addr = %grpc_addr,
        "服务器地址配置完成"
    );

    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let create_shutdown_signal = || {
        let mut shutdown_rx = shutdown_tx.subscribe();
        async move {
            let _ = shutdown_rx.recv().await;
        }
    };

    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
            warn!("收到 Ctrl+C 信号，开始优雅关闭...");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
            warn!("收到 SIGTERM 信号，开始优雅关闭...");
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        let _ = shutdown_tx_clone.send(());
    });

    info!("启动 HTTP 和 gRPC 服务器...");

    tokio::select! {
        result = serve(http_addr, create_shutdown_signal()) => {
            match result {
                Ok(_) => {
                    info!("HTTP 服务器已关闭");
                }
                Err(e) => {
                    tracing::error!(error = ?e, "HTTP 服务器错误");
                    return Err(e);
                }
            }
        }
        result = grpc::serve_grpc(grpc_addr, create_shutdown_signal()) => {
            match result {
                Ok(_) => {
                    info!("gRPC 服务器已关闭");
                }
                Err(e) => {
                    tracing::error!(error = ?e, "gRPC 服务器错误");
                    return Err(e);
                }
            }
        }
    }

    info!("所有服务器已关闭，正在清理资源...");

    if config.enable_otel {
        log::shutdown_otel();
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct MetricsLayer;

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { inner }
    }
}

#[derive(Clone)]
struct MetricsService<S> {
    inner: S,
}

impl<ReqBody, S> tower::Service<axum::http::Request<ReqBody>> for MetricsService<S>
where
    S: tower::Service<axum::http::Request<ReqBody>, Response = axum::response::Response>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<ReqBody>) -> Self::Future {
        let start = std::time::Instant::now();
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let mut svc = self.inner.clone();
        Box::pin(async move {
            let res = svc.call(req).await?;
            let status = res.status().as_u16().to_string();
            let labels = Labels {
                method,
                path,
                status,
            };
            HTTP_REQUESTS_TOTAL.get_or_create(&labels).inc();
            let elapsed = start.elapsed();
            HTTP_REQUEST_DURATION
                .get_or_create(&labels)
                .observe(elapsed.as_secs_f64());
            Ok(res)
        })
    }
}

fn default_histogram() -> Histogram {
    Histogram::new(exponential_buckets(0.005, 2.0, 14))
}

static REGISTRY: OnceCell<Arc<Registry>> = OnceCell::new();
static HTTP_REQUESTS_TOTAL: Lazy<Family<Labels, Counter>> = Lazy::new(Family::default);
static HTTP_REQUEST_DURATION: Lazy<Family<Labels, Histogram>> =
    Lazy::new(|| Family::new_with_constructor(default_histogram));

#[derive(Deserialize, utoipa::ToSchema)]
struct StartReq {}
#[derive(Serialize, utoipa::ToSchema)]
struct StartResp {
    authorize_url: String,
}

#[utoipa::path(post, path = "/oauth/github/start", responses((status = 200)))]
async fn oauth_github_start(Json(_req): Json<StartReq>) -> impl IntoResponse {
    let client_id = env::var("GITHUB_CLIENT_ID").unwrap_or_default();
    let redirect_uri = env::var("OAUTH_REDIRECT")
        .unwrap_or_else(|_| "http://127.0.0.1:8765/oauth/github/callback".to_string());
    if client_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "missing client id").into_response();
    }
    // generate state and code_verifier
    let mut state_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut state_bytes);
    let state = URL_SAFE_NO_PAD.encode(state_bytes);
    let mut verifier_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut verifier_bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    if let Some(map) = OAUTH_STATE.get() {
        map.lock()
            .await
            .insert(state.clone(), code_verifier.clone());
    }
    let scope = "read:user user:email";
    let authorize_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        client_id,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(scope),
        state,
        code_challenge
    );
    Json(StartResp { authorize_url }).into_response()
}

#[utoipa::path(get, path = "/oauth/github/callback", responses((status = 200)))]
async fn oauth_github_callback(
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(code) = params.get("code").cloned() else {
        return (StatusCode::BAD_REQUEST, "missing code").into_response();
    };
    let Some(state) = params.get("state").cloned() else {
        return (StatusCode::BAD_REQUEST, "missing state").into_response();
    };
    let redirect_uri = env::var("OAUTH_REDIRECT")
        .unwrap_or_else(|_| "http://127.0.0.1:8765/oauth/github/callback".to_string());
    let client_id = match env::var("GITHUB_CLIENT_ID") {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "missing client id").into_response(),
    };
    let client_secret = match env::var("GITHUB_CLIENT_SECRET") {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "missing secret").into_response(),
    };
    let code_verifier = if let Some(map) = OAUTH_STATE.get() {
        map.lock().await.remove(&state)
    } else {
        None
    };
    let Some(code_verifier) = code_verifier else {
        return (StatusCode::BAD_REQUEST, "invalid state").into_response();
    };

    let client = reqwest::Client::new();
    // exchange token
    let token_resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", code_verifier.as_str()),
        ])
        .send()
        .await;
    let Ok(resp) = token_resp else {
        return (StatusCode::BAD_GATEWAY, "token request failed").into_response();
    };
    let Ok(token_json) = resp.json::<serde_json::Value>().await else {
        return (StatusCode::BAD_GATEWAY, "token parse failed").into_response();
    };
    let Some(access_token) = token_json
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    else {
        return (StatusCode::BAD_GATEWAY, "no access token").into_response();
    };
    let scope = token_json
        .get("scope")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // fetch user info
    let user_resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "pgone")
        .send()
        .await;
    let Ok(user_json) = user_resp else {
        return (StatusCode::BAD_GATEWAY, "user request failed").into_response();
    };
    let Ok(user_json) = user_json.json::<serde_json::Value>().await else {
        return (StatusCode::BAD_GATEWAY, "user parse failed").into_response();
    };
    let id = user_json
        .get("id")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .unwrap_or_else(uuid);
    let login = user_json
        .get("login")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let name = user_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let avatar_url = user_json
        .get("avatar_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // maybe email
    let email = user_json
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let email = if email.is_none() {
        let emails_resp = client
            .get("https://api.github.com/user/emails")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "pgone")
            .send()
            .await;
        if let Ok(emails_resp) = emails_resp {
            if let Ok(v) = emails_resp.json::<serde_json::Value>().await {
                v.as_array()
                    .and_then(|arr| {
                        arr.iter()
                            .find(|e| e.get("primary").and_then(|p| p.as_bool()).unwrap_or(false))
                    })
                    .and_then(|e| e.get("email").and_then(|x| x.as_str()))
                    .map(|s| s.to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        email
    };

    let now = now_ts();
    let user = pgone_storage::models::AuthUser {
        id: id.clone(),
        login,
        name,
        avatar_url,
        email,
        created_at: now,
        updated_at: now,
    };
    let token = pgone_storage::models::AuthToken {
        id: uuid(),
        user_id: id.clone(),
        provider: "github".to_string(),
        access_token: access_token.clone(),
        scope,
        created_at: now,
        updated_at: now,
    };
    if let Some(st) = STORAGE.get() {
        let _ = st.upsert_auth_user(&user).await;
        let _ = st.insert_auth_token(&token).await;
    }

    Html("<html><body>Login success, you can close this window.</body></html>").into_response()
}

#[utoipa::path(get, path = "/auth/me", responses((status = 200)))]
async fn auth_me() -> impl IntoResponse {
    if let Some(st) = STORAGE.get()
        && let Ok(Some(u)) = st.get_current_user().await
    {
        return Json(u).into_response();
    }
    (StatusCode::NOT_FOUND, "").into_response()
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn uuid() -> String {
    format!("id-{}", now_ts())
}
