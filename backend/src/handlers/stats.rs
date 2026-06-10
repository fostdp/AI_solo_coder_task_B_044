use axum::extract::State;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_voyages: i64,
    pub total_ports: i64,
    pub storm_encounters: i64,
    pub min_year: i32,
    pub max_year: i32,
    pub cargo_distribution: Vec<CargoCount>,
    pub ship_distribution: Vec<ShipCount>,
    pub season_distribution: Vec<SeasonCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoCount {
    pub cargo_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipCount {
    pub ship_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonCount {
    pub season: String,
    pub count: i64,
}

pub async fn get_stats(State(pool): State<PgPool>) -> Json<StatsResponse> {
    let total_voyages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM voyage_records")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let total_ports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ports")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let storm_encounters: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM voyage_records WHERE encountered_storm = true")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let year_range: (i32, i32) = sqlx::query_as("SELECT MIN(voyage_year), MAX(voyage_year) FROM voyage_records")
        .fetch_one(&pool)
        .await
        .map(|row: (Option<i32>, Option<i32>)| {
            (row.0.unwrap_or(-1000), row.1.unwrap_or(1800))
        })
        .unwrap_or((-1000, 1800));

    let cargo_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT cargo_type, COUNT(*) as count FROM voyage_records GROUP BY cargo_type ORDER BY count DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let ship_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT ship_type, COUNT(*) as count FROM voyage_records GROUP BY ship_type ORDER BY count DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let season_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT season, COUNT(*) as count FROM voyage_records GROUP BY season ORDER BY count DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Json(StatsResponse {
        total_voyages,
        total_ports,
        storm_encounters,
        min_year: year_range.0,
        max_year: year_range.1,
        cargo_distribution: cargo_rows.into_iter().map(|(t, c)| CargoCount { cargo_type: t, count: c }).collect(),
        ship_distribution: ship_rows.into_iter().map(|(t, c)| ShipCount { ship_type: t, count: c }).collect(),
        season_distribution: season_rows.into_iter().map(|(t, c)| SeasonCount { season: t, count: c }).collect(),
    })
}
