mod analysis;

use axum::Router;
use maritime_common::config::AppConfig;
use maritime_common::db;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let config = AppConfig::load_or_default();

    tracing_subscriber::fmt()
        .with_env_filter("network_analyzer=debug,tower_http=debug")
        .init();

    let pool = db::create_pool(&config.database.url, config.database.max_connections)
        .await
        .expect("Failed to create database pool");

    tracing::info!("NetworkAnalyzer service starting on port {}", config.network_analyzer.port);

    let app = Router::new()
        .route("/api/network", axum::routing::get(analysis::get_network_analysis))
        .layer(CorsLayer::permissive())
        .with_state((pool, config.network_analyzer.clone()));

    let addr = format!("0.0.0.0:{}", config.network_analyzer.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind port");
    axum::serve(listener, app).await.expect("Server error");
}
