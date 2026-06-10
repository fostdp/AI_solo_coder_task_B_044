use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Port {
    pub id: i32,
    pub name: String,
    pub name_zh: Option<String>,
    pub region: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoyageRecord {
    pub id: i32,
    pub departure_port_id: i32,
    pub arrival_port_id: i32,
    pub voyage_year: i32,
    pub season: String,
    pub ship_type: String,
    pub cargo_type: String,
    pub encountered_storm: bool,
    pub route_points: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoyageDetail {
    pub id: i32,
    pub departure_port: String,
    pub departure_port_zh: Option<String>,
    pub arrival_port: String,
    pub arrival_port_zh: Option<String>,
    pub departure_lat: f64,
    pub departure_lon: f64,
    pub arrival_lat: f64,
    pub arrival_lon: f64,
    pub voyage_year: i32,
    pub season: String,
    pub ship_type: String,
    pub cargo_type: String,
    pub encountered_storm: bool,
    pub route_points: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ClimatePeriod {
    pub id: i32,
    pub period_start: i32,
    pub period_end: i32,
    pub avg_temperature: Option<f64>,
    pub avg_wind_speed: Option<f64>,
    pub avg_rainfall: Option<f64>,
    pub storm_frequency: Option<f64>,
    pub nao_index: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OceanCurrent {
    pub id: i32,
    pub name: String,
    pub period_id: i32,
    pub season: String,
    pub direction_deg: Option<f64>,
    pub speed_knots: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WindField {
    pub id: i32,
    pub period_id: i32,
    pub season: String,
    pub region: String,
    pub avg_direction_deg: Option<f64>,
    pub avg_speed_knots: Option<f64>,
    pub variability: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkResult {
    pub port_id: i32,
    pub port_name: String,
    pub port_name_zh: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub betweenness_centrality: f64,
    pub degree_centrality: f64,
    pub trade_flow: f64,
    pub community_id: i32,
    pub is_hub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StormRiskResult {
    pub departure_port_id: i32,
    pub arrival_port_id: i32,
    pub departure_port_name: String,
    pub arrival_port_name: String,
    pub season: String,
    pub risk_score: f64,
    pub sample_size: i32,
    pub model_type: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoyageQuery {
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
    pub season: Option<String>,
    pub cargo_type: Option<String>,
    pub ship_type: Option<String>,
    pub encountered_storm: Option<bool>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkQuery {
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StormRiskQuery {
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
    pub model_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEdge {
    pub source: i32,
    pub target: i32,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAnalysisResponse {
    pub nodes: Vec<NetworkResult>,
    pub edges: Vec<TradeEdge>,
    pub period_start: i32,
    pub period_end: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StormHeatmapPoint {
    pub lat: f64,
    pub lon: f64,
    pub intensity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StormAnalysisResponse {
    pub risks: Vec<StormRiskResult>,
    pub heatmap: Vec<StormHeatmapPoint>,
    pub model_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_voyages: i64,
    pub total_ports: i64,
    pub storm_encounters: i64,
    pub year_range: (i32, i32),
    pub top_cargo_types: Vec<(String, i64)>,
    pub top_ship_types: Vec<(String, i64)>,
}
