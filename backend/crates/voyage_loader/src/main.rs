mod handlers;

use axum::Router;
use maritime_common::config::AppConfig;
use maritime_common::db;
use tower_http::cors::CorsLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics::{counter, histogram};
use std::time::Instant;

use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let config = AppConfig::load_or_default();

    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| {
            "voyage_loader=info,tower_http=info,axum=info".to_string()
        }))
        .with_target(true)
        .with_level(true)
        .json()
        .init();

    let prometheus_port = config.voyage_loader.port + 1000;
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], prometheus_port))
        .install()
        .expect("Failed to install Prometheus exporter");

    counter!("voyage_loader_startups_total").increment(1);

    let pool = db::create_pool(&config.database.url, config.database.max_connections)
        .await
        .expect("Failed to create database pool");

    tracing::info!(
        port = config.voyage_loader.port,
        metrics_port = prometheus_port,
        "VoyageLoader service starting"
    );

    let frontend_dir = std::env::var("FRONTEND_DIR").unwrap_or_else(|_| "/app/frontend".to_string());
    let serve_dir = ServeDir::new(frontend_dir);

    let api = Router::new()
        .route("/ports", axum::routing::get(handlers::get_ports))
        .route("/voyages", axum::routing::get(handlers::get_voyages))
        .route("/voyages/:id", axum::routing::get(handlers::get_voyage_by_id))
        .route("/climate/periods", axum::routing::get(handlers::get_climate_periods))
        .route("/climate/currents", axum::routing::get(handlers::get_currents))
        .route("/climate/winds", axum::routing::get(handlers::get_winds))
        .route("/stats", axum::routing::get(handlers::get_stats))
        .with_state(pool);

    let app = Router::new()
        .nest_service("/", serve_dir)
        .nest("/api", api)
        .route_layer(axum::middleware::from_fn(metrics_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new().gzip(true))
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", config.voyage_loader.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind port");
    tracing::info!(addr = %addr, "Listening for HTTP requests");
    axum::serve(listener, app).await.expect("Server error");
}

async fn metrics_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = Instant::now();
    let response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status().as_u16();
    counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status.to_string()).increment(1);
    histogram!("http_request_duration_seconds", "method" => method, "path" => path).record(duration.as_secs_f64());
    response
}
