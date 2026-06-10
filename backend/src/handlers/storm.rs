use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::Deserialize;
use sqlx::PgPool;

use crate::analysis::storm;
use crate::models::{StormAnalysisResponse, StormRiskQuery, VoyageRecord, Port};

pub async fn get_storm_risk(
    State(pool): State<PgPool>,
    Query(params): Query<StormRiskQuery>,
) -> Json<StormAnalysisResponse> {
    let year_start = params.year_start.unwrap_or(-1000);
    let year_end = params.year_end.unwrap_or(1800);
    let model_type = params.model_type.unwrap_or_else(|| "logistic_regression".to_string());

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

    let (mut risks, heatmap) = storm::analyze_storm_risk(&voyages, &model_type);

    let port_map: std::collections::HashMap<i32, String> = ports.iter().map(|p| (p.id, p.name.clone())).collect();
    for risk in &mut risks {
        risk.departure_port_name = port_map.get(&risk.departure_port_id).cloned().unwrap_or_default();
        risk.arrival_port_name = port_map.get(&risk.arrival_port_id).cloned().unwrap_or_default();
    }

    Json(StormAnalysisResponse {
        risks,
        heatmap,
        model_type,
    })
}
