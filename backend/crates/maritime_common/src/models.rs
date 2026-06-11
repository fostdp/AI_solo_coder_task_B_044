use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

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
pub struct PortAlias {
    pub id: i32,
    pub port_id: i32,
    pub alias_name: String,
    pub alias_name_zh: Option<String>,
    pub period_start: Option<i32>,
    pub period_end: Option<i32>,
    pub language: Option<String>,
    pub source: Option<String>,
}

pub struct PortNameIndex {
    name_to_id: HashMap<String, i32>,
    normalized_to_id: HashMap<String, i32>,
}

impl PortNameIndex {
    pub fn build(ports: &[Port], aliases: &[PortAlias]) -> Self {
        let mut name_to_id = HashMap::new();
        let mut normalized_to_id = HashMap::new();

        for p in ports {
            name_to_id.insert(p.name.to_lowercase(), p.id);
            if let Some(ref zh) = p.name_zh {
                name_to_id.insert(zh.clone(), p.id);
            }
            normalized_to_id.insert(Self::normalize(&p.name), p.id);
        }

        for a in aliases {
            name_to_id.insert(a.alias_name.to_lowercase(), a.port_id);
            if let Some(ref zh) = a.alias_name_zh {
                name_to_id.insert(zh.clone(), a.port_id);
            }
            normalized_to_id.insert(Self::normalize(&a.alias_name), a.port_id);
        }

        PortNameIndex { name_to_id, normalized_to_id }
    }

    pub fn lookup(&self, name: &str) -> Option<i32> {
        if let Some(&id) = self.name_to_id.get(&name.to_lowercase()) {
            return Some(id);
        }
        if let Some(&id) = self.name_to_id.get(name) {
            return Some(id);
        }
        let norm = Self::normalize(name);
        if let Some(&id) = self.normalized_to_id.get(&norm) {
            return Some(id);
        }
        self.fuzzy_lookup(name)
    }

    fn normalize(name: &str) -> String {
        let lower = name.to_lowercase();
        lower
            .replace("ae", "e")
            .replace("oe", "e")
            .replace("ph", "f")
            .replace("th", "t")
            .replace("ou", "u")
            .replace("-", "")
            .replace(" ", "")
            .replace("'", "")
    }

    fn fuzzy_lookup(&self, name: &str) -> Option<i32> {
        let query_lower = name.to_lowercase();
        let query_norm = Self::normalize(name);
        let query_chars: Vec<char> = query_lower.chars().collect();
        let mut best_id: Option<i32> = None;
        let mut best_score: f64 = 0.6;

        for (key, &id) in &self.name_to_id {
            let key_chars: Vec<char> = key.chars().collect();
            let score = Self::jaro_winkler(&query_chars, &key_chars);
            if score > best_score {
                best_score = score;
                best_id = Some(id);
            }
        }

        if best_id.is_none() {
            for (key, &id) in &self.normalized_to_id {
                let key_chars: Vec<char> = key.chars().collect();
                let query_norm_chars: Vec<char> = query_norm.chars().collect();
                let score = Self::jaro_winkler(&query_norm_chars, &key_chars);
                if score > best_score {
                    best_score = score;
                    best_id = Some(id);
                }
            }
        }

        best_id
    }

    fn jaro_winkler(s1: &[char], s2: &[char]) -> f64 {
        if s1.is_empty() && s2.is_empty() { return 1.0; }
        if s1.is_empty() || s2.is_empty() { return 0.0; }

        let max_dist = (s1.len().max(s2.len()) / 2).max(1).saturating_sub(1);
        let mut s1_matches = vec![false; s1.len()];
        let mut s2_matches = vec![false; s2.len()];

        let mut matches = 0usize;
        let mut transpositions = 0usize;

        for (i, &c1) in s1.iter().enumerate() {
            let start = if i > max_dist { i - max_dist } else { 0 };
            let end = (i + max_dist + 1).min(s2.len());
            for j in start..end {
                if !s2_matches[j] && c1 == s2[j] {
                    s1_matches[i] = true;
                    s2_matches[j] = true;
                    matches += 1;
                    break;
                }
            }
        }

        if matches == 0 { return 0.0; }

        let mut k = 0usize;
        for (i, &matched) in s1_matches.iter().enumerate() {
            if matched {
                while !s2_matches[k] { k += 1; }
                if s1[i] != s2[k] { transpositions += 1; }
                k += 1;
            }
        }

        let jaro = (matches as f64 / s1.len() as f64
            + matches as f64 / s2.len() as f64
            + (matches - transpositions / 2) as f64 / matches as f64) / 3.0;

        let prefix_len = s1.iter().zip(s2.iter()).take(4).filter(|(a, b)| a == b).count();
        jaro + prefix_len as f64 * 0.1 * (1.0 - jaro)
    }
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

// ============ 港口兴衰分析 ============

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HistoricalEvent {
    pub id: i32,
    pub event_name: String,
    pub event_name_zh: Option<String>,
    pub event_type: String,
    pub region: Option<String>,
    pub start_year: i32,
    pub end_year: Option<i32>,
    pub severity: Option<f64>,
    pub description: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PortYearlyFlow {
    pub port_id: i32,
    pub year: i32,
    pub total_flow: i32,
    pub departure_count: i32,
    pub arrival_count: i32,
    pub storm_count: i32,
    pub storm_rate: Option<f64>,
    pub unique_cargo_types: i32,
    pub unique_destinations: i32,
    pub flow_rank: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionCoefficient {
    pub variable: String,
    pub variable_zh: String,
    pub coefficient: f64,
    pub standard_error: f64,
    pub t_statistic: f64,
    pub p_value: f64,
    pub is_significant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelRegressionResult {
    pub port_id: i32,
    pub port_name: String,
    pub dependent_var: String,
    pub model_type: String,
    pub period_start: i32,
    pub period_end: i32,
    pub coefficients: Vec<RegressionCoefficient>,
    pub r_squared: f64,
    pub adj_r_squared: f64,
    pub f_statistic: f64,
    pub p_value: f64,
    pub n_observations: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrangerCausalityResult {
    pub port_id: i32,
    pub cause_variable: String,
    pub cause_variable_zh: String,
    pub effect_variable: String,
    pub effect_variable_zh: String,
    pub lag_order: i32,
    pub f_statistic: f64,
    pub p_value: f64,
    pub is_significant: bool,
    pub direction: String,
    pub period_start: i32,
    pub period_end: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRiseFallResponse {
    pub port_flows: Vec<PortYearlyFlow>,
    pub historical_events: Vec<HistoricalEvent>,
    pub regression_results: Vec<PanelRegressionResult>,
    pub granger_results: Vec<GrangerCausalityResult>,
    pub factor_weights: Vec<FactorWeight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorWeight {
    pub factor: String,
    pub factor_zh: String,
    pub avg_coefficient: f64,
    pub significance_rate: f64,
    pub importance_rank: i32,
}

// ============ 航线规划 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePlanningResult {
    pub departure_port_id: i32,
    pub arrival_port_id: i32,
    pub departure_port_name: String,
    pub arrival_port_name: String,
    pub season: String,
    pub ship_type: String,
    pub method: String,
    pub route_points: Vec<Vec<f64>>,
    pub distance_nautical_miles: f64,
    pub estimated_days: f64,
    pub avg_speed_knots: f64,
    pub storm_risk: f64,
    pub historical_deviation_pct: f64,
    pub historical_correlation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePlanningResponse {
    pub optimized_route: RoutePlanningResult,
    pub historical_route: Option<RoutePlanningResult>,
    pub comparison: RouteComparison,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteComparison {
    pub distance_diff_pct: f64,
    pub time_diff_pct: f64,
    pub risk_diff_pct: f64,
    pub similarity_score: f64,
    pub waypoints_matched: i32,
    pub total_waypoints: i32,
}

// ============ 货物传播 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoSpreadRecord {
    pub cargo_type: String,
    pub from_port_id: i32,
    pub to_port_id: i32,
    pub voyage_year: i32,
    pub spread_direction: String,
    pub quantity_estimate: f64,
    pub cultural_significance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechDiffusionPath {
    pub id: i32,
    pub tech_name: String,
    pub tech_name_zh: String,
    pub tech_category: String,
    pub origin_port_id: i32,
    pub origin_port_name: String,
    pub spread_route: Vec<i32>,
    pub estimated_start_year: i32,
    pub estimated_end_year: i32,
    pub diffusion_speed_km_yr: f64,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoSpreadResponse {
    pub cargo_type: String,
    pub spread_records: Vec<CargoSpreadRecord>,
    pub tech_diffusions: Vec<TechDiffusionPath>,
    pub spread_network: SpreadNetwork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadNetwork {
    pub nodes: Vec<SpreadNode>,
    pub edges: Vec<SpreadEdge>,
    pub origin_ports: Vec<i32>,
    pub hub_ports: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadNode {
    pub port_id: i32,
    pub port_name: String,
    pub first_received_year: i32,
    pub adoption_level: f64,
    pub betweenness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadEdge {
    pub from_port_id: i32,
    pub to_port_id: i32,
    pub flow_volume: f64,
    pub first_spread_year: i32,
}

// ============ 现代航运对比 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernShip {
    pub id: i32,
    pub ship_name: String,
    pub mmsi: String,
    pub ship_type: String,
    pub gross_tonnage: f64,
    pub length_m: f64,
    pub beam_m: f64,
    pub max_speed_knots: f64,
    pub flag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernWeatherForecast {
    pub region: String,
    pub wind_direction_deg: f64,
    pub wind_speed_knots: f64,
    pub wave_height_m: f64,
    pub current_direction_deg: f64,
    pub current_speed_knots: f64,
    pub storm_probability: f64,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernRiskResult {
    pub departure_port_id: i32,
    pub arrival_port_id: i32,
    pub risk_score: f64,
    pub risk_level: String,
    pub model_type: String,
    pub ancient_comparison_score: f64,
    pub route_points: Vec<Vec<f64>>,
    pub estimated_delay_hours: f64,
    pub alternative_route_suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernComparisonResponse {
    pub ancient_risks: Vec<StormRiskResult>,
    pub modern_risks: Vec<ModernRiskResult>,
    pub comparison_summary: RiskComparisonSummary,
    pub heatmap_ancient: Vec<StormHeatmapPoint>,
    pub heatmap_modern: Vec<StormHeatmapPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskComparisonSummary {
    pub avg_ancient_risk: f64,
    pub avg_modern_risk: f64,
    pub risk_reduction_pct: f64,
    pub high_risk_routes_ancient: i32,
    pub high_risk_routes_modern: i32,
    pub most_dangerous_region_ancient: String,
    pub most_dangerous_region_modern: String,
    pub correlation_coefficient: f64,
}

// ============ 通用查询参数 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightsQuery {
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
    pub port_id: Option<i32>,
    pub region: Option<String>,
    pub model_type: Option<String>,
    pub analysis_type: Option<String>,
    pub cargo_type: Option<String>,
    pub season: Option<String>,
    pub ship_type: Option<String>,
    pub departure_port_id: Option<i32>,
    pub arrival_port_id: Option<i32>,
    pub lag_order: Option<i32>,
}
