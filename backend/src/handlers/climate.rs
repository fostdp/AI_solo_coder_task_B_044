use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::models::ClimatePeriod;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateQuery {
    pub year: Option<i32>,
    pub period_start: Option<i32>,
    pub period_end: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateResponse {
    pub periods: Vec<ClimatePeriod>,
}

pub async fn get_climate_periods(
    State(pool): State<PgPool>,
    Query(params): Query<ClimateQuery>,
) -> Json<ClimateResponse> {
    let periods = if let Some(year) = params.year {
        sqlx::query_as!(
            ClimatePeriod,
            "SELECT id, period_start, period_end, avg_temperature, avg_wind_speed, \
             avg_rainfall, storm_frequency, nao_index, description \
             FROM climate_periods WHERE period_start <= $1 AND period_end >= $1",
            year
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else if let (Some(ps), Some(pe)) = (params.period_start, params.period_end) {
        sqlx::query_as!(
            ClimatePeriod,
            "SELECT id, period_start, period_end, avg_temperature, avg_wind_speed, \
             avg_rainfall, storm_frequency, nao_index, description \
             FROM climate_periods WHERE period_start >= $1 AND period_end <= $2 \
             ORDER BY period_start",
            ps, pe
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            ClimatePeriod,
            "SELECT id, period_start, period_end, avg_temperature, avg_wind_speed, \
             avg_rainfall, storm_frequency, nao_index, description \
             FROM climate_periods ORDER BY period_start"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    };

    Json(ClimateResponse { periods })
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CurrentData {
    pub id: i32,
    pub name: String,
    pub period_id: i32,
    pub season: String,
    pub direction_deg: Option<f64>,
    pub speed_knots: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentsResponse {
    pub currents: Vec<CurrentData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CurrentQuery {
    pub period_id: Option<i32>,
    pub season: Option<String>,
}

pub async fn get_currents(
    State(pool): State<PgPool>,
    Query(params): Query<CurrentQuery>,
) -> Json<CurrentsResponse> {
    let currents = if let (Some(pid), Some(ref season)) = (params.period_id, params.season) {
        sqlx::query_as!(
            CurrentData,
            "SELECT id, name, period_id, season, direction_deg, speed_knots \
             FROM ocean_currents WHERE period_id = $1 AND season = $2",
            pid, season
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else if let Some(pid) = params.period_id {
        sqlx::query_as!(
            CurrentData,
            "SELECT id, name, period_id, season, direction_deg, speed_knots \
             FROM ocean_currents WHERE period_id = $1",
            pid
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            CurrentData,
            "SELECT id, name, period_id, season, direction_deg, speed_knots \
             FROM ocean_currents LIMIT 200"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    };

    Json(CurrentsResponse { currents })
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WindData {
    pub id: i32,
    pub period_id: i32,
    pub season: String,
    pub region: String,
    pub avg_direction_deg: Option<f64>,
    pub avg_speed_knots: Option<f64>,
    pub variability: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindsResponse {
    pub winds: Vec<WindData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WindQuery {
    pub period_id: Option<i32>,
    pub season: Option<String>,
}

pub async fn get_winds(
    State(pool): State<PgPool>,
    Query(params): Query<WindQuery>,
) -> Json<WindsResponse> {
    let winds = if let (Some(pid), Some(ref season)) = (params.period_id, params.season) {
        sqlx::query_as!(
            WindData,
            "SELECT id, period_id, season, region, avg_direction_deg, avg_speed_knots, variability \
             FROM wind_fields WHERE period_id = $1 AND season = $2",
            pid, season
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else if let Some(pid) = params.period_id {
        sqlx::query_as!(
            WindData,
            "SELECT id, period_id, season, region, avg_direction_deg, avg_speed_knots, variability \
             FROM wind_fields WHERE period_id = $1",
            pid
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            WindData,
            "SELECT id, period_id, season, region, avg_direction_deg, avg_speed_knots, variability \
             FROM wind_fields LIMIT 200"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    };

    Json(WindsResponse { winds })
}
