use maritime_insights::modern_comparison::{
    ship_risk_factor, risk_level, bayesian_smooth,
    calculate_single_modern_risk,
};
use maritime_common::models::{ModernShip, ModernWeatherForecast, ModernRiskResult};
use maritime_common::config::ModernComparisonConfig;

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

fn default_config() -> ModernComparisonConfig {
    ModernComparisonConfig {
        modern_risk_multiplier: 0.6,
        tech_improvement_factor: 0.7,
        weather_forecast_accuracy: 0.85,
    }
}

fn make_forecast(wind: f64, wave: f64, storm: f64) -> ModernWeatherForecast {
    ModernWeatherForecast {
        region: "TestRegion".to_string(),
        wind_direction_deg: 180.0,
        wind_speed_knots: wind,
        wave_height_m: wave,
        current_direction_deg: 90.0,
        current_speed_knots: 1.0,
        storm_probability: storm,
        lat: 30.0,
        lon: 120.0,
    }
}

fn make_ship(tonnage: f64, length: f64, speed: f64, ship_type: &str) -> ModernShip {
    ModernShip {
        id: 1,
        ship_name: "TestShip".to_string(),
        mmsi: "123456789".to_string(),
        ship_type: ship_type.to_string(),
        gross_tonnage: tonnage,
        length_m: length,
        beam_m: 20.0,
        max_speed_knots: speed,
        flag: "TestFlag".to_string(),
    }
}

#[test]
fn test_ship_risk_factor_ordering() {
    let fishing = ship_risk_factor("fishing_vessel");
    let sailing = ship_risk_factor("sailing_vessel");
    let tanker = ship_risk_factor("tanker");
    let bulk = ship_risk_factor("bulk_carrier");
    let cargo = ship_risk_factor("cargo_ship");
    let container = ship_risk_factor("container_ship");
    let passenger = ship_risk_factor("passenger_ship");
    let unknown = ship_risk_factor("unknown_type");

    assert!(fishing >= tanker);
    assert!(bulk >= container);
    assert!(passenger < cargo);
    assert_eq!(unknown, 0.75);

    assert!(fishing > 0.0 && fishing <= 1.0);
}

#[test]
fn test_risk_level_boundaries() {
    assert_eq!(risk_level(1.0), "very_high");
    assert_eq!(risk_level(0.7), "very_high");
    assert_eq!(risk_level(0.6999), "high");
    assert_eq!(risk_level(0.5), "high");
    assert_eq!(risk_level(0.4999), "medium");
    assert_eq!(risk_level(0.3), "medium");
    assert_eq!(risk_level(0.2999), "low");
    assert_eq!(risk_level(0.1), "low");
    assert_eq!(risk_level(0.0999), "very_low");
    assert_eq!(risk_level(0.0), "very_low");
}

#[test]
fn test_bayesian_smooth_behavior() {
    let raw = 0.8;
    let prior = 0.3;

    let sample_dom = bayesian_smooth(raw, prior, 100.0, 1.0);
    assert!(approx_eq(sample_dom, raw, 0.05));

    let prior_dom = bayesian_smooth(raw, prior, 1.0, 100.0);
    assert!(approx_eq(prior_dom, prior, 0.05));

    let equal = bayesian_smooth(raw, prior, 1.0, 1.0);
    assert!(approx_eq(equal, (raw + prior) / 2.0, 0.001));

    let same = bayesian_smooth(0.5, 0.5, 5.0, 5.0);
    assert!(approx_eq(same, 0.5, 0.0001));
}

#[test]
fn test_modern_risk_monotonic_wind() {
    let config = default_config();
    let ship = make_ship(10000.0, 150.0, 15.0, "cargo_ship");

    let low = calculate_single_modern_risk(
        &make_forecast(5.0, 1.0, 0.0), 10.0, Some(&ship), "cargo_ship", &config, 0.1
    );
    let high = calculate_single_modern_risk(
        &make_forecast(30.0, 1.0, 0.0), 10.0, Some(&ship), "cargo_ship", &config, 0.1
    );

    assert!(high > low);
}

#[test]
fn test_modern_risk_monotonic_wave() {
    let config = default_config();
    let ship = make_ship(10000.0, 150.0, 15.0, "cargo_ship");

    let low = calculate_single_modern_risk(
        &make_forecast(10.0, 0.5, 0.0), 10.0, Some(&ship), "cargo_ship", &config, 0.1
    );
    let high = calculate_single_modern_risk(
        &make_forecast(10.0, 5.0, 0.0), 10.0, Some(&ship), "cargo_ship", &config, 0.1
    );

    assert!(high > low);
}

#[test]
fn test_modern_risk_monotonic_visibility() {
    let config = default_config();
    let ship = make_ship(10000.0, 150.0, 15.0, "cargo_ship");
    let fc = make_forecast(10.0, 1.0, 0.0);

    let low_vis = calculate_single_modern_risk(&fc, 1.0, Some(&ship), "cargo_ship", &config, 0.1);
    let high_vis = calculate_single_modern_risk(&fc, 20.0, Some(&ship), "cargo_ship", &config, 0.1);

    assert!(low_vis > high_vis);
}

#[test]
fn test_modern_risk_increases_with_storm_prob() {
    let config = default_config();
    let ship = make_ship(10000.0, 150.0, 15.0, "cargo_ship");

    let low = calculate_single_modern_risk(
        &make_forecast(10.0, 1.0, 0.0), 10.0, Some(&ship), "cargo_ship", &config, 0.1
    );
    let high = calculate_single_modern_risk(
        &make_forecast(10.0, 1.0, 0.8), 10.0, Some(&ship), "cargo_ship", &config, 0.1
    );

    assert!(high > low);
}

#[test]
fn test_modern_risk_bounded_0_1() {
    let config = default_config();
    let ship = make_ship(50000.0, 300.0, 25.0, "container_ship");

    for wind in [0.0, 10.0, 50.0, 100.0] {
        for wave in [0.0, 2.0, 10.0] {
            for storm in [0.0, 0.5, 1.0] {
                let fc = make_forecast(wind, wave, storm);
                let risk = calculate_single_modern_risk(
                    &fc, 10.0, Some(&ship), "container_ship", &config, 0.1
                );
                assert!(risk >= 0.0 && risk <= 1.0, "Risk {} out of [0,1]", risk);
            }
        }
    }
}

#[test]
fn test_tech_improvement_reduces_risk() {
    let ship = make_ship(10000.0, 150.0, 15.0, "cargo_ship");
    let fc = make_forecast(20.0, 2.0, 0.3);

    let low_tech = ModernComparisonConfig {
        tech_improvement_factor: 0.3,
        weather_forecast_accuracy: 0.5,
        ..default_config()
    };
    let high_tech = ModernComparisonConfig {
        tech_improvement_factor: 0.9,
        weather_forecast_accuracy: 0.95,
        ..default_config()
    };

    let risk_low = calculate_single_modern_risk(&fc, 10.0, Some(&ship), "cargo_ship", &low_tech, 0.1);
    let risk_high = calculate_single_modern_risk(&fc, 10.0, Some(&ship), "cargo_ship", &high_tech, 0.1);

    assert!(risk_high < risk_low);
}

#[test]
fn test_larger_ship_lower_risk() {
    let config = default_config();
    let fc = make_forecast(20.0, 2.0, 0.3);

    let small = make_ship(1000.0, 50.0, 10.0, "cargo_ship");
    let big = make_ship(100000.0, 350.0, 25.0, "cargo_ship");

    let risk_small = calculate_single_modern_risk(&fc, 10.0, Some(&small), "cargo_ship", &config, 0.1);
    let risk_big = calculate_single_modern_risk(&fc, 10.0, Some(&big), "cargo_ship", &config, 0.1);

    assert!(risk_big < risk_small);
}

#[test]
fn test_no_ship_uses_base_factor() {
    let config = default_config();
    let fc = make_forecast(15.0, 1.5, 0.2);
    let ship = make_ship(20000.0, 180.0, 18.0, "tanker");

    let with_ship = calculate_single_modern_risk(&fc, 10.0, Some(&ship), "tanker", &config, 0.1);
    let without_ship = calculate_single_modern_risk(&fc, 10.0, None, "tanker", &config, 0.1);

    assert!(without_ship >= with_ship || approx_eq(without_ship, with_ship, 0.001));
}

#[test]
fn test_modern_risk_result_structure() {
    let result = ModernRiskResult {
        departure_port_id: 21,
        arrival_port_id: 22,
        risk_score: 0.42,
        risk_level: "medium".to_string(),
        model_type: "modern_combined".to_string(),
        ancient_comparison_score: 0.65,
        route_points: vec![],
        estimated_delay_hours: 0.0,
        alternative_route_suggestion: None,
    };

    assert_eq!(result.risk_level, "medium");
    assert!(result.risk_score >= 0.0 && result.risk_score <= 1.0);
    assert!(result.ancient_comparison_score >= 0.0);
}

#[test]
fn test_config_values_sensible() {
    let config = default_config();

    assert!(config.modern_risk_multiplier > 0.0 && config.modern_risk_multiplier <= 1.0);
    assert!(config.tech_improvement_factor >= 0.0 && config.tech_improvement_factor <= 1.0);
    assert!(config.weather_forecast_accuracy >= 0.0 && config.weather_forecast_accuracy <= 1.0);
}

#[test]
fn test_extreme_weather_handling() {
    let config = default_config();
    let ship = make_ship(10000.0, 150.0, 15.0, "cargo_ship");

    let zero_risk = calculate_single_modern_risk(
        &make_forecast(0.0, 0.0, 0.0), 100.0, Some(&ship), "cargo_ship", &config, 0.0
    );
    assert!(zero_risk >= 0.0);

    let high_risk = calculate_single_modern_risk(
        &make_forecast(1000.0, 100.0, 1.0), 0.0, Some(&ship), "cargo_ship", &config, 1.0
    );
    assert!(high_risk <= 1.0 && high_risk >= 0.0);
}
