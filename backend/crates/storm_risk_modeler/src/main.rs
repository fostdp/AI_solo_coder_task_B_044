mod analysis;

use axum::Router;
use maritime_common::config::AppConfig;
use maritime_common::db;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let config = AppConfig::load_or_default();

    tracing_subscriber::fmt()
        .with_env_filter("storm_risk_modeler=debug,tower_http=debug")
        .init();

    let pool = db::create_pool(&config.database.url, config.database.max_connections)
        .await
        .expect("Failed to create database pool");

    tracing::info!("StormRiskModeler service starting on port {}", config.storm_risk_modeler.port);

    let app = Router::new()
        .route("/api/storm-risk", axum::routing::get(analysis::get_storm_risk))
        .layer(CorsLayer::permissive())
        .with_state((pool, config.storm_risk_modeler.clone()));

    let addr = format!("0.0.0.0:{}", config.storm_risk_modeler.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind port");
    axum::serve(listener, app).await.expect("Server error");
}
