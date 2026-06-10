use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::Deserialize;
use sqlx::PgPool;

use crate::analysis::network::TradeNetwork;
use crate::models::{NetworkAnalysisResponse, NetworkQuery, Port, VoyageRecord};

pub async fn get_network_analysis(
    State(pool): State<PgPool>,
    Query(params): Query<NetworkQuery>,
) -> Json<NetworkAnalysisResponse> {
    let year_start = params.year_start.unwrap_or(-1000);
    let year_end = params.year_end.unwrap_or(1800);

    let voyages = sqlx::query_as!(
        VoyageRecord,
        "SELECT id, departure_port_id, arrival_port_id, voyage_year, season, \
         ship_type, cargo_type, encountered_storm, route_points, created_at \
         FROM voyage_records WHERE voyage_year >= $1 AND voyage_year <= $2",
        year_start, year_end
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let ports = sqlx::query_as!(
        Port,
        "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon FROM ports"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let network = TradeNetwork::from_voyages(&voyages, &ports);
    let (nodes, edges) = network.analyze(year_start, year_end);

    Json(NetworkAnalysisResponse {
        nodes,
        edges,
        period_start: year_start,
        period_end: year_end,
    })
}
