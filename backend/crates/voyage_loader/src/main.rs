mod handlers;

use axum::Router;
use maritime_common::config::AppConfig;
use maritime_common::db;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let config = AppConfig::load_or_default();

    tracing_subscriber::fmt()
        .with_env_filter("voyage_loader=debug,tower_http=debug")
        .init();

    let pool = db::create_pool(&config.database.url, config.database.max_connections)
        .await
        .expect("Failed to create database pool");

    tracing::info!("VoyageLoader service starting on port {}", config.voyage_loader.port);

    let app = Router::new()
        .route("/api/ports", axum::routing::get(handlers::get_ports))
        .route("/api/voyages", axum::routing::get(handlers::get_voyages))
        .route("/api/voyages/:id", axum::routing::get(handlers::get_voyage_by_id))
        .route("/api/climate/periods", axum::routing::get(handlers::get_climate_periods))
        .route("/api/climate/currents", axum::routing::get(handlers::get_currents))
        .route("/api/climate/winds", axum::routing::get(handlers::get_winds))
        .route("/api/stats", axum::routing::get(handlers::get_stats))
        .layer(CorsLayer::permissive())
        .with_state(pool);

    let addr = format!("0.0.0.0:{}", config.voyage_loader.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind port");
    axum::serve(listener, app).await.expect("Server error");
}
