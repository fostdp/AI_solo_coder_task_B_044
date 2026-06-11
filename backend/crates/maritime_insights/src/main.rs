use std::sync::Arc;
use std::net::SocketAddr;

use axum::{
    Router,
    routing::get,
    http::StatusCode,
    Json,
    extract::{Query, State},
};
use tower_http::cors::CorsLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use prometheus::{Encoder, TextEncoder, IntCounterVec, HistogramVec, Opts, HistogramOpts, Registry};
use tracing::{info, error};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};
use rand::rngs::StdRng;
use rand::SeedableRng;

use maritime_common::config::AppConfig;
use maritime_common::db::create_db_pool;
use maritime_common::models::InsightsQuery;

use port_rise_fall as port_rise_fall_mod;
use route_simulator as route_simulator_mod;
use goods_spread_network as goods_spread_network_mod;
use modern_shipping_comparator as modern_shipping_comparator_mod;

mod handlers;

use handlers::*;

#[derive(Clone)]
struct AppState {
    db_pool: sqlx::PgPool,
    config: Arc<AppConfig>,
    rng: Arc<std::sync::Mutex<StdRng>>,
    http_requests_total: IntCounterVec,
    http_request_duration: HistogramVec,
    regression_worker: Arc<Option<std::sync::mpsc::SyncSender<port_rise_fall_mod::regression_service::RegressionRequest>>>,
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

async fn metrics_handler(State(state): State<AppState>) -> Result<String, StatusCode> {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    let registry = Registry::new();
    registry.register(Box::new(state.http_requests_total.clone()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    registry.register(Box::new(state.http_request_duration.clone()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let metric_families = registry.gather();
    encoder.encode(&metric_families, &mut buffer)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    String::from_utf8(buffer).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_port_rise_fall(
    State(state): State<AppState>,
    Query(query): Query<InsightsQuery>,
) -> Result<Json<port_rise_fall_mod::PortRiseFallResponse>, StatusCode> {
    state.http_requests_total
        .with_label_values(&["GET", "/api/insights/port-rise-fall"])
        .inc();
    let timer = state.http_request_duration
        .with_label_values(&["GET", "/api/insights/port-rise-fall"])
        .start_timer();

    let config_clone = state.config.maritime_insights.clone();
    let pool = state.db_pool.clone();
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(port_rise_fall_mod::get_port_rise_fall_analysis(
            &pool,
            &config_clone,
            query.year_start,
            query.year_end,
            query.port_id,
            query.region,
        ))
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    timer.observe_duration();
    Ok(Json(result))
}

async fn get_route_planning(
    State(state): State<AppState>,
    Query(query): Query<InsightsQuery>,
) -> Result<Json<route_simulator_mod::RoutePlanningResponse>, StatusCode> {
    state.http_requests_total
        .with_label_values(&["GET", "/api/insights/route-planning"])
        .inc();
    let timer = state.http_request_duration
        .with_label_values(&["GET", "/api/insights/route-planning"])
        .start_timer();

    let dep_id = query.departure_port_id.ok_or(StatusCode::BAD_REQUEST)?;
    let arr_id = query.arrival_port_id.ok_or(StatusCode::BAD_REQUEST)?;
    let season = query.season.clone().unwrap_or_else(|| "summer".to_string());
    let ship_type = query.ship_type.clone().unwrap_or_else(|| "merchant_round_ship".to_string());

    let route_config = state.config.maritime_insights.route_planning.clone();
    let pool = state.db_pool.clone();
    let optimized = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(route_simulator_mod::plan_optimal_route(
            &pool,
            &route_config,
            dep_id,
            arr_id,
            &season,
            &ship_type,
        ))
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
      .ok_or(StatusCode::NOT_FOUND)?;

    let historical = route_simulator_mod::get_historical_route(
        &state.db_pool,
        dep_id,
        arr_id,
        &season,
    ).await;

    let comparison = route_simulator_mod::compare_routes(&optimized, historical.as_ref());

    let response = route_simulator_mod::RoutePlanningResponse {
        optimized_route: optimized,
        historical_route: historical,
        comparison,
    };

    timer.observe_duration();
    Ok(Json(response))
}

async fn get_cargo_spread(
    State(state): State<AppState>,
    Query(query): Query<InsightsQuery>,
) -> Result<Json<goods_spread_network_mod::CargoSpreadResponse>, StatusCode> {
    state.http_requests_total
        .with_label_values(&["GET", "/api/insights/cargo-spread"])
        .inc();
    let timer = state.http_request_duration
        .with_label_values(&["GET", "/api/insights/cargo-spread"])
        .start_timer();

    let cargo_type = query.cargo_type.as_deref().unwrap_or("spices");
    let year_start = query.year_start.unwrap_or(-1000);
    let year_end = query.year_end.unwrap_or(1800);

    let result = goods_spread_network_mod::analyze_cargo_spread(
        &state.db_pool,
        &state.config.maritime_insights.cargo_spread,
        cargo_type,
        year_start,
        year_end,
    ).await;

    timer.observe_duration();
    Ok(Json(result))
}

async fn get_modern_comparison(
    State(state): State<AppState>,
    Query(query): Query<InsightsQuery>,
) -> Result<Json<modern_shipping_comparator_mod::ModernComparisonResponse>, StatusCode> {
    state.http_requests_total
        .with_label_values(&["GET", "/api/insights/modern-comparison"])
        .inc();
    let timer = state.http_request_duration
        .with_label_values(&["GET", "/api/insights/modern-comparison"])
        .start_timer();

    let result = modern_shipping_comparator_mod::get_modern_comparison(
        &state.db_pool,
        &state.config.maritime_insights.modern_comparison,
        &query,
    ).await;

    timer.observe_duration();

    match result {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            error!("Failed to get modern comparison: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn spawn_metrics_server(
    state: AppState,
    metrics_port: u16,
) {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], metrics_port));
    info!("Metrics server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(fmt::layer().json())
        .with(EnvFilter::from_default_env())
        .init();

    let config = Arc::new(maritime_common::config::load_config()?);
    let pool = create_db_pool(&config.database).await?;

    info!("Maritime Insights service starting on port {}", config.maritime_insights.port);
    info!("Delegated modules: port_rise_fall, route_simulator, goods_spread_network, modern_shipping_comparator");

    let regression_worker = port_rise_fall_mod::regression_service::start_regression_worker(
        config.maritime_insights.panel_regression.clone(),
    );

    let http_requests_total = IntCounterVec::new(
        Opts::new("http_requests_total", "Total HTTP requests"),
        &["method", "path"],
    ).unwrap();

    let http_request_duration = HistogramVec::new(
        HistogramOpts::new("http_request_duration_seconds", "HTTP request duration in seconds"),
        &["method", "path"],
    ).unwrap();

    let rng = Arc::new(std::sync::Mutex::new(StdRng::seed_from_u64(42)));

    let state = AppState {
        db_pool: pool.clone(),
        config: config.clone(),
        rng,
        http_requests_total: http_requests_total.clone(),
        http_request_duration: http_request_duration.clone(),
        regression_worker: Arc::new(Some(regression_worker)),
    };

    let metrics_state = state.clone();
    let metrics_port = config.maritime_insights.metrics_port;
    tokio::spawn(async move {
        spawn_metrics_server(metrics_state, metrics_port).await;
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/insights/port-rise-fall", get(get_port_rise_fall))
        .route("/api/insights/route-planning", get(get_route_planning))
        .route("/api/insights/cargo-spread", get(get_cargo_spread))
        .route("/api/insights/modern-comparison", get(get_modern_comparison))
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.maritime_insights.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Maritime Insights server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
