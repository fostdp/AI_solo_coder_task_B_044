use maritime_insights::cargo_spread::{
    SpreadGraph, TechDiffusionSimulator, TechnologyPreset, get_technology_presets,
    find_origin_port_by_keyword, build_cargo_spread_network, compute_cultural_diversity_index,
    identify_cross_civilization_routes,
};
use maritime_common::models::{Port, VoyageRecord, CargoSpreadResponse};
use maritime_common::config::CargoSpreadConfig;
use std::collections::HashMap;

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

fn make_port(id: i32, name: &str, region: &str) -> Port {
    Port {
        id,
        name: name.to_string(),
        name_zh: None,
        region: Some(region.to_string()),
        lat: Some(30.0 + id as f64),
        lon: Some(120.0 + id as f64),
    }
}

fn make_voyage(id: i32, from: i32, to: i32, year: i32, cargo: &str) -> VoyageRecord {
    VoyageRecord {
        id,
        departure_port_id: from,
        arrival_port_id: to,
        voyage_year: year,
        season: "spring".to_string(),
        ship_type: "cog".to_string(),
        cargo_type: cargo.to_string(),
        encountered_storm: false,
        route_points: None,
        created_at: None,
    }
}

fn default_config() -> CargoSpreadConfig {
    CargoSpreadConfig {
        min_spread_threshold: 0.1,
        diffusion_decay_rate: 0.1,
        max_propagation_steps: 10,
    }
}

#[test]
fn test_spread_graph_full_lifecycle() {
    let ports = vec![
        make_port(1, "PortA", "Region1"),
        make_port(2, "PortB", "Region1"),
        make_port(3, "PortC", "Region2"),
    ];

    let voyages = vec![
        make_voyage(1, 1, 2, 1000, "spices"),
        make_voyage(2, 2, 3, 1001, "spices"),
        make_voyage(3, 1, 3, 998, "porcelain"),
    ];

    let graph = SpreadGraph::from_voyages(&voyages, &ports);

    assert_eq!(graph.nodes().len(), 3);
    assert_eq!(graph.edges().len(), 3);
    assert!(graph.get_port(1).is_some());
    assert!(graph.get_port(999).is_none());
    assert_eq!(graph.get_first_year(1), Some(998));
    assert!(graph.get_trade_volume(1) > 0.0);
    assert!(graph.max_trade_volume() > 0.0);
}

#[test]
fn test_tech_diffusion_simulator_full_run() {
    let ports = vec![
        make_port(1, "Origin", "Region1"),
        make_port(2, "Mid1", "Region1"),
        make_port(3, "Mid2", "Region2"),
        make_port(4, "Dest", "Region2"),
    ];

    let voyages = vec![
        make_voyage(1, 1, 2, 1000, "tech"),
        make_voyage(2, 2, 3, 1000, "tech"),
        make_voyage(3, 3, 4, 1000, "tech"),
    ];

    let graph = SpreadGraph::from_voyages(&voyages, &ports);
    let config = CargoSpreadConfig {
        min_spread_threshold: 0.0,
        diffusion_decay_rate: 0.0,
        max_propagation_steps: 10,
    };

    let mut sim = TechDiffusionSimulator::new(graph, config.clone());

    sim.seed_origin(1, 1000);
    sim.simulate(5000.0);

    assert!(sim.get_activation_year(1).is_some());
    assert!(sim.get_activation_year(4).is_some());
    assert!(sim.get_activation_year(4).unwrap() >= sim.get_activation_year(1).unwrap());
}

#[test]
fn test_technology_presets_valid() {
    let presets = get_technology_presets();

    assert!(!presets.is_empty());

    for preset in &presets {
        assert!(!preset.name.is_empty());
        assert!(!preset.name_zh.is_empty());
        assert!(!preset.origin_keywords.is_empty());
        assert_ne!(preset.estimated_start_year, 0);
    }

    let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"iron_smelting"));
    assert!(names.contains(&"porcelain"));
}

#[test]
fn test_find_origin_port_by_keyword_matching() {
    let ports = vec![
        make_port(1, "Quanzhou", "East Asia"),
        make_port(2, "Guangzhou", "East Asia"),
        make_port(3, "Alexandria", "Mediterranean"),
    ];

    let result = find_origin_port_by_keyword(&ports, &["quanzhou".to_string()]);
    assert_eq!(result, Some(1));

    let result = find_origin_port_by_keyword(&ports, &["east".to_string(), "asia".to_string()]);
    assert!(result.is_some());

    let result = find_origin_port_by_keyword(&ports, &["nonexistent".to_string()]);
    assert_eq!(result, None);
}

#[test]
fn test_cultural_diversity_index() {
    let ports = vec![
        make_port(1, "P1", "RegionA"),
        make_port(2, "P2", "RegionB"),
        make_port(3, "P3", "RegionC"),
        make_port(4, "P4", "RegionA"),
    ];

    let voyages = vec![
        make_voyage(1, 1, 2, 1000, "spices"),
        make_voyage(2, 2, 3, 1000, "spices"),
    ];

    let graph = SpreadGraph::from_voyages(&voyages, &ports);
    let diversity = compute_cultural_diversity_index(&graph);

    assert!(diversity >= 0.0 && diversity <= 1.0);
    assert!(diversity > 0.0);
}

#[test]
fn test_cross_civilization_routes_identification() {
    let ports = vec![
        make_port(1, "P1", "CivilizationA"),
        make_port(2, "P2", "CivilizationB"),
        make_port(3, "P3", "CivilizationA"),
    ];

    let voyages = vec![
        make_voyage(1, 1, 2, 1000, "spices"),
        make_voyage(2, 1, 3, 1000, "spices"),
    ];

    let graph = SpreadGraph::from_voyages(&voyages, &ports);
    let cross_routes = identify_cross_civilization_routes(&graph);

    assert!(!cross_routes.is_empty());
}

#[test]
fn test_spread_graph_trade_volume_accumulation() {
    let ports = vec![
        make_port(1, "A", "R1"),
        make_port(2, "B", "R1"),
    ];

    let voyages = vec![
        make_voyage(1, 1, 2, 1000, "spices"),
        make_voyage(2, 1, 2, 1001, "spices"),
        make_voyage(3, 2, 1, 1002, "spices"),
    ];

    let graph = SpreadGraph::from_voyages(&voyages, &ports);

    assert_eq!(graph.get_trade_volume(1), 3.0);
    assert_eq!(graph.get_trade_volume(2), 3.0);
    assert_eq!(graph.max_trade_volume(), 3.0);
}

#[test]
fn test_cargo_spread_config_validation() {
    let config = default_config();

    assert!(config.min_spread_threshold >= 0.0);
    assert!(config.diffusion_decay_rate >= 0.0);
    assert!(config.max_propagation_steps > 0);
}

#[test]
fn test_diffusion_node_tracking() {
    let ports = vec![make_port(1, "A", "R1")];
    let graph = SpreadGraph::from_voyages(&[], &ports);
    let config = default_config();
    let mut sim = TechDiffusionSimulator::new(graph, config);

    assert_eq!(sim.get_activation_year(1), None);

    sim.seed_origin(1, 500);

    assert_eq!(sim.get_activation_year(1), Some(500));
}

#[test]
fn test_build_cargo_spread_network_basic() {
    let ports = vec![
        make_port(1, "A", "R1"),
        make_port(2, "B", "R1"),
        make_port(3, "C", "R2"),
    ];

    let voyages = vec![
        make_voyage(1, 1, 2, 1000, "spices"),
        make_voyage(2, 2, 3, 1000, "spices"),
    ];

    let config = default_config();
    let response = build_cargo_spread_network(&voyages, &ports, "spices", 900, 1100, &config);

    assert_eq!(response.cargo_type, "spices");
    assert!(response.spread_network.nodes.len() >= 2);
    assert!(response.spread_network.edges.len() >= 2);
    assert!(!response.spread_network.origin_ports.is_empty());
}
