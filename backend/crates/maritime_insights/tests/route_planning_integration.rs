use maritime_insights::route_planning::{
    plan_optimal_route, get_historical_route, compare_routes, RoutePlanningResult, RouteComparison,
};
use maritime_common::models::Port;
use maritime_common::config::RoutePlanningConfig;

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

fn default_test_config() -> RoutePlanningConfig {
    RoutePlanningConfig {
        grid_resolution_km: 100.0,
        max_iterations: 5000,
        current_weight: 0.3,
        wind_weight: 0.3,
        storm_risk_weight: 0.2,
        distance_weight: 0.2,
    }
}

fn make_port(id: i32, name: &str, lat: f64, lon: f64, region: &str) -> Port {
    Port {
        id,
        name: name.to_string(),
        name_zh: None,
        region: Some(region.to_string()),
        lat: Some(lat),
        lon: Some(lon),
    }
}

fn make_route_result(points: Vec<Vec<f64>>, distance: f64, days: f64, risk: f64) -> RoutePlanningResult {
    RoutePlanningResult {
        departure_port_id: 21,
        arrival_port_id: 22,
        departure_port_name: "Start".to_string(),
        arrival_port_name: "End".to_string(),
        season: "summer".to_string(),
        ship_type: "merchant".to_string(),
        method: "test".to_string(),
        route_points: points,
        distance_nautical_miles: distance,
        estimated_days: days,
        avg_speed_knots: 5.0,
        storm_risk: risk,
        historical_deviation_pct: 0.0,
        historical_correlation: 1.0,
    }
}

#[test]
fn test_compare_routes_same_route_max_similarity() {
    let points = vec![
        vec![24.87, 118.68],
        vec![23.5, 117.2],
        vec![22.0, 115.5],
    ];

    let route = make_route_result(points.clone(), 500.0, 10.0, 0.15);
    let comparison = compare_routes(&route, Some(&route));

    assert!(approx_eq(comparison.similarity_score, 1.0, 0.001));
    assert!(approx_eq(comparison.distance_diff_pct, 0.0, 0.001));
    assert!(approx_eq(comparison.time_diff_pct, 0.0, 0.001));
    assert_eq!(comparison.waypoints_matched, comparison.total_waypoints);
}

#[test]
fn test_compare_routes_historical_none_returns_defaults() {
    let points = vec![vec![24.87, 118.68], vec![23.5, 117.2]];
    let optimized = make_route_result(points, 200.0, 5.0, 0.1);

    let comparison = compare_routes(&optimized, None);

    assert_eq!(comparison.similarity_score, 0.0);
    assert_eq!(comparison.distance_diff_pct, 0.0);
    assert_eq!(comparison.waypoints_matched, 0);
    assert_eq!(comparison.total_waypoints, 2);
}

#[test]
fn test_compare_routes_distance_diff_calculation() {
    let points = vec![vec![0.0, 0.0], vec![1.0, 1.0]];

    let optimized = make_route_result(points.clone(), 110.0, 5.0, 0.2);
    let historical = make_route_result(points, 100.0, 4.0, 0.15);

    let comparison = compare_routes(&optimized, Some(&historical));

    assert!(approx_eq(comparison.distance_diff_pct, 10.0, 0.01));
    assert!(approx_eq(comparison.time_diff_pct, 25.0, 0.01));
}

#[test]
fn test_route_planning_config_bounds() {
    let config = default_test_config();

    assert!(config.grid_resolution_km > 0.0);
    assert!(config.max_iterations > 0);
    assert!(config.current_weight >= 0.0);
    assert!(config.wind_weight >= 0.0);
    assert!(config.storm_risk_weight >= 0.0);
    assert!(config.distance_weight >= 0.0);
}

#[test]
fn test_route_points_order_preserved() {
    let points = vec![
        vec![30.0, 120.0],
        vec![25.0, 118.0],
        vec![20.0, 115.0],
    ];

    let route = make_route_result(points.clone(), 800.0, 15.0, 0.25);

    assert_eq!(route.route_points.len(), 3);
    assert_eq!(route.route_points[0], points[0]);
    assert_eq!(route.route_points[2], points[2]);
}

#[test]
fn test_storm_risk_bounds() {
    let points = vec![vec![0.0, 0.0], vec![1.0, 1.0]];
    let route = make_route_result(points, 100.0, 3.0, 0.5);

    assert!(route.storm_risk >= 0.0 && route.storm_risk <= 1.0);
}

#[test]
fn test_port_data_structure() {
    let port = make_port(42, "TestPort", 30.0, 120.0, "East Asia");

    assert_eq!(port.id, 42);
    assert_eq!(port.name, "TestPort");
    assert_eq!(port.region.as_deref(), Some("East Asia"));
    assert!(port.lat.is_some());
    assert_eq!(port.lat.unwrap(), 30.0);
}

#[test]
fn test_route_comparison_fields() {
    let comparison = RouteComparison {
        distance_diff_pct: 5.0,
        time_diff_pct: 8.0,
        risk_diff_pct: -10.0,
        similarity_score: 0.75,
        waypoints_matched: 6,
        total_waypoints: 8,
    };

    assert_eq!(comparison.distance_diff_pct, 5.0);
    assert!(comparison.similarity_score >= 0.0 && comparison.similarity_score <= 1.0);
    assert!(comparison.waypoints_matched <= comparison.total_waypoints);
}
