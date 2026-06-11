use maritime_common::config::ModernComparisonConfig;
use maritime_common::models::*;
use sqlx::PgPool;
use std::collections::HashMap;

fn ship_risk_factor(ship_type: &str) -> f64 {
    match ship_type {
        "container_ship" => 0.6,
        "bulk_carrier" => 0.65,
        "tanker" => 0.7,
        "cargo_ship" => 0.75,
        "passenger_ship" => 0.55,
        "fishing_vessel" => 0.9,
        "sailing_vessel" => 0.95,
        _ => 0.75,
    }
}

fn dynamic_ship_risk_factor(ship: Option<&ModernShip>, ship_type: &str) -> f64 {
    let base_factor = ship_risk_factor(ship_type);
    if let Some(s) = ship {
        let tonnage_factor = (s.gross_tonnage / 50000.0).min(1.0).max(0.0);
        let length_factor = (s.length_m / 300.0).min(1.0).max(0.0);
        let speed_factor = (s.max_speed_knots / 25.0).min(1.0).max(0.0);
        let size_bonus = 0.15 * (tonnage_factor * 0.5 + length_factor * 0.3 + speed_factor * 0.2);
        (base_factor - size_bonus).max(0.3)
    } else {
        base_factor
    }
}

fn risk_level(score: f64) -> &'static str {
    if score >= 0.7 {
        "very_high"
    } else if score >= 0.5 {
        "high"
    } else if score >= 0.3 {
        "medium"
    } else if score >= 0.1 {
        "low"
    } else {
        "very_low"
    }
}

fn bayesian_smooth(raw_score: f64, global_avg: f64, sample_weight: f64, prior_weight: f64) -> f64 {
    (sample_weight * raw_score + prior_weight * global_avg) / (sample_weight + prior_weight)
}

fn calculate_single_modern_risk(
    forecast: &ModernWeatherForecast,
    visibility_nm: f64,
    ship: Option<&ModernShip>,
    ship_type: &str,
    config: &ModernComparisonConfig,
    global_avg_risk: f64,
) -> f64 {
    let wind_risk = (forecast.wind_speed_knots / 30.0).min(1.0);
    let wave_risk = (forecast.wave_height_m / 5.0).min(1.0);
    let visibility_risk = ((10.0 - visibility_nm) / 10.0).clamp(0.0, 1.0);
    let storm_prob = forecast.storm_probability.clamp(0.0, 1.0);

    let ship_factor = dynamic_ship_risk_factor(ship, ship_type);

    let wind_weight = 0.3;
    let wave_weight = 0.25;
    let visibility_weight = 0.15;
    let storm_weight = 0.3;

    let weighted_sum = wind_weight * wind_risk
        + wave_weight * wave_risk
        + visibility_weight * visibility_risk
        + storm_weight * storm_prob;

    let base_risk = weighted_sum * ship_factor;

    let smoothed = bayesian_smooth(base_risk, global_avg_risk, 5.0, 2.0);

    let tech_factor = 1.0 - config.tech_improvement_factor * config.weather_forecast_accuracy;
    let adjusted = smoothed * config.modern_risk_multiplier * tech_factor;

    adjusted.clamp(0.0, 1.0)
}

fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 {
        return 0.0;
    }

    let nf = n as f64;
    let sum_x: f64 = x.iter().take(n).sum();
    let sum_y: f64 = y.iter().take(n).sum();
    let sum_x_sq: f64 = x.iter().take(n).map(|v| v * v).sum();
    let sum_y_sq: f64 = y.iter().take(n).map(|v| v * v).sum();
    let sum_xy: f64 = x
        .iter()
        .take(n)
        .zip(y.iter().take(n))
        .map(|(a, b)| a * b)
        .sum();

    let numerator = nf * sum_xy - sum_x * sum_y;
    let denominator_x = (nf * sum_x_sq - sum_x * sum_x).sqrt();
    let denominator_y = (nf * sum_y_sq - sum_y * sum_y).sqrt();

    let denominator = denominator_x * denominator_y;
    if denominator.abs() < 1e-10 {
        0.0
    } else {
        (numerator / denominator).clamp(-1.0, 1.0)
    }
}

fn haversine_distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    r * c
}

fn route_offset_distance(route1: &[Vec<f64>], route2: &[Vec<f64>]) -> f64 {
    if route1.is_empty() || route2.is_empty() {
        return 0.0;
    }

    let mut total_dist = 0.0;
    let count = route1.len().min(route2.len());

    for i in 0..count {
        let p1 = &route1[i];
        let p2 = &route2[i];
        if p1.len() >= 2 && p2.len() >= 2 {
            total_dist += haversine_distance_km(p1[1], p1[0], p2[1], p2[0]);
        }
    }

    if count > 0 {
        total_dist / count as f64
    } else {
        0.0
    }
}

struct RouteComparisonDetail {
    pub departure_port_id: i32,
    pub arrival_port_id: i32,
    pub ancient_risk: f64,
    pub modern_risk: f64,
    pub risk_difference: f64,
    pub offset_distance_km: f64,
    pub ancient_route: Vec<Vec<f64>>,
    pub modern_route: Vec<Vec<f64>>,
}

fn compute_region_risks(
    risks: &[StormRiskResult],
    port_region_map: &HashMap<i32, String>,
) -> HashMap<String, Vec<f64>> {
    let mut region_risks: HashMap<String, Vec<f64>> = HashMap::new();

    for r in risks {
        let region = port_region_map
            .get(&r.departure_port_id)
            .or_else(|| port_region_map.get(&r.arrival_port_id))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        region_risks.entry(region).or_default().push(r.risk_score);
    }

    region_risks
}

fn compute_modern_region_risks(
    forecasts: &[ModernWeatherForecast],
    ship: Option<&ModernShip>,
    config: &ModernComparisonConfig,
    global_avg: f64,
    ship_type: &str,
) -> HashMap<String, Vec<f64>> {
    let mut region_risks: HashMap<String, Vec<f64>> = HashMap::new();

    for f in forecasts {
        let risk = calculate_single_modern_risk(f, 10.0, ship, ship_type, config, global_avg);
        region_risks.entry(f.region.clone()).or_default().push(risk);
    }

    region_risks
}

fn find_most_dangerous_region(region_risks: &HashMap<String, Vec<f64>>) -> String {
    region_risks
        .iter()
        .map(|(region, risks)| {
            let avg = if risks.is_empty() {
                0.0
            } else {
                risks.iter().sum::<f64>() / risks.len() as f64
            };
            (region.clone(), avg)
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(r, _)| r)
        .unwrap_or_else(|| "unknown".to_string())
}

fn generate_heatmap_from_risks(
    risks: &[StormRiskResult],
    port_coords: &HashMap<i32, (f64, f64)>,
    frequency_map: &HashMap<(i32, i32), i32>,
) -> Vec<StormHeatmapPoint> {
    let mut heatmap = Vec::new();

    for r in risks {
        let dep_coord = port_coords.get(&r.departure_port_id);
        let arr_coord = port_coords.get(&r.arrival_port_id);

        if let (Some(&(dep_lat, dep_lon)), Some(&(arr_lat, arr_lon))) = (dep_coord, arr_coord) {
            let freq = frequency_map
                .get(&(r.departure_port_id, r.arrival_port_id))
                .copied()
                .unwrap_or(1) as f64;

            let mid_lat = (dep_lat + arr_lat) / 2.0;
            let mid_lon = (dep_lon + arr_lon) / 2.0;

            let intensity = r.risk_score * freq.log1p().max(1.0);

            heatmap.push(StormHeatmapPoint {
                lat: mid_lat,
                lon: mid_lon,
                intensity,
            });

            heatmap.push(StormHeatmapPoint {
                lat: dep_lat,
                lon: dep_lon,
                intensity: r.risk_score * 0.5 * freq.log1p().max(1.0),
            });

            heatmap.push(StormHeatmapPoint {
                lat: arr_lat,
                lon: arr_lon,
                intensity: r.risk_score * 0.5 * freq.log1p().max(1.0),
            });
        }
    }

    heatmap
}

fn generate_modern_heatmap(
    forecasts: &[ModernWeatherForecast],
    ship: Option<&ModernShip>,
    config: &ModernComparisonConfig,
    global_avg: f64,
    ship_type: &str,
) -> Vec<StormHeatmapPoint> {
    forecasts
        .iter()
        .map(|f| {
            let risk = calculate_single_modern_risk(f, 10.0, ship, ship_type, config, global_avg);
            StormHeatmapPoint {
                lat: f.lat,
                lon: f.lon,
                intensity: risk,
            }
        })
        .collect()
}

fn compute_route_comparisons(
    ancient_risks: &[StormRiskResult],
    modern_forecasts: &[ModernWeatherForecast],
    ancient_routes: &HashMap<(i32, i32), Vec<Vec<f64>>>,
    port_coords: &HashMap<i32, (f64, f64)>,
    ship: Option<&ModernShip>,
    config: &ModernComparisonConfig,
    global_avg_modern: f64,
    ship_type: &str,
) -> Vec<RouteComparisonDetail> {
    let mut comparisons = Vec::new();

    for ancient in ancient_risks {
        let key = (ancient.departure_port_id, ancient.arrival_port_id);
        let ancient_route = ancient_routes.get(&key).cloned().unwrap_or_default();

        let dep_coord = port_coords.get(&ancient.departure_port_id);
        let arr_coord = port_coords.get(&ancient.arrival_port_id);

        let modern_route = if let (Some(&(dep_lat, dep_lon)), Some(&(arr_lat, arr_lon))) =
            (dep_coord, arr_coord)
        {
            vec![vec![dep_lon, dep_lat], vec![arr_lon, arr_lat]]
        } else {
            Vec::new()
        };

        let nearest_forecast = modern_forecasts.iter().min_by(|a, b| {
            let ref_lat = dep_coord.map(|c| c.0).unwrap_or(a.lat);
            let ref_lon = dep_coord.map(|c| c.1).unwrap_or(a.lon);
            let da = haversine_distance_km(ref_lat, ref_lon, a.lat, a.lon);
            let db = haversine_distance_km(ref_lat, ref_lon, b.lat, b.lon);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        let modern_risk = if let Some(f) = nearest_forecast {
            calculate_single_modern_risk(f, 10.0, ship, ship_type, config, global_avg_modern)
        } else {
            global_avg_modern
        };

        let offset_distance = route_offset_distance(&ancient_route, &modern_route);

        comparisons.push(RouteComparisonDetail {
            departure_port_id: ancient.departure_port_id,
            arrival_port_id: ancient.arrival_port_id,
            ancient_risk: ancient.risk_score,
            modern_risk,
            risk_difference: ancient.risk_score - modern_risk,
            offset_distance_km: offset_distance,
            ancient_route,
            modern_route,
        });
    }

    comparisons.sort_by(|a, b| {
        b.risk_difference
            .abs()
            .partial_cmp(&a.risk_difference.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    comparisons
}

pub async fn get_modern_comparison(
    pool: &PgPool,
    config: &ModernComparisonConfig,
    query: &InsightsQuery,
) -> ModernComparisonResponse {
    let model_type = query
        .model_type
        .clone()
        .unwrap_or_else(|| "logistic_regression".to_string());
    let ship_type = query
        .ship_type
        .clone()
        .unwrap_or_else(|| "cargo_ship".to_string());
    let region_filter = query.region.clone();

    let ports = sqlx::query_as!(
        Port,
        "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon FROM ports"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut port_region_map: HashMap<i32, String> = HashMap::new();
    let mut port_coords: HashMap<i32, (f64, f64)> = HashMap::new();
    for p in &ports {
        if let Some(ref r) = p.region {
            port_region_map.insert(p.id, r.clone());
        }
        if let (Some(lat), Some(lon)) = (p.lat, p.lon) {
            port_coords.insert(p.id, (lat, lon));
        }
    }

    let voyages = sqlx::query_as!(
        VoyageRecord,
        "SELECT id, departure_port_id, arrival_port_id, voyage_year, season, \
         ship_type, cargo_type, encountered_storm, route_points, created_at \
         FROM voyage_records"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut route_frequency: HashMap<(i32, i32), i32> = HashMap::new();
    let mut ancient_routes: HashMap<(i32, i32), Vec<Vec<f64>>> = HashMap::new();

    for v in &voyages {
        let key = (v.departure_port_id, v.arrival_port_id);
        *route_frequency.entry(key).or_insert(0) += 1;

        if !ancient_routes.contains_key(&key) {
            if let Some(ref pts) = v.route_points {
                if let Some(arr) = pts.as_array() {
                    let route: Vec<Vec<f64>> = arr
                        .iter()
                        .filter_map(|pt| pt.as_array())
                        .filter(|coord| coord.len() >= 2)
                        .map(|coord| {
                            vec![
                                coord[0].as_f64().unwrap_or(0.0),
                                coord[1].as_f64().unwrap_or(0.0),
                            ]
                        })
                        .collect();
                    if !route.is_empty() {
                        ancient_routes.insert(key, route);
                    }
                }
            }
        }
    }

    let ancient_risks_query = if let Some(region) = region_filter.as_ref() {
        sqlx::query_as!(
            StormRiskResult,
            "SELECT sr.departure_port_id, sr.arrival_port_id, sr.departure_port_name, \
             sr.arrival_port_name, sr.season, sr.risk_score, sr.sample_size, \
             sr.model_type, sr.confidence \
             FROM storm_risk_results sr \
             JOIN ports p ON sr.departure_port_id = p.id \
             WHERE sr.model_type = $1 AND p.region = $2",
            model_type,
            region
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            StormRiskResult,
            "SELECT departure_port_id, arrival_port_id, departure_port_name, \
             arrival_port_name, season, risk_score, sample_size, model_type, confidence \
             FROM storm_risk_results WHERE model_type = $1",
            model_type
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let weather_forecasts = if let Some(region) = region_filter.as_ref() {
        sqlx::query_as!(
            ModernWeatherForecast,
            "SELECT region, wind_direction_deg, wind_speed_knots, wave_height_m, \
             current_direction_deg, current_speed_knots, storm_probability, lat, lon \
             FROM modern_weather_forecasts WHERE region = $1",
            region
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            ModernWeatherForecast,
            "SELECT region, wind_direction_deg, wind_speed_knots, wave_height_m, \
             current_direction_deg, current_speed_knots, storm_probability, lat, lon \
             FROM modern_weather_forecasts"
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let modern_ships = sqlx::query_as!(
        ModernShip,
        "SELECT id, ship_name, mmsi, ship_type, gross_tonnage, length_m, beam_m, \
         max_speed_knots, flag FROM modern_ships WHERE ship_type = $1 LIMIT 1",
        ship_type
    )
    .fetch_optional(pool)
    .await
    .unwrap_or_default();

    let avg_ancient_risk = if ancient_risks_query.is_empty() {
        0.0
    } else {
        ancient_risks_query
            .iter()
            .map(|r| r.risk_score)
            .sum::<f64>()
            / ancient_risks_query.len() as f64
    };

    let ship_ref = modern_ships.as_ref();
    let global_avg_modern = if weather_forecasts.is_empty() {
        0.1
    } else {
        weather_forecasts
            .iter()
            .map(|f| {
                let wind = (f.wind_speed_knots / 30.0).min(1.0);
                let wave = (f.wave_height_m / 5.0).min(1.0);
                let storm = f.storm_probability.clamp(0.0, 1.0);
                let base = (0.3 * wind + 0.25 * wave + 0.15 * 0.0 + 0.3 * storm)
                    * dynamic_ship_risk_factor(ship_ref, &ship_type);
                let tech_factor =
                    1.0 - config.tech_improvement_factor * config.weather_forecast_accuracy;
                base * config.modern_risk_multiplier * tech_factor
            })
            .sum::<f64>()
            / weather_forecasts.len() as f64
    };

    let modern_risks: Vec<ModernRiskResult> = ancient_risks_query
        .iter()
        .map(|ancient| {
            let dep_coord = port_coords.get(&ancient.departure_port_id);
            let arr_coord = port_coords.get(&ancient.arrival_port_id);

            let nearest_forecast = weather_forecasts.iter().min_by(|a, b| {
                let ref_lat = dep_coord.map(|c| c.0).unwrap_or(a.lat);
                let ref_lon = dep_coord.map(|c| c.1).unwrap_or(a.lon);
                let da = haversine_distance_km(ref_lat, ref_lon, a.lat, a.lon);
                let db = haversine_distance_km(ref_lat, ref_lon, b.lat, b.lon);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });

            let risk_score = if let Some(f) = nearest_forecast {
                calculate_single_modern_risk(
                    f,
                    10.0,
                    ship_ref,
                    &ship_type,
                    config,
                    global_avg_modern,
                )
            } else {
                global_avg_modern
            };

            let route_points = if let (Some(&(dep_lat, dep_lon)), Some(&(arr_lat, arr_lon))) =
                (dep_coord, arr_coord)
            {
                vec![vec![dep_lon, dep_lat], vec![arr_lon, arr_lat]]
            } else {
                Vec::new()
            };

            ModernRiskResult {
                departure_port_id: ancient.departure_port_id,
                arrival_port_id: ancient.arrival_port_id,
                risk_score,
                risk_level: risk_level(risk_score).to_string(),
                model_type: "modern_forecast".to_string(),
                ancient_comparison_score: ancient.risk_score - risk_score,
                route_points,
                estimated_delay_hours: risk_score * 24.0,
                alternative_route_suggestion: if risk_score > 0.5 {
                    Some("Consider alternative route with lower storm probability".to_string())
                } else {
                    None
                },
            }
        })
        .collect();

    let avg_modern_risk = if modern_risks.is_empty() {
        0.0
    } else {
        modern_risks.iter().map(|r| r.risk_score).sum::<f64>() / modern_risks.len() as f64
    };

    let risk_reduction_pct = if avg_ancient_risk > 0.0 {
        ((avg_ancient_risk - avg_modern_risk) / avg_ancient_risk) * 100.0
    } else {
        0.0
    };

    let high_risk_routes_ancient = ancient_risks_query
        .iter()
        .filter(|r| r.risk_score > 0.3)
        .count() as i32;

    let high_risk_routes_modern = modern_risks.iter().filter(|r| r.risk_score > 0.3).count() as i32;

    let ancient_region_risks = compute_region_risks(&ancient_risks_query, &port_region_map);
    let modern_region_risks = compute_modern_region_risks(
        &weather_forecasts,
        ship_ref,
        config,
        global_avg_modern,
        &ship_type,
    );

    let most_dangerous_region_ancient = find_most_dangerous_region(&ancient_region_risks);
    let most_dangerous_region_modern = find_most_dangerous_region(&modern_region_risks);

    let matched_ancient: Vec<f64> = ancient_risks_query.iter().map(|r| r.risk_score).collect();
    let matched_modern: Vec<f64> = modern_risks.iter().map(|r| r.risk_score).collect();
    let correlation_coefficient = pearson_correlation(&matched_ancient, &matched_modern);

    let comparison_summary = RiskComparisonSummary {
        avg_ancient_risk,
        avg_modern_risk,
        risk_reduction_pct,
        high_risk_routes_ancient,
        high_risk_routes_modern,
        most_dangerous_region_ancient,
        most_dangerous_region_modern,
        correlation_coefficient,
    };

    let heatmap_ancient =
        generate_heatmap_from_risks(&ancient_risks_query, &port_coords, &route_frequency);

    let heatmap_modern = generate_modern_heatmap(
        &weather_forecasts,
        ship_ref,
        config,
        global_avg_modern,
        &ship_type,
    );

    let _route_comparisons = compute_route_comparisons(
        &ancient_risks_query,
        &weather_forecasts,
        &ancient_routes,
        &port_coords,
        ship_ref,
        config,
        global_avg_modern,
        &ship_type,
    );

    ModernComparisonResponse {
        ancient_risks: ancient_risks_query,
        modern_risks,
        comparison_summary,
        heatmap_ancient,
        heatmap_modern,
    }
}
