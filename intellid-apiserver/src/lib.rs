use std::net::SocketAddr;

use anyhow::Result;
use axum::{Router, routing::get, response::IntoResponse};
use http::StatusCode;
use tokio::net::TcpListener;
use tokio::signal;
use tower::{ServiceBuilder, Layer};
use tower_http::trace::{TraceLayer, DefaultOnResponse};
use tracing::{info, Level};

use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
use prometheus_client::registry::Registry;
use std::sync::Arc;
use once_cell::sync::{Lazy, OnceCell};

use utoipa::OpenApi;
use axum::response::Html;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct Labels {
    method: String,
    path: String,
    status: String,
}

#[utoipa::path(get, path = "/health", responses((status = 200, description = "OK")))]
async fn health() -> impl IntoResponse { (StatusCode::OK, "ok") }

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
    paths(health, metrics),
    tags(
        (name = "intellid-apiserver", description = "HTTP APIs for intellid"),
    )
)]
struct ApiDoc;

async fn docs_html() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
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
</html>"#)
}

pub async fn serve(addr: SocketAddr) -> Result<()> {
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
    let trace_layer = TraceLayer::new_for_http()
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    let metrics_layer = MetricsLayer;

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/api-docs/openapi.json", get(|| async { axum::Json(ApiDoc::openapi()) }))
        .route("/docs", get(docs_html))
        .layer(ServiceBuilder::new()
            .layer(trace_layer)
            .layer(metrics_layer));

    let listener = TcpListener::bind(addr).await?;
    info!("listening on {}", addr);

    let shutdown = async move {
        let _ = signal::ctrl_c().await;
    };

    axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;
    Ok(())
}

#[derive(Clone, Copy)]
struct MetricsLayer;

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;
    fn layer(&self, inner: S) -> Self::Service { MetricsService { inner } }
}

#[derive(Clone)]
struct MetricsService<S> { inner: S }

impl<ReqBody, S> tower::Service<axum::http::Request<ReqBody>> for MetricsService<S>
where
    S: tower::Service<axum::http::Request<ReqBody>, Response = axum::response::Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
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
            let labels = Labels { method, path, status };
            HTTP_REQUESTS_TOTAL.get_or_create(&labels).inc();
            let elapsed = start.elapsed();
            HTTP_REQUEST_DURATION.get_or_create(&labels).observe(elapsed.as_secs_f64());
            Ok(res)
        })
    }
}

fn default_histogram() -> Histogram {
    Histogram::new(exponential_buckets(0.005, 2.0, 14))
}

static REGISTRY: OnceCell<Arc<Registry>> = OnceCell::new();
static HTTP_REQUESTS_TOTAL: Lazy<Family<Labels, Counter>> = Lazy::new(|| Family::default());
static HTTP_REQUEST_DURATION: Lazy<Family<Labels, Histogram>> = Lazy::new(|| Family::new_with_constructor(default_histogram));


