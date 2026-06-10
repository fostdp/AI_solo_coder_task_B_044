use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::models::{VoyageDetail, VoyageQuery, VoyageRecord, Port};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoyagesResponse {
    pub voyages: Vec<VoyageDetail>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortsResponse {
    pub ports: Vec<PortGeoJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortGeoJson {
    pub id: i32,
    pub name: String,
    pub name_zh: Option<String>,
    pub region: Option<String>,
    pub lat: f64,
    pub lon: f64,
}

pub async fn get_ports(State(pool): State<PgPool>) -> Json<PortsResponse> {
    let rows = sqlx::query_as!(
        Port,
        "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon FROM ports ORDER BY id"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let ports: Vec<PortGeoJson> = rows
        .into_iter()
        .map(|p| PortGeoJson {
            id: p.id,
            name: p.name,
            name_zh: p.name_zh,
            region: p.region,
            lat: p.lat.unwrap_or(0.0),
            lon: p.lon.unwrap_or(0.0),
        })
        .collect();

    Json(PortsResponse { ports })
}

pub async fn get_voyages(
    State(pool): State<PgPool>,
    Query(params): Query<VoyageQuery>,
) -> Json<VoyagesResponse> {
    let year_start = params.year_start.unwrap_or(-1000);
    let year_end = params.year_end.unwrap_or(1800);

    let mut conditions = vec![
        "v.voyage_year >= $1".to_string(),
        "v.voyage_year <= $2".to_string(),
    ];

    let mut param_idx = 3;
    let season_param: Option<String> = if let Some(ref s) = params.season {
        conditions.push(&format!("v.season = ${}", param_idx));
        param_idx += 1;
        Some(s.clone())
    } else {
        None
    };

    let cargo_param: Option<String> = if let Some(ref c) = params.cargo_type {
        conditions.push(&format!("v.cargo_type = ${}", param_idx));
        param_idx += 1;
        Some(c.clone())
    } else {
        None
    };

    let ship_param: Option<String> = if let Some(ref s) = params.ship_type {
        conditions.push(&format!("v.ship_type = ${}", param_idx));
        param_idx += 1;
        Some(s.clone())
    } else {
        None
    };

    let storm_param: Option<bool> = if let Some(s) = params.encountered_storm {
        conditions.push(&format!("v.encountered_storm = ${}", param_idx));
        param_idx += 1;
        Some(s)
    } else {
        None
    };

    let region_param: Option<String> = if let Some(ref r) = params.region {
        conditions.push(&format!("dp.region = ${}", param_idx));
        param_idx += 1;
        Some(r.clone())
    } else {
        None
    };

    let where_clause = conditions.join(" AND ");

    let count_sql = format!(
        "SELECT COUNT(*) as count FROM voyage_records v JOIN ports dp ON v.departure_port_id = dp.id WHERE {}",
        where_clause
    );

    let query_sql = format!(
        "SELECT v.id, v.departure_port_id, v.arrival_port_id, v.voyage_year, v.season, \
         v.ship_type, v.cargo_type, v.encountered_storm, v.route_points, \
         dp.name as departure_name, dp.name_zh as departure_name_zh, \
         ap.name as arrival_name, ap.name_zh as arrival_name_zh, \
         ST_Y(dp.geom) as dep_lat, ST_X(dp.geom) as dep_lon, \
         ST_Y(ap.geom) as arr_lat, ST_X(ap.geom) as arr_lon \
         FROM voyage_records v \
         JOIN ports dp ON v.departure_port_id = dp.id \
         JOIN ports ap ON v.arrival_port_id = ap.id \
         WHERE {} \
         ORDER BY v.voyage_year \
         LIMIT 2000",
        where_clause
    );

    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(year_start)
        .bind(year_end)
        .bind(&season_param)
        .bind(&cargo_param)
        .bind(&ship_param)
        .bind(storm_param)
        .bind(&region_param)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let rows = sqlx::query(&query_sql)
        .bind(year_start)
        .bind(year_end)
        .bind(&season_param)
        .bind(&cargo_param)
        .bind(&ship_param)
        .bind(storm_param)
        .bind(&region_param)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let voyages: Vec<VoyageDetail> = rows
        .iter()
        .map(|row| VoyageDetail {
            id: row.get("id"),
            departure_port: row.get("departure_name"),
            departure_port_zh: row.get("departure_name_zh"),
            arrival_port: row.get("arrival_name"),
            arrival_port_zh: row.get("arrival_name_zh"),
            departure_lat: row.get("dep_lat"),
            departure_lon: row.get("dep_lon"),
            arrival_lat: row.get("arr_lat"),
            arrival_lon: row.get("arr_lon"),
            voyage_year: row.get("voyage_year"),
            season: row.get("season"),
            ship_type: row.get("ship_type"),
            cargo_type: row.get("cargo_type"),
            encountered_storm: row.get("encountered_storm"),
            route_points: row.get("route_points"),
        })
        .collect();

    Json(VoyagesResponse { voyages, total })
}

pub async fn get_voyage_by_id(
    State(pool): State<PgPool>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<Json<VoyageDetail>, axum::http::StatusCode> {
    let row = sqlx::query(
        "SELECT v.id, v.departure_port_id, v.arrival_port_id, v.voyage_year, v.season, \
         v.ship_type, v.cargo_type, v.encountered_storm, v.route_points, \
         dp.name as departure_name, dp.name_zh as departure_name_zh, \
         ap.name as arrival_name, ap.name_zh as arrival_name_zh, \
         ST_Y(dp.geom) as dep_lat, ST_X(dp.geom) as dep_lon, \
         ST_Y(ap.geom) as arr_lat, ST_X(ap.geom) as arr_lon \
         FROM voyage_records v \
         JOIN ports dp ON v.departure_port_id = dp.id \
         JOIN ports ap ON v.arrival_port_id = ap.id \
         WHERE v.id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    match row {
        Some(row) => Ok(Json(VoyageDetail {
            id: row.get("id"),
            departure_port: row.get("departure_name"),
            departure_port_zh: row.get("departure_name_zh"),
            arrival_port: row.get("arrival_name"),
            arrival_port_zh: row.get("arrival_name_zh"),
            departure_lat: row.get("dep_lat"),
            departure_lon: row.get("dep_lon"),
            arrival_lat: row.get("arr_lat"),
            arrival_lon: row.get("arr_lon"),
            voyage_year: row.get("voyage_year"),
            season: row.get("season"),
            ship_type: row.get("ship_type"),
            cargo_type: row.get("cargo_type"),
            encountered_storm: row.get("encountered_storm"),
            route_points: row.get("route_points"),
        })),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}
