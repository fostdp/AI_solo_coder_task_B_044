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

#[cfg(test)]
mod tests {
    use super::*;
    use maritime_common::config::ModernComparisonConfig;
    use maritime_common::models::*;
    use std::collections::HashMap;

    fn create_test_ship(ship_type: &str, tonnage: f64, length: f64, speed: f64) -> ModernShip {
        ModernShip {
            id: 1,
            ship_name: "Test Ship".to_string(),
            mmsi: "123456789".to_string(),
            ship_type: ship_type.to_string(),
            gross_tonnage: tonnage,
            length_m: length,
            beam_m: 20.0,
            max_speed_knots: speed,
            flag: "Test".to_string(),
        }
    }

    fn create_test_forecast(
        wind_speed: f64,
        wave_height: f64,
        storm_prob: f64,
    ) -> ModernWeatherForecast {
        ModernWeatherForecast {
            region: "test_region".to_string(),
            wind_direction_deg: 180.0,
            wind_speed_knots: wind_speed,
            wave_height_m: wave_height,
            current_direction_deg: 90.0,
            current_speed_knots: 2.0,
            storm_probability: storm_prob,
            lat: 30.0,
            lon: 120.0,
        }
    }

    fn create_test_config() -> ModernComparisonConfig {
        ModernComparisonConfig {
            modern_risk_multiplier: 0.8,
            tech_improvement_factor: 0.3,
            weather_forecast_accuracy: 0.8,
        }
    }

    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }

    mod ship_risk_factor_tests {
        use super::*;

        #[test]
        fn test_ship_risk_factor_range() {
            let ship_types = vec![
                "container_ship",
                "bulk_carrier",
                "tanker",
                "cargo_ship",
                "passenger_ship",
                "fishing_vessel",
                "sailing_vessel",
                "unknown_type",
            ];
            for st in ship_types {
                let risk = ship_risk_factor(st);
                assert!(risk >= 0.3, "{} risk {} should be >= 0.3", st, risk);
                assert!(risk <= 1.0, "{} risk {} should be <= 1.0", st, risk);
            }
        }

        #[test]
        fn test_ship_risk_factor_ordering() {
            let fishing = ship_risk_factor("fishing_vessel");
            let bulk = ship_risk_factor("bulk_carrier");
            let container = ship_risk_factor("container_ship");
            assert!(fishing > bulk, "fishing ({}) > bulk ({})", fishing, bulk);
            assert!(bulk > container, "bulk ({}) > container ({})", bulk, container);
        }

        #[test]
        fn test_ship_risk_factor_unknown() {
            let unknown = ship_risk_factor("unknown_ship_type");
            let cargo = ship_risk_factor("cargo_ship");
            assert_eq!(unknown, cargo);
            assert_eq!(unknown, 0.75);
        }

        #[test]
        fn test_dynamic_ship_risk_factor_large_ship_lower_risk() {
            let ship_type = "cargo_ship";
            let small_ship = create_test_ship(ship_type, 1000.0, 50.0, 10.0);
            let large_ship = create_test_ship(ship_type, 80000.0, 300.0, 25.0);

            let small_risk = dynamic_ship_risk_factor(Some(&small_ship), ship_type);
            let large_risk = dynamic_ship_risk_factor(Some(&large_ship), ship_type);
            let base_risk = dynamic_ship_risk_factor(None, ship_type);

            assert!(large_risk < small_risk, "large ship risk ({}) < small ship risk ({})", large_risk, small_risk);
            assert!(small_risk <= base_risk, "small ship risk ({}) <= base risk ({})", small_risk, base_risk);
            assert!(large_risk <= base_risk, "large ship risk ({}) <= base risk ({})", large_risk, base_risk);
        }

        #[test]
        fn test_dynamic_ship_risk_factor_none_degenerates() {
            let ship_type = "cargo_ship";
            let base = ship_risk_factor(ship_type);
            let dynamic = dynamic_ship_risk_factor(None, ship_type);
            assert_eq!(base, dynamic);
        }

        #[test]
        fn test_dynamic_ship_risk_factor_min_bounded() {
            let ship_type = "cargo_ship";
            let huge_ship = create_test_ship(ship_type, 200000.0, 400.0, 35.0);
            let risk = dynamic_ship_risk_factor(Some(&huge_ship), ship_type);
            assert!(risk >= 0.3, "risk {} should be >= 0.3", risk);
        }
    }

    mod risk_level_tests {
        use super::*;

        #[test]
        fn test_risk_level_boundaries() {
            assert_eq!(risk_level(0.7), "very_high");
            assert_eq!(risk_level(0.5), "high");
            assert_eq!(risk_level(0.3), "medium");
            assert_eq!(risk_level(0.1), "low");
        }

        #[test]
        fn test_risk_level_extremes() {
            assert_eq!(risk_level(0.0), "very_low");
            assert_eq!(risk_level(1.0), "very_high");
        }

        #[test]
        fn test_risk_level_boundary_precise() {
            assert_eq!(risk_level(0.6999999), "high");
            assert_eq!(risk_level(0.7000001), "very_high");
            assert_eq!(risk_level(0.4999999), "medium");
            assert_eq!(risk_level(0.5000001), "high");
            assert_eq!(risk_level(0.2999999), "low");
            assert_eq!(risk_level(0.3000001), "medium");
            assert_eq!(risk_level(0.0999999), "very_low");
            assert_eq!(risk_level(0.1000001), "low");
        }

        #[test]
        fn test_risk_level_mid_values() {
            assert_eq!(risk_level(0.85), "very_high");
            assert_eq!(risk_level(0.6), "high");
            assert_eq!(risk_level(0.4), "medium");
            assert_eq!(risk_level(0.2), "low");
            assert_eq!(risk_level(0.05), "very_low");
        }
    }

    mod bayesian_smooth_tests {
        use super::*;

        #[test]
        fn test_bayesian_smooth_large_sample_weight() {
            let raw = 0.8;
            let global_avg = 0.3;
            let result = bayesian_smooth(raw, global_avg, 1000.0, 1.0);
            assert!(approx_eq(result, raw, 0.01), "result {} should be close to raw {}", result, raw);
        }

        #[test]
        fn test_bayesian_smooth_large_prior_weight() {
            let raw = 0.8;
            let global_avg = 0.3;
            let result = bayesian_smooth(raw, global_avg, 1.0, 1000.0);
            assert!(approx_eq(result, global_avg, 0.01), "result {} should be close to prior {}", result, global_avg);
        }

        #[test]
        fn test_bayesian_smooth_equal_weights() {
            let raw = 0.8;
            let global_avg = 0.4;
            let result = bayesian_smooth(raw, global_avg, 5.0, 5.0);
            let expected = (raw + global_avg) / 2.0;
            assert!(approx_eq(result, expected, 1e-10), "result {} should be average {}", result, expected);
        }

        #[test]
        fn test_bayesian_smooth_identical_values() {
            let result = bayesian_smooth(0.5, 0.5, 3.0, 2.0);
            assert!(approx_eq(result, 0.5, 1e-10));
        }
    }

    mod modern_risk_tests {
        use super::*;

        #[test]
        fn test_calculate_single_modern_risk_wind_increases_risk() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let global_avg = 0.3;

            let low_wind = create_test_forecast(5.0, 2.0, 0.1);
            let high_wind = create_test_forecast(25.0, 2.0, 0.1);

            let low_risk = calculate_single_modern_risk(&low_wind, 10.0, Some(&ship), "cargo_ship", &config, global_avg);
            let high_risk = calculate_single_modern_risk(&high_wind, 10.0, Some(&ship), "cargo_ship", &config, global_avg);

            assert!(high_risk > low_risk, "high wind risk ({}) > low wind risk ({})", high_risk, low_risk);
        }

        #[test]
        fn test_calculate_single_modern_risk_wave_increases_risk() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let global_avg = 0.3;

            let low_wave = create_test_forecast(15.0, 1.0, 0.1);
            let high_wave = create_test_forecast(15.0, 4.0, 0.1);

            let low_risk = calculate_single_modern_risk(&low_wave, 10.0, Some(&ship), "cargo_ship", &config, global_avg);
            let high_risk = calculate_single_modern_risk(&high_wave, 10.0, Some(&ship), "cargo_ship", &config, global_avg);

            assert!(high_risk > low_risk, "high wave risk ({}) > low wave risk ({})", high_risk, low_risk);
        }

        #[test]
        fn test_calculate_single_modern_risk_visibility_decreases_risk() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let global_avg = 0.3;
            let forecast = create_test_forecast(15.0, 2.0, 0.1);

            let low_vis_risk = calculate_single_modern_risk(&forecast, 1.0, Some(&ship), "cargo_ship", &config, global_avg);
            let high_vis_risk = calculate_single_modern_risk(&forecast, 10.0, Some(&ship), "cargo_ship", &config, global_avg);

            assert!(low_vis_risk > high_vis_risk, "low vis risk ({}) > high vis risk ({})", low_vis_risk, high_vis_risk);
        }

        #[test]
        fn test_calculate_single_modern_risk_storm_prob_increases_risk() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let global_avg = 0.3;

            let low_storm = create_test_forecast(15.0, 2.0, 0.1);
            let high_storm = create_test_forecast(15.0, 2.0, 0.9);

            let low_risk = calculate_single_modern_risk(&low_storm, 10.0, Some(&ship), "cargo_ship", &config, global_avg);
            let high_risk = calculate_single_modern_risk(&high_storm, 10.0, Some(&ship), "cargo_ship", &config, global_avg);

            assert!(high_risk > low_risk, "high storm risk ({}) > low storm risk ({})", high_risk, low_risk);
        }

        #[test]
        fn test_calculate_single_modern_risk_bounded() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);

            let extreme_good = create_test_forecast(0.0, 0.0, 0.0);
            let extreme_bad = create_test_forecast(100.0, 20.0, 1.0);

            let good_risk = calculate_single_modern_risk(&extreme_good, 20.0, Some(&ship), "cargo_ship", &config, 0.1);
            let bad_risk = calculate_single_modern_risk(&extreme_bad, 0.0, Some(&ship), "cargo_ship", &config, 0.9);

            assert!(good_risk >= 0.0, "good risk {} >= 0", good_risk);
            assert!(good_risk <= 1.0, "good risk {} <= 1", good_risk);
            assert!(bad_risk >= 0.0, "bad risk {} >= 0", bad_risk);
            assert!(bad_risk <= 1.0, "bad risk {} <= 1", bad_risk);
        }

        #[test]
        fn test_tech_improvement_reduces_risk() {
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let forecast = create_test_forecast(20.0, 3.0, 0.5);
            let global_avg = 0.3;

            let low_tech = ModernComparisonConfig {
                modern_risk_multiplier: 0.8,
                tech_improvement_factor: 0.1,
                weather_forecast_accuracy: 0.8,
            };
            let high_tech = ModernComparisonConfig {
                modern_risk_multiplier: 0.8,
                tech_improvement_factor: 0.5,
                weather_forecast_accuracy: 0.8,
            };

            let low_tech_risk = calculate_single_modern_risk(&forecast, 10.0, Some(&ship), "cargo_ship", &low_tech, global_avg);
            let high_tech_risk = calculate_single_modern_risk(&forecast, 10.0, Some(&ship), "cargo_ship", &high_tech, global_avg);

            assert!(high_tech_risk < low_tech_risk, "high tech risk ({}) < low tech risk ({})", high_tech_risk, low_tech_risk);
        }
    }

    mod comparison_summary_tests {
        use super::*;

        #[test]
        fn test_ancient_risk_greater_than_modern() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let global_avg = 0.2;

            let mut ancient_scores: Vec<f64> = vec![0.6, 0.7, 0.8, 0.5];
            let forecasts = vec![
                create_test_forecast(10.0, 1.0, 0.2),
                create_test_forecast(15.0, 2.0, 0.3),
                create_test_forecast(20.0, 3.0, 0.4),
                create_test_forecast(12.0, 1.5, 0.25),
            ];

            let avg_ancient: f64 = ancient_scores.iter().sum::<f64>() / ancient_scores.len() as f64;
            let avg_modern: f64 = forecasts
                .iter()
                .map(|f| calculate_single_modern_risk(f, 10.0, Some(&ship), "cargo_ship", &config, global_avg))
                .sum::<f64>()
                / forecasts.len() as f64;

            assert!(avg_ancient > avg_modern, "ancient ({}) > modern ({})", avg_ancient, avg_modern);
        }

        #[test]
        fn test_risk_reduction_percentage_calculation() {
            let avg_ancient = 0.8;
            let avg_modern = 0.4;
            let reduction_pct = if avg_ancient > 0.0 {
                (avg_ancient - avg_modern) / avg_ancient * 100.0
            } else {
                0.0
            };
            assert!(approx_eq(reduction_pct, 50.0, 1e-10));
        }

        #[test]
        fn test_risk_reduction_percentage_zero_ancient() {
            let avg_ancient = 0.0;
            let avg_modern = 0.2;
            let reduction_pct = if avg_ancient > 0.0 {
                (avg_ancient - avg_modern) / avg_ancient * 100.0
            } else {
                0.0
            };
            assert_eq!(reduction_pct, 0.0);
        }

        #[test]
        fn test_correlation_in_range() {
            let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
            let y_pos = vec![2.0, 4.0, 6.0, 8.0, 10.0];
            let y_neg = vec![10.0, 8.0, 6.0, 4.0, 2.0];

            let corr_pos = pearson_correlation(&x, &y_pos);
            let corr_neg = pearson_correlation(&x, &y_neg);

            assert!(corr_pos >= -1.0 && corr_pos <= 1.0);
            assert!(corr_neg >= -1.0 && corr_neg <= 1.0);
            assert!(corr_pos > 0.0);
            assert!(corr_neg < 0.0);
        }

        #[test]
        fn test_high_risk_count() {
            let risks = vec![0.1, 0.4, 0.2, 0.6, 0.35, 0.8];
            let high_risk_count = risks.iter().filter(|&&r| r > 0.3).count() as i32;
            assert_eq!(high_risk_count, 4);
        }

        #[test]
        fn test_pearson_correlation_perfect_positive() {
            let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
            let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
            let corr = pearson_correlation(&x, &y);
            assert!(approx_eq(corr, 1.0, 1e-6));
        }

        #[test]
        fn test_pearson_correlation_perfect_negative() {
            let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
            let y = vec![10.0, 8.0, 6.0, 4.0, 2.0];
            let corr = pearson_correlation(&x, &y);
            assert!(approx_eq(corr, -1.0, 1e-6));
        }

        #[test]
        fn test_pearson_correlation_short_input() {
            let x = vec![1.0];
            let y = vec![2.0];
            let corr = pearson_correlation(&x, &y);
            assert_eq!(corr, 0.0);

            let x_empty: Vec<f64> = vec![];
            let y_empty: Vec<f64> = vec![];
            let corr_empty = pearson_correlation(&x_empty, &y_empty);
            assert_eq!(corr_empty, 0.0);
        }
    }

    mod distance_tests {
        use super::*;

        #[test]
        fn test_haversine_distance_same_point() {
            let dist = haversine_distance_km(30.0, 120.0, 30.0, 120.0);
            assert!(approx_eq(dist, 0.0, 1e-6));
        }

        #[test]
        fn test_haversine_distance_known_distance() {
            let dist = haversine_distance_km(0.0, 0.0, 0.0, 1.0);
            let expected = 6371.0 * 1.0_f64.to_radians();
            assert!(approx_eq(dist, expected, 1.0));
        }

        #[test]
        fn test_route_offset_distance_empty() {
            let route1: Vec<Vec<f64>> = vec![];
            let route2 = vec![vec![120.0, 30.0]];
            assert_eq!(route_offset_distance(&route1, &route2), 0.0);
            assert_eq!(route_offset_distance(&route2, &route1), 0.0);
        }

        #[test]
        fn test_route_offset_distance_identical() {
            let route = vec![
                vec![120.0, 30.0],
                vec![121.0, 31.0],
            ];
            let dist = route_offset_distance(&route, &route);
            assert!(approx_eq(dist, 0.0, 1e-6));
        }
    }

    mod edge_case_tests {
        use super::*;

        #[test]
        fn test_empty_input_returns_default() {
            let region_risks: HashMap<String, Vec<f64>> = HashMap::new();
            let most_dangerous = find_most_dangerous_region(&region_risks);
            assert_eq!(most_dangerous, "unknown");

            let empty_risks: Vec<StormRiskResult> = vec![];
            let port_map: HashMap<i32, String> = HashMap::new();
            let result = compute_region_risks(&empty_risks, &port_map);
            assert!(result.is_empty());
        }

        #[test]
        fn test_wind_speed_zero() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let forecast = create_test_forecast(0.0, 2.0, 0.1);
            let risk = calculate_single_modern_risk(&forecast, 10.0, Some(&ship), "cargo_ship", &config, 0.2);
            assert!(risk >= 0.0 && risk <= 1.0);
        }

        #[test]
        fn test_wind_speed_extreme() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let forecast = create_test_forecast(1000.0, 2.0, 0.1);
            let risk = calculate_single_modern_risk(&forecast, 10.0, Some(&ship), "cargo_ship", &config, 0.2);
            assert!(risk >= 0.0 && risk <= 1.0);
        }

        #[test]
        fn test_visibility_zero() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let forecast = create_test_forecast(15.0, 2.0, 0.1);
            let risk = calculate_single_modern_risk(&forecast, 0.0, Some(&ship), "cargo_ship", &config, 0.2);
            assert!(risk >= 0.0 && risk <= 1.0);
        }

        #[test]
        fn test_visibility_extreme_high() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);
            let forecast = create_test_forecast(15.0, 2.0, 0.1);
            let risk = calculate_single_modern_risk(&forecast, 1000.0, Some(&ship), "cargo_ship", &config, 0.2);
            assert!(risk >= 0.0 && risk <= 1.0);
        }

        #[test]
        fn test_storm_probability_clamped() {
            let config = create_test_config();
            let ship = create_test_ship("cargo_ship", 50000.0, 200.0, 18.0);

            let neg_storm = create_test_forecast(15.0, 2.0, -0.5);
            let over_storm = create_test_forecast(15.0, 2.0, 1.5);

            let neg_risk = calculate_single_modern_risk(&neg_storm, 10.0, Some(&ship), "cargo_ship", &config, 0.2);
            let over_risk = calculate_single_modern_risk(&over_storm, 10.0, Some(&ship), "cargo_ship", &config, 0.2);

            assert!(neg_risk >= 0.0 && neg_risk <= 1.0);
            assert!(over_risk >= 0.0 && over_risk <= 1.0);
            assert!(over_risk > neg_risk);
        }

        #[test]
        fn test_find_most_dangerous_region_empty_values() {
            let mut region_risks: HashMap<String, Vec<f64>> = HashMap::new();
            region_risks.insert("north".to_string(), vec![]);
            region_risks.insert("south".to_string(), vec![0.5, 0.6]);
            let most_dangerous = find_most_dangerous_region(&region_risks);
            assert_eq!(most_dangerous, "south");
        }

        #[test]
        fn test_compute_region_risks_unknown_region() {
            let risks = vec![StormRiskResult {
                departure_port_id: 999,
                arrival_port_id: 888,
                departure_port_name: "Test".to_string(),
                arrival_port_name: "Test2".to_string(),
                season: "summer".to_string(),
                risk_score: 0.5,
                sample_size: 10,
                model_type: "test".to_string(),
                confidence: 0.8,
            }];
            let port_map: HashMap<i32, String> = HashMap::new();
            let result = compute_region_risks(&risks, &port_map);
            assert!(result.contains_key("unknown"));
            assert_eq!(result.get("unknown").unwrap().len(), 1);
        }
    }
}
