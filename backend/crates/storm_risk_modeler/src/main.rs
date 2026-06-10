mod analysis;

use axum::Router;
use maritime_common::config::AppConfig;
use maritime_common::db;
use tower_http::cors::CorsLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics::{counter, histogram};
use std::time::Instant;

#[tokio::main]
async fn main() {
    let config = AppConfig::load_or_default();

    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| {
            "storm_risk_modeler=info,tower_http=info,axum=info".to_string()
        }))
        .with_target(true)
        .with_level(true)
        .json()
        .init();

    let prometheus_port = config.storm_risk_modeler.port + 1000;
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], prometheus_port))
        .install()
        .expect("Failed to install Prometheus exporter");

    counter!("storm_risk_modeler_startups_total").increment(1);

    let pool = db::create_pool(&config.database.url, config.database.max_connections)
        .await
        .expect("Failed to create database pool");

    tracing::info!(
        port = config.storm_risk_modeler.port,
        metrics_port = prometheus_port,
        "StormRiskModeler service starting"
    );

    let app = Router::new()
        .route("/api/storm-risk", axum::routing::get(analysis::get_storm_risk))
        .route_layer(axum::middleware::from_fn(metrics_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new().gzip(true))
        .layer(CorsLayer::permissive())
        .with_state((pool, config.storm_risk_modeler.clone()));

    let addr = format!("0.0.0.0:{}", config.storm_risk_modeler.port);
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
