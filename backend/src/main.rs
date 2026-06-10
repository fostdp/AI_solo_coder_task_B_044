mod analysis;
mod db;
mod handlers;
mod models;

use axum::{
    routing::{get, get as post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("ancient_maritime_backend=debug,tower_http=debug")
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/ancient_maritime".to_string());

    let pool = db::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");

    tracing::info!("Database pool created successfully");

    let app = Router::new()
        .route("/api/ports", get(handlers::voyages::get_ports))
        .route("/api/voyages", get(handlers::voyages::get_voyages))
        .route("/api/voyages/:id", get(handlers::voyages::get_voyage_by_id))
        .route("/api/climate/periods", get(handlers::climate::get_climate_periods))
        .route("/api/climate/currents", get(handlers::climate::get_currents))
        .route("/api/climate/winds", get(handlers::climate::get_winds))
        .route("/api/network", get(handlers::network::get_network_analysis))
        .route("/api/storm-risk", get(handlers::storm::get_storm_risk))
        .route("/api/stats", get(handlers::stats::get_stats))
        .fallback_service(ServeDir::new("../frontend").append_index_html_on_directories(true))
        .layer(CorsLayer::permissive())
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");

    tracing::info!("Server running on http://localhost:3000");
    axum::serve(listener, app).await.expect("Server error");
}
