use maritime_common::config::CargoSpreadConfig;
use maritime_common::models::*;
use rand::Rng;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct SpreadGraph {
    adjacency: HashMap<i32, HashMap<i32, f64>>,
    ports: HashMap<i32, Port>,
    first_year: HashMap<i32, i32>,
    trade_volume: HashMap<i32, f64>,
}

impl SpreadGraph {
    pub fn new() -> Self {
        SpreadGraph {
            adjacency: HashMap::new(),
            ports: HashMap::new(),
            first_year: HashMap::new(),
            trade_volume: HashMap::new(),
        }
    }

    pub fn from_voyages(voyages: &[VoyageRecord], ports: &[Port]) -> Self {
        let mut graph = SpreadGraph::new();
        graph.ports = ports.iter().map(|p| (p.id, p.clone())).collect();

        for v in voyages {
            let from = v.departure_port_id;
            let to = v.arrival_port_id;
            let year = v.voyage_year;

            let entry = graph.adjacency.entry(from).or_insert_with(HashMap::new);
            *entry.entry(to).or_insert(0.0) += 1.0;

            *graph.trade_volume.entry(from).or_insert(0.0) += 1.0;
            *graph.trade_volume.entry(to).or_insert(0.0) += 1.0;

            let current_from = graph.first_year.get(&from).copied().unwrap_or(i32::MAX);
            if year < current_from {
                graph.first_year.insert(from, year);
            }

            let current_to = graph.first_year.get(&to).copied().unwrap_or(i32::MAX);
            if year < current_to {
                graph.first_year.insert(to, year);
            }
        }

        graph
    }

    pub fn nodes(&self) -> Vec<i32> {
        self.ports.keys().copied().collect()
    }

    pub fn edges(&self) -> Vec<(i32, i32, f64)> {
        let mut result = Vec::new();
        for (&src, neighbors) in &self.adjacency {
            for (&dst, &weight) in neighbors {
                result.push((src, dst, weight));
            }
        }
        result
    }

    pub fn get_port(&self, port_id: i32) -> Option<&Port> {
        self.ports.get(&port_id)
    }

    pub fn get_first_year(&self, port_id: i32) -> Option<i32> {
        self.first_year.get(&port_id).copied()
    }

    pub fn get_trade_volume(&self, port_id: i32) -> f64 {
        self.trade_volume.get(&port_id).copied().unwrap_or(0.0)
    }

    pub fn max_trade_volume(&self) -> f64 {
        self.trade_volume.values().copied().fold(0.0, f64::max)
    }

    pub fn compute_betweenness_bfs(&self) -> HashMap<i32, f64> {
        let nodes: Vec<i32> = self.ports.keys().copied().collect();
        let mut betweenness: HashMap<i32, f64> = HashMap::new();
        for &n in &nodes {
            betweenness.insert(n, 0.0);
        }

        for &source in &nodes {
            let (dist, sigma, pred) = self.bfs_single_source(source);

            let mut delta: HashMap<i32, f64> = HashMap::new();
            for &n in &nodes {
                delta.insert(n, 0.0);
            }

            let mut sorted_nodes: Vec<(i32, i32)> = dist
                .iter()
                .filter(|(_, &d)| d < i32::MAX)
                .map(|(&p, &d)| (p, d))
                .collect();
            sorted_nodes.sort_by(|a, b| b.1.cmp(&a.1));

            for (w, _) in &sorted_nodes {
                if *w == source {
                    continue;
                }
                let sigma_w = sigma.get(w).copied().unwrap_or(0);
                if sigma_w == 0 {
                    continue;
                }
                for v in pred.get(w).unwrap_or(&Vec::new()) {
                    let sigma_v = sigma.get(v).copied().unwrap_or(1).max(1) as f64;
                    let delta_v = delta.get(v).copied().unwrap_or(0.0);
                    let delta_w = delta.get(w).copied().unwrap_or(0.0);
                    delta.insert(*v, delta_v + (sigma_v / sigma_w as f64) * (1.0 + delta_w));
                }
                if *w != source {
                    let bw = betweenness.get(w).copied().unwrap_or(0.0);
                    betweenness.insert(*w, bw + delta.get(w).copied().unwrap_or(0.0));
                }
            }
        }

        let n = nodes.len() as f64;
        if n > 2.0 {
            let scale = 2.0 / ((n - 1.0) * (n - 2.0));
            for v in betweenness.values_mut() {
                *v *= scale;
            }
        }

        betweenness
    }

    fn bfs_single_source(
        &self,
        source: i32,
    ) -> (HashMap<i32, i32>, HashMap<i32, i32>, HashMap<i32, Vec<i32>>) {
        let mut dist: HashMap<i32, i32> = HashMap::new();
        let mut sigma: HashMap<i32, i32> = HashMap::new();
        let mut pred: HashMap<i32, Vec<i32>> = HashMap::new();

        for &p in self.ports.keys() {
            dist.insert(p, i32::MAX);
            sigma.insert(p, 0);
            pred.insert(p, Vec::new());
        }

        dist.insert(source, 0);
        sigma.insert(source, 1);

        let mut queue = VecDeque::new();
        queue.push_back(source);

        while let Some(v) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(&v) {
                for (&w, _) in neighbors {
                    let d_v = dist.get(&v).copied().unwrap_or(i32::MAX);
                    let d_w = dist.get(&w).copied().unwrap_or(i32::MAX);

                    if d_v.saturating_add(1) < d_w {
                        dist.insert(w, d_v.saturating_add(1));
                        sigma.insert(w, sigma.get(&v).copied().unwrap_or(0));
                        pred.entry(w)
                            .and_modify(|p| p.clear())
                            .or_insert_with(Vec::new);
                        pred.get_mut(&w).unwrap().push(v);
                        queue.push_back(w);
                    } else if d_v.saturating_add(1) == d_w {
                        let s_v = sigma.get(&v).copied().unwrap_or(0);
                        let s_w = sigma.get(&w).copied().unwrap_or(0);
                        sigma.insert(w, s_w + s_v);
                        pred.get_mut(&w).unwrap().push(v);
                    }
                }
            }
        }

        (dist, sigma, pred)
    }

    pub fn compute_adoption_levels(&self) -> HashMap<i32, f64> {
        let max_vol = self.max_trade_volume();
        let mut adoption = HashMap::new();
        for (&port_id, &vol) in &self.trade_volume {
            let level = if max_vol > 0.0 { vol / max_vol } else { 0.0 };
            adoption.insert(port_id, level);
        }
        adoption
    }

    pub fn find_origin_ports(&self) -> Vec<i32> {
        let min_year = self.first_year.values().copied().min().unwrap_or(0);
        self.first_year
            .iter()
            .filter(|(_, &y)| y == min_year)
            .map(|(&id, _)| id)
            .collect()
    }

    pub fn find_hub_ports(&self, top_k: usize) -> Vec<i32> {
        let betweenness = self.compute_betweenness_bfs();
        let mut sorted: Vec<(i32, f64)> = betweenness.iter().map(|(&id, &b)| (id, b)).collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.iter().take(top_k).map(|(id, _)| *id).collect()
    }
}

impl Default for SpreadGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct DiffusionNode {
    pub port_id: i32,
    pub activated: bool,
    pub activation_year: Option<i32>,
    pub source_port: Option<i32>,
    pub diffusion_probability: f64,
}

impl DiffusionNode {
    pub fn new(port_id: i32) -> Self {
        DiffusionNode {
            port_id,
            activated: false,
            activation_year: None,
            source_port: None,
            diffusion_probability: 0.0,
        }
    }
}

pub struct TechDiffusionSimulator {
    graph: SpreadGraph,
    config: CargoSpreadConfig,
    nodes: HashMap<i32, DiffusionNode>,
    current_year: i32,
}

impl TechDiffusionSimulator {
    pub fn new(graph: SpreadGraph, config: CargoSpreadConfig) -> Self {
        let mut nodes = HashMap::new();
        for &port_id in graph.nodes().iter() {
            nodes.insert(port_id, DiffusionNode::new(port_id));
        }
        TechDiffusionSimulator {
            graph,
            config,
            nodes,
            current_year: 0,
        }
    }

    pub fn seed_origin(&mut self, origin_port_id: i32, start_year: i32) {
        if let Some(node) = self.nodes.get_mut(&origin_port_id) {
            node.activated = true;
            node.activation_year = Some(start_year);
            node.source_port = None;
            node.diffusion_probability = 1.0;
        }
        self.current_year = start_year;
    }

    fn haversine_distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0;
        let lat1_rad = lat1.to_radians();
        let lat2_rad = lat2.to_radians();
        let dlat = (lat2 - lat1).to_radians();
        let dlon = (lon2 - lon1).to_radians();

        let a = (dlat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }

    fn distance_between(&self, port1: i32, port2: i32) -> f64 {
        let p1 = self.graph.get_port(port1);
        let p2 = self.graph.get_port(port2);
        match (p1, p2) {
            (Some(a), Some(b)) => {
                let lat1 = a.lat.unwrap_or(0.0);
                let lon1 = a.lon.unwrap_or(0.0);
                let lat2 = b.lat.unwrap_or(0.0);
                let lon2 = b.lon.unwrap_or(0.0);
                Self::haversine_distance_km(lat1, lon1, lat2, lon2)
            }
            _ => 1000.0,
        }
    }

    pub fn simulate(&mut self, diffusion_speed_km_yr: f64) {
        let mut rng = rand::thread_rng();
        let mut activated_this_round: VecDeque<i32> = VecDeque::new();

        for (&port_id, node) in &self.nodes {
            if node.activated && node.activation_year.is_some() {
                activated_this_round.push_back(port_id);
            }
        }

        let mut steps = 0;
        while !activated_this_round.is_empty() && steps < self.config.max_propagation_steps {
            steps += 1;
            let current_size = activated_this_round.len();
            let mut next_round: VecDeque<i32> = VecDeque::new();

            for _ in 0..current_size {
                if let Some(source_id) = activated_this_round.pop_front() {
                    let source_node = self.nodes.get(&source_id).cloned().unwrap();
                    if !source_node.activated {
                        continue;
                    }

                    let source_year = source_node.activation_year.unwrap_or(self.current_year);

                    if let Some(neighbors) = self.graph.adjacency.get(&source_id) {
                        for (&target_id, &weight) in neighbors {
                            let target_node = self.nodes.get(&target_id);
                            if target_node.map(|n| n.activated).unwrap_or(false) {
                                continue;
                            }

                            let distance = self.distance_between(source_id, target_id);
                            let travel_years =
                                (distance / diffusion_speed_km_yr.max(1.0)).ceil() as i32;
                            let arrival_year = source_year + travel_years.max(1);

                            let base_prob = weight * self.config.diffusion_decay_rate;
                            let prob = base_prob.min(1.0).max(0.0);

                            if rng.gen::<f64>() < prob || weight >= self.config.min_spread_threshold
                            {
                                let target = self.nodes.get_mut(&target_id).unwrap();
                                target.activated = true;
                                target.activation_year = Some(arrival_year);
                                target.source_port = Some(source_id);
                                target.diffusion_probability = prob;
                                next_round.push_back(target_id);
                            }
                        }
                    }
                }
            }

            activated_this_round = next_round;
        }
    }

    pub fn get_diffusion_path(&self, target_port_id: i32) -> Vec<i32> {
        let mut path = Vec::new();
        let mut current = Some(target_port_id);

        while let Some(port_id) = current {
            path.push(port_id);
            let node = self.nodes.get(&port_id);
            current = node.and_then(|n| n.source_port);
        }

        path.reverse();
        path
    }

    pub fn get_activation_year(&self, port_id: i32) -> Option<i32> {
        self.nodes.get(&port_id).and_then(|n| n.activation_year)
    }

    pub fn activated_ports(&self) -> Vec<i32> {
        self.nodes
            .iter()
            .filter(|(_, n)| n.activated)
            .map(|(id, _)| *id)
            .collect()
    }
}

pub struct TechnologyPreset {
    pub name: String,
    pub name_zh: String,
    pub category: String,
    pub origin_keywords: Vec<String>,
    pub default_origin_port_id: i32,
    pub estimated_start_year: i32,
    pub diffusion_speed_km_yr: f64,
}

pub fn get_technology_presets() -> Vec<TechnologyPreset> {
    vec![
        TechnologyPreset {
            name: "iron_smelting".to_string(),
            name_zh: "冶铁技术".to_string(),
            category: "metallurgy".to_string(),
            origin_keywords: vec![
                "levant".to_string(),
                "phoenicia".to_string(),
                "syria".to_string(),
            ],
            default_origin_port_id: 1,
            estimated_start_year: -1500,
            diffusion_speed_km_yr: 50.0,
        },
        TechnologyPreset {
            name: "porcelain".to_string(),
            name_zh: "瓷器制造".to_string(),
            category: "ceramics".to_string(),
            origin_keywords: vec![
                "quanzhou".to_string(),
                "guangzhou".to_string(),
                "china".to_string(),
            ],
            default_origin_port_id: 2,
            estimated_start_year: 600,
            diffusion_speed_km_yr: 30.0,
        },
        TechnologyPreset {
            name: "shipbuilding".to_string(),
            name_zh: "造船技术".to_string(),
            category: "maritime".to_string(),
            origin_keywords: vec![
                "mediterranean".to_string(),
                "arab".to_string(),
                "phoenicia".to_string(),
            ],
            default_origin_port_id: 3,
            estimated_start_year: -1000,
            diffusion_speed_km_yr: 80.0,
        },
        TechnologyPreset {
            name: "navigation".to_string(),
            name_zh: "航海术".to_string(),
            category: "maritime".to_string(),
            origin_keywords: vec![
                "arab".to_string(),
                "indian ocean".to_string(),
                "persian gulf".to_string(),
            ],
            default_origin_port_id: 4,
            estimated_start_year: 800,
            diffusion_speed_km_yr: 100.0,
        },
        TechnologyPreset {
            name: "papermaking".to_string(),
            name_zh: "造纸术".to_string(),
            category: "technology".to_string(),
            origin_keywords: vec![
                "china".to_string(),
                "changan".to_string(),
                "luoyang".to_string(),
            ],
            default_origin_port_id: 5,
            estimated_start_year: 200,
            diffusion_speed_km_yr: 40.0,
        },
        TechnologyPreset {
            name: "coinage".to_string(),
            name_zh: "铸币技术".to_string(),
            category: "economy".to_string(),
            origin_keywords: vec![
                "lydia".to_string(),
                "mediterranean".to_string(),
                "greece".to_string(),
            ],
            default_origin_port_id: 6,
            estimated_start_year: -700,
            diffusion_speed_km_yr: 60.0,
        },
    ]
}

pub fn find_origin_port_by_keyword(ports: &[Port], keywords: &[String]) -> Option<i32> {
    for keyword in keywords {
        let kw = keyword.to_lowercase();
        for port in ports {
            if port.name.to_lowercase().contains(&kw) {
                return Some(port.id);
            }
            if let Some(ref name_zh) = port.name_zh {
                if name_zh.contains(&kw) {
                    return Some(port.id);
                }
            }
            if let Some(ref region) = port.region {
                if region.to_lowercase().contains(&kw) {
                    return Some(port.id);
                }
            }
        }
    }
    None
}

pub fn build_cargo_spread_network(
    voyages: &[VoyageRecord],
    ports: &[Port],
    cargo_type: &str,
    hub_top_k: usize,
) -> SpreadNetwork {
    let classified = classify_cargo_type(cargo_type);
    let (diffusion_rate, cultural_weight) = match &classified {
        Some(s) => (s.category.diffusion_rate(), s.category.cultural_weight()),
        None => (1.0, 1.0),
    };

    let filtered_voyages: Vec<VoyageRecord> = voyages
        .iter()
        .filter(|v| {
            if v.cargo_type == cargo_type {
                return true;
            }
            if let Some(ref s) = classified {
                let v_lower = v.cargo_type.to_lowercase();
                v_lower.contains(s.code)
                    || v_lower.contains(s.name)
                    || v_lower.contains(s.name_zh)
                    || s.code == v.cargo_type.to_lowercase()
            } else {
                false
            }
        })
        .cloned()
        .collect();

    let mut graph = SpreadGraph::new();
    graph.ports = ports.iter().map(|p| (p.id, p.clone())).collect();

    for v in &filtered_voyages {
        let from = v.departure_port_id;
        let to = v.arrival_port_id;
        let year = v.voyage_year;

        let edge_weight = diffusion_rate * cultural_weight;
        let entry = graph.adjacency.entry(from).or_insert_with(HashMap::new);
        *entry.entry(to).or_insert(0.0) += edge_weight;

        *graph.trade_volume.entry(from).or_insert(0.0) += edge_weight;
        *graph.trade_volume.entry(to).or_insert(0.0) += edge_weight;

        let current_from = graph.first_year.get(&from).copied().unwrap_or(i32::MAX);
        if year < current_from {
            graph.first_year.insert(from, year);
        }

        let current_to = graph.first_year.get(&to).copied().unwrap_or(i32::MAX);
        if year < current_to {
            graph.first_year.insert(to, year);
        }
    }

    let betweenness = graph.compute_betweenness_bfs();
    let adoption = graph.compute_adoption_levels();
    let origin_ports = graph.find_origin_ports();
    let hub_ports = graph.find_hub_ports(hub_top_k);

    let mut nodes = Vec::new();
    for port_id in graph.nodes() {
        let port = graph.get_port(port_id);
        if let Some(p) = port {
            nodes.push(SpreadNode {
                port_id,
                port_name: p.name.clone(),
                first_received_year: graph.get_first_year(port_id).unwrap_or(0),
                adoption_level: adoption.get(&port_id).copied().unwrap_or(0.0),
                betweenness: betweenness.get(&port_id).copied().unwrap_or(0.0),
            });
        }
    }

    let mut edges = Vec::new();
    for (from, to, weight) in graph.edges() {
        let mut first_year = i32::MAX;
        for v in &filtered_voyages {
            if v.departure_port_id == from && v.arrival_port_id == to {
                if v.voyage_year < first_year {
                    first_year = v.voyage_year;
                }
            }
        }
        edges.push(SpreadEdge {
            from_port_id: from,
            to_port_id: to,
            flow_volume: weight,
            first_spread_year: if first_year == i32::MAX {
                0
            } else {
                first_year
            },
        });
    }

    SpreadNetwork {
        nodes,
        edges,
        origin_ports,
        hub_ports,
    }
}

pub fn simulate_tech_diffusion(
    tech: &TechnologyPreset,
    graph: &SpreadGraph,
    config: &CargoSpreadConfig,
    ports: &[Port],
    path_id: i32,
) -> TechDiffusionPath {
    let origin_port_id = find_origin_port_by_keyword(ports, &tech.origin_keywords)
        .unwrap_or(tech.default_origin_port_id);

    let mut simulator = TechDiffusionSimulator::new(graph.clone(), config.clone());
    simulator.seed_origin(origin_port_id, tech.estimated_start_year);
    simulator.simulate(tech.diffusion_speed_km_yr);

    let activated = simulator.activated_ports();
    let mut end_year = tech.estimated_start_year;
    let mut farthest_port = origin_port_id;

    for &port_id in &activated {
        if let Some(year) = simulator.get_activation_year(port_id) {
            if year > end_year {
                end_year = year;
                farthest_port = port_id;
            }
        }
    }

    let spread_route = simulator.get_diffusion_path(farthest_port);

    let origin_port = ports
        .iter()
        .find(|p| p.id == origin_port_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();

    TechDiffusionPath {
        id: path_id,
        tech_name: tech.name.clone(),
        tech_name_zh: tech.name_zh.clone(),
        tech_category: tech.category.clone(),
        origin_port_id,
        origin_port_name: origin_port,
        spread_route,
        estimated_start_year: tech.estimated_start_year,
        estimated_end_year: end_year,
        diffusion_speed_km_yr: tech.diffusion_speed_km_yr,
        description: None,
    }
}

pub fn compute_cultural_diversity_index(
    voyages: &[VoyageRecord],
    ports: &[Port],
) -> HashMap<i32, f64> {
    let port_regions: HashMap<i32, String> = ports
        .iter()
        .filter_map(|p| p.region.as_ref().map(|r| (p.id, r.clone())))
        .collect();

    let mut port_cargo_regions: HashMap<i32, HashSet<(String, String)>> = HashMap::new();

    for v in voyages {
        let dep_region = port_regions
            .get(&v.departure_port_id)
            .cloned()
            .unwrap_or_default();
        let arr_region = port_regions
            .get(&v.arrival_port_id)
            .cloned()
            .unwrap_or_default();

        if !dep_region.is_empty() {
            port_cargo_regions
                .entry(v.arrival_port_id)
                .or_insert_with(HashSet::new)
                .insert((v.cargo_type.clone(), dep_region.clone()));
        }

        if !arr_region.is_empty() {
            port_cargo_regions
                .entry(v.departure_port_id)
                .or_insert_with(HashSet::new)
                .insert((v.cargo_type.clone(), arr_region.clone()));
        }
    }

    let mut diversity = HashMap::new();
    for (&port_id, set) in &port_cargo_regions {
        diversity.insert(port_id, set.len() as f64);
    }
    diversity
}

pub fn identify_cross_civilization_routes(
    voyages: &[VoyageRecord],
    ports: &[Port],
) -> Vec<(i32, i32, String, String, i32)> {
    let port_regions: HashMap<i32, String> = ports
        .iter()
        .filter_map(|p| p.region.as_ref().map(|r| (p.id, r.clone())))
        .collect();

    let mut route_counts: HashMap<(i32, i32), (String, String, i32)> = HashMap::new();

    for v in voyages {
        let dep_region = port_regions
            .get(&v.departure_port_id)
            .cloned()
            .unwrap_or_default();
        let arr_region = port_regions
            .get(&v.arrival_port_id)
            .cloned()
            .unwrap_or_default();

        if dep_region.is_empty() || arr_region.is_empty() {
            continue;
        }

        if dep_region != arr_region {
            let key = if v.departure_port_id < v.arrival_port_id {
                (v.departure_port_id, v.arrival_port_id)
            } else {
                (v.arrival_port_id, v.departure_port_id)
            };

            let (r1, r2, _) = route_counts.get(&key).cloned().unwrap_or_else(|| {
                if v.departure_port_id < v.arrival_port_id {
                    (dep_region, arr_region, 0)
                } else {
                    (arr_region, dep_region, 0)
                }
            });

            route_counts.insert(key, (r1, r2, v.voyage_year));
        }
    }

    let mut result = Vec::new();
    for ((from, to), (r1, r2, year)) in route_counts {
        result.push((from, to, r1, r2, year));
    }
    result.sort_by(|a, b| b.4.cmp(&a.4));
    result
}

pub fn get_cargo_spread_records(
    voyages: &[VoyageRecord],
    cargo_type: &str,
) -> Vec<CargoSpreadRecord> {
    let filtered: Vec<&VoyageRecord> = voyages
        .iter()
        .filter(|v| v.cargo_type == cargo_type)
        .collect();

    let mut records = Vec::new();
    for v in filtered {
        records.push(CargoSpreadRecord {
            cargo_type: v.cargo_type.clone(),
            from_port_id: v.departure_port_id,
            to_port_id: v.arrival_port_id,
            voyage_year: v.voyage_year,
            spread_direction: "forward".to_string(),
            quantity_estimate: 1.0,
            cultural_significance: None,
        });
    }
    records.sort_by(|a, b| a.voyage_year.cmp(&b.voyage_year));
    records
}

pub async fn analyze_cargo_spread(
    pool: &sqlx::PgPool,
    config: &CargoSpreadConfig,
    cargo_type: &str,
    year_start: i32,
    year_end: i32,
) -> CargoSpreadResponse {
    let voyages = sqlx::query_as!(
        VoyageRecord,
        "SELECT id, departure_port_id, arrival_port_id, voyage_year, season, \
         ship_type, cargo_type, encountered_storm, route_points, created_at \
         FROM voyage_records WHERE voyage_year >= $1 AND voyage_year <= $2",
        year_start,
        year_end
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let ports = sqlx::query_as!(
        Port,
        "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon FROM ports"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let spread_records = get_cargo_spread_records(&voyages, cargo_type);
    let spread_network = build_cargo_spread_network(&voyages, &ports, cargo_type, 10);

    let all_graph = SpreadGraph::from_voyages(&voyages, &ports);
    let tech_presets = get_technology_presets();
    let mut tech_diffusions = Vec::new();

    for (i, tech) in tech_presets.iter().enumerate() {
        let path = simulate_tech_diffusion(tech, &all_graph, config, &ports, i as i32 + 1);
        tech_diffusions.push(path);
    }

    CargoSpreadResponse {
        cargo_type: cargo_type.to_string(),
        spread_records,
        tech_diffusions,
        spread_network,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CargoCategory {
    LuxuryGoods,
    StapleFoods,
    RawMaterials,
    ManufacturedGoods,
    CulturalRelics,
    StrategicMaterials,
}

impl CargoCategory {
    pub fn name(&self) -> &'static str {
        match self {
            CargoCategory::LuxuryGoods => "luxury_goods",
            CargoCategory::StapleFoods => "staple_foods",
            CargoCategory::RawMaterials => "raw_materials",
            CargoCategory::ManufacturedGoods => "manufactured_goods",
            CargoCategory::CulturalRelics => "cultural_relics",
            CargoCategory::StrategicMaterials => "strategic_materials",
        }
    }

    pub fn name_zh(&self) -> &'static str {
        match self {
            CargoCategory::LuxuryGoods => "奢侈品",
            CargoCategory::StapleFoods => "主食",
            CargoCategory::RawMaterials => "原材料",
            CargoCategory::ManufacturedGoods => "制成品",
            CargoCategory::CulturalRelics => "文物/宗教",
            CargoCategory::StrategicMaterials => "战略物资",
        }
    }

    pub fn diffusion_rate(&self) -> f64 {
        match self {
            CargoCategory::LuxuryGoods => 0.85,
            CargoCategory::StapleFoods => 0.95,
            CargoCategory::RawMaterials => 0.70,
            CargoCategory::ManufacturedGoods => 0.75,
            CargoCategory::CulturalRelics => 0.40,
            CargoCategory::StrategicMaterials => 0.55,
        }
    }

    pub fn cultural_weight(&self) -> f64 {
        match self {
            CargoCategory::LuxuryGoods => 1.5,
            CargoCategory::StapleFoods => 0.6,
            CargoCategory::RawMaterials => 0.8,
            CargoCategory::ManufacturedGoods => 1.2,
            CargoCategory::CulturalRelics => 2.5,
            CargoCategory::StrategicMaterials => 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CargoSubtype {
    pub code: &'static str,
    pub name: &'static str,
    pub name_zh: &'static str,
    pub category: CargoCategory,
    pub origin_regions: Vec<&'static str>,
    pub tech_requirement: f64,
    pub rarity_score: f64,
}

#[derive(Debug, Clone)]
pub struct HierarchicalSpreadGraph {
    base_graph: SpreadGraph,
    by_category: HashMap<CargoCategory, SpreadGraph>,
    by_subtype: HashMap<String, SpreadGraph>,
}

impl HierarchicalSpreadGraph {
    pub fn new() -> Self {
        HierarchicalSpreadGraph {
            base_graph: SpreadGraph::new(),
            by_category: HashMap::new(),
            by_subtype: HashMap::new(),
        }
    }

    pub fn by_category(&self, category: &CargoCategory) -> Option<&SpreadGraph> {
        self.by_category.get(category)
    }

    pub fn by_subtype(&self, code: &str) -> Option<&SpreadGraph> {
        self.by_subtype.get(code)
    }

    pub fn base_graph(&self) -> &SpreadGraph {
        &self.base_graph
    }

    pub fn build_multi_layer(voyages: &[VoyageRecord], ports: &[Port]) -> Self {
        let mut result = HierarchicalSpreadGraph::new();
        result.base_graph = SpreadGraph::from_voyages(voyages, ports);

        let mut category_voyages: HashMap<CargoCategory, Vec<VoyageRecord>> = HashMap::new();
        let mut subtype_voyages: HashMap<String, Vec<VoyageRecord>> = HashMap::new();

        for v in voyages {
            if let Some(subtype) = classify_cargo_type(&v.cargo_type) {
                category_voyages
                    .entry(subtype.category.clone())
                    .or_insert_with(Vec::new)
                    .push(v.clone());
                subtype_voyages
                    .entry(subtype.code.to_string())
                    .or_insert_with(Vec::new)
                    .push(v.clone());
            }
        }

        for (cat, vs) in category_voyages {
            result
                .by_category
                .insert(cat, SpreadGraph::from_voyages(&vs, ports));
        }

        for (code, vs) in subtype_voyages {
            result
                .by_subtype
                .insert(code, SpreadGraph::from_voyages(&vs, ports));
        }

        result
    }
}

impl Default for HierarchicalSpreadGraph {
    fn default() -> Self {
        Self::new()
    }
}

pub const CARGO_HIERARCHY: &[CargoSubtype] = &[
    CargoSubtype {
        code: "porcelain",
        name: "porcelain",
        name_zh: "瓷器",
        category: CargoCategory::ManufacturedGoods,
        origin_regions: vec!["China", "East Asia"],
        tech_requirement: 0.8,
        rarity_score: 0.7,
    },
    CargoSubtype {
        code: "silk",
        name: "silk",
        name_zh: "丝绸",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["China", "East Asia"],
        tech_requirement: 0.7,
        rarity_score: 0.8,
    },
    CargoSubtype {
        code: "spices",
        name: "spices",
        name_zh: "香料",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["Southeast Asia", "India", "Moluccas"],
        tech_requirement: 0.3,
        rarity_score: 0.75,
    },
    CargoSubtype {
        code: "spice",
        name: "spice",
        name_zh: "香料",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["Southeast Asia", "India"],
        tech_requirement: 0.3,
        rarity_score: 0.75,
    },
    CargoSubtype {
        code: "iron_ore",
        name: "iron_ore",
        name_zh: "铁矿石",
        category: CargoCategory::RawMaterials,
        origin_regions: vec!["Mediterranean", "Middle East"],
        tech_requirement: 0.2,
        rarity_score: 0.3,
    },
    CargoSubtype {
        code: "grain",
        name: "grain",
        name_zh: "粮食",
        category: CargoCategory::StapleFoods,
        origin_regions: vec!["Egypt", "Black Sea", "Sicily"],
        tech_requirement: 0.1,
        rarity_score: 0.1,
    },
    CargoSubtype {
        code: "tin",
        name: "tin",
        name_zh: "锡",
        category: CargoCategory::StrategicMaterials,
        origin_regions: vec!["Britain", "Cornwall", "Malay"],
        tech_requirement: 0.4,
        rarity_score: 0.6,
    },
    CargoSubtype {
        code: "tea",
        name: "tea",
        name_zh: "茶叶",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["China", "India", "Assam"],
        tech_requirement: 0.4,
        rarity_score: 0.65,
    },
    CargoSubtype {
        code: "cotton",
        name: "cotton",
        name_zh: "棉花",
        category: CargoCategory::RawMaterials,
        origin_regions: vec!["India", "Egypt", "Persia"],
        tech_requirement: 0.2,
        rarity_score: 0.35,
    },
    CargoSubtype {
        code: "timber",
        name: "timber",
        name_zh: "木材",
        category: CargoCategory::RawMaterials,
        origin_regions: vec!["Black Sea", "Baltic", "Levant"],
        tech_requirement: 0.1,
        rarity_score: 0.2,
    },
    CargoSubtype {
        code: "wine",
        name: "wine",
        name_zh: "葡萄酒",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["Greece", "Italy", "Gaul"],
        tech_requirement: 0.35,
        rarity_score: 0.45,
    },
    CargoSubtype {
        code: "olive_oil",
        name: "olive_oil",
        name_zh: "橄榄油",
        category: CargoCategory::StapleFoods,
        origin_regions: vec!["Mediterranean", "Greece", "Iberia"],
        tech_requirement: 0.25,
        rarity_score: 0.25,
    },
    CargoSubtype {
        code: "copper",
        name: "copper",
        name_zh: "铜",
        category: CargoCategory::StrategicMaterials,
        origin_regions: vec!["Cyprus", "Anatolia", "Arabia"],
        tech_requirement: 0.45,
        rarity_score: 0.55,
    },
    CargoSubtype {
        code: "silver",
        name: "silver",
        name_zh: "白银",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["Spain", "Anatolia", "Greece"],
        tech_requirement: 0.6,
        rarity_score: 0.85,
    },
    CargoSubtype {
        code: "gold",
        name: "gold",
        name_zh: "黄金",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["Nubia", "Arabia", "India"],
        tech_requirement: 0.7,
        rarity_score: 0.95,
    },
    CargoSubtype {
        code: "ivory",
        name: "ivory",
        name_zh: "象牙",
        category: CargoCategory::LuxuryGoods,
        origin_regions: vec!["Africa", "India"],
        tech_requirement: 0.3,
        rarity_score: 0.9,
    },
    CargoSubtype {
        code: "relics",
        name: "relics",
        name_zh: "文物圣物",
        category: CargoCategory::CulturalRelics,
        origin_regions: vec!["Rome", "Constantinople", "Jerusalem"],
        tech_requirement: 0.5,
        rarity_score: 1.0,
    },
    CargoSubtype {
        code: "paper",
        name: "paper",
        name_zh: "纸张",
        category: CargoCategory::ManufacturedGoods,
        origin_regions: vec!["China", "Samarkand", "Baghdad"],
        tech_requirement: 0.65,
        rarity_score: 0.5,
    },
    CargoSubtype {
        code: "glass",
        name: "glass",
        name_zh: "玻璃",
        category: CargoCategory::ManufacturedGoods,
        origin_regions: vec!["Phoenicia", "Egypt", "Syria"],
        tech_requirement: 0.6,
        rarity_score: 0.6,
    },
    CargoSubtype {
        code: "salt",
        name: "salt",
        name_zh: "盐",
        category: CargoCategory::StapleFoods,
        origin_regions: vec!["North Africa", "Mediterranean", "India"],
        tech_requirement: 0.15,
        rarity_score: 0.4,
    },
];

pub fn classify_cargo_type(cargo_type: &str) -> Option<&'static CargoSubtype> {
    let lower = cargo_type.to_lowercase();
    for subtype in CARGO_HIERARCHY {
        if lower == subtype.code.to_lowercase()
            || lower == subtype.name.to_lowercase()
            || lower.contains(subtype.code)
            || lower.contains(subtype.name)
            || lower.contains(subtype.name_zh)
        {
            return Some(subtype);
        }
    }
    None
}

pub fn compute_layered_spread_paths(
    graph: &HierarchicalSpreadGraph,
    cargo_type: &str,
) -> Vec<(i32, i32, f64, String)> {
    let mut results = Vec::new();
    let subtype = classify_cargo_type(cargo_type);
    let category = subtype.as_ref().map(|s| s.category.clone());

    let working_graph = if let Some(ref st) = subtype {
        graph.by_subtype(st.code).unwrap_or(graph.base_graph())
    } else {
        graph.base_graph()
    };

    let (dist_filter, weight_multiplier) = match &category {
        Some(CargoCategory::LuxuryGoods) => (None, 1.5),
        Some(CargoCategory::StapleFoods) => (Some(1500.0), 0.6),
        Some(CargoCategory::RawMaterials) => (None, 0.8),
        Some(CargoCategory::ManufacturedGoods) => (None, 1.2),
        Some(CargoCategory::CulturalRelics) => (None, 2.0),
        Some(CargoCategory::StrategicMaterials) => (Some(3000.0), 1.0),
        None => (None, 1.0),
    };

    let origins = working_graph.find_origin_ports();
    for origin in &origins {
        for (from, to, weight) in working_graph.edges() {
            let adjusted_weight = weight * weight_multiplier;
            if let Some(max_km) = dist_filter {
                if let (Some(p1), Some(p2)) = (working_graph.get_port(from), working_graph.get_port(to))
                {
                    let lat1 = p1.lat.unwrap_or(0.0);
                    let lon1 = p1.lon.unwrap_or(0.0);
                    let lat2 = p2.lat.unwrap_or(0.0);
                    let lon2 = p2.lon.unwrap_or(0.0);
                    let dist = TechDiffusionSimulator::haversine_distance_km(lat1, lon1, lat2, lon2);
                    if dist > max_km {
                        continue;
                    }
                }
            }
            let layer = category.as_ref().map(|c| c.name().to_string()).unwrap_or_default();
            results.push((from, to, adjusted_weight, layer));
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use maritime_common::config::CargoSpreadConfig;
    use maritime_common::models::*;
    use std::collections::HashSet;

    fn make_test_port(id: i32, name: &str, region: Option<&str>, lat: Option<f64>, lon: Option<f64>) -> Port {
        Port {
            id,
            name: name.to_string(),
            name_zh: None,
            region: region.map(|r| r.to_string()),
            lat,
            lon,
        }
    }

    fn make_test_voyage(id: i32, from: i32, to: i32, year: i32, cargo: &str) -> VoyageRecord {
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

    fn default_test_config() -> CargoSpreadConfig {
        CargoSpreadConfig {
            min_spread_threshold: 0.0,
            diffusion_decay_rate: 1.0,
            max_propagation_steps: 100,
        }
    }

    mod spread_graph_tests {
        use super::*;

        #[test]
        fn test_new_empty_graph() {
            let graph = SpreadGraph::new();
            assert_eq!(graph.nodes().len(), 0);
            assert_eq!(graph.edges().len(), 0);
            assert_eq!(graph.max_trade_volume(), 0.0);
        }

        #[test]
        fn test_from_voyages_basic() {
            let ports = vec![
                make_test_port(1, "PortA", Some("Region1"), Some(0.0), Some(0.0)),
                make_test_port(2, "PortB", Some("Region1"), Some(1.0), Some(1.0)),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "spice"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);

            assert_eq!(graph.nodes().len(), 2);
            assert_eq!(graph.edges().len(), 1);

            let edges = graph.edges();
            assert_eq!(edges[0].0, 1);
            assert_eq!(edges[0].1, 2);
            assert_eq!(edges[0].2, 1.0);
        }

        #[test]
        fn test_nodes_count() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
                make_test_port(3, "C", None, None, None),
            ];
            let voyages: Vec<VoyageRecord> = Vec::new();
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            assert_eq!(graph.nodes().len(), 3);
            let node_ids: HashSet<i32> = graph.nodes().into_iter().collect();
            assert!(node_ids.contains(&1));
            assert!(node_ids.contains(&2));
            assert!(node_ids.contains(&3));
        }

        #[test]
        fn test_edges_count() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
                make_test_port(3, "C", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 2, 3, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            assert_eq!(graph.edges().len(), 2);
        }

        #[test]
        fn test_get_port() {
            let ports = vec![
                make_test_port(1, "TestPort", Some("TestRegion"), Some(10.0), Some(20.0)),
            ];
            let voyages: Vec<VoyageRecord> = Vec::new();
            let graph = SpreadGraph::from_voyages(&voyages, &ports);

            let port = graph.get_port(1);
            assert!(port.is_some());
            assert_eq!(port.unwrap().name, "TestPort");

            assert!(graph.get_port(999).is_none());
        }

        #[test]
        fn test_get_first_year() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 500, "cargo"),
                make_test_voyage(2, 1, 2, 300, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);

            assert_eq!(graph.get_first_year(1), Some(300));
            assert_eq!(graph.get_first_year(2), Some(300));
            assert!(graph.get_first_year(999).is_none());
        }

        #[test]
        fn test_get_trade_volume() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
                make_test_port(3, "C", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 1, 3, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);

            assert_eq!(graph.get_trade_volume(1), 2.0);
            assert_eq!(graph.get_trade_volume(2), 1.0);
            assert_eq!(graph.get_trade_volume(3), 1.0);
            assert_eq!(graph.get_trade_volume(999), 0.0);
        }

        #[test]
        fn test_max_trade_volume_empty() {
            let graph = SpreadGraph::new();
            assert_eq!(graph.max_trade_volume(), 0.0);
        }

        #[test]
        fn test_max_trade_volume() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 1, 2, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            assert_eq!(graph.max_trade_volume(), 2.0);
        }
    }

    mod graph_algorithm_tests {
        use super::*;

        #[test]
        fn test_compute_betweenness_chain() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
                make_test_port(3, "C", None, None, None),
                make_test_port(4, "D", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 2, 3, 100, "cargo"),
                make_test_voyage(3, 3, 4, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let betweenness = graph.compute_betweenness_bfs();

            let b1 = betweenness.get(&1).copied().unwrap_or(0.0);
            let b2 = betweenness.get(&2).copied().unwrap_or(0.0);
            let b3 = betweenness.get(&3).copied().unwrap_or(0.0);
            let b4 = betweenness.get(&4).copied().unwrap_or(0.0);

            assert_eq!(b1, 0.0);
            assert_eq!(b4, 0.0);
            assert!(b2 > 0.0);
            assert!(b3 > 0.0);
        }

        #[test]
        fn test_compute_betweenness_star() {
            let ports = vec![
                make_test_port(1, "Center", None, None, None),
                make_test_port(2, "A", None, None, None),
                make_test_port(3, "B", None, None, None),
                make_test_port(4, "C", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 1, 3, 100, "cargo"),
                make_test_voyage(3, 1, 4, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let betweenness = graph.compute_betweenness_bfs();

            let center_b = betweenness.get(&1).copied().unwrap_or(0.0);
            let a_b = betweenness.get(&2).copied().unwrap_or(0.0);
            let b_b = betweenness.get(&3).copied().unwrap_or(0.0);
            let c_b = betweenness.get(&4).copied().unwrap_or(0.0);

            assert!(center_b > a_b);
            assert!(center_b > b_b);
            assert!(center_b > c_b);
            assert_eq!(a_b, 0.0);
            assert_eq!(b_b, 0.0);
            assert_eq!(c_b, 0.0);
        }

        #[test]
        fn test_find_origin_ports() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
                make_test_port(3, "C", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 500, "cargo"),
                make_test_voyage(2, 2, 3, 300, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let origins = graph.find_origin_ports();

            assert!(origins.contains(&2));
            assert!(origins.contains(&3));
            assert_eq!(origins.len(), 2);
        }

        #[test]
        fn test_find_hub_ports() {
            let ports = vec![
                make_test_port(1, "Center", None, None, None),
                make_test_port(2, "A", None, None, None),
                make_test_port(3, "B", None, None, None),
                make_test_port(4, "C", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 1, 3, 100, "cargo"),
                make_test_voyage(3, 1, 4, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let hubs = graph.find_hub_ports(2);

            assert_eq!(hubs.len(), 2);
            assert_eq!(hubs[0], 1);
        }

        #[test]
        fn test_compute_adoption_levels() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 1, 2, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let adoption = graph.compute_adoption_levels();

            assert_eq!(adoption.get(&1).copied().unwrap_or(0.0), 1.0);
            assert_eq!(adoption.get(&2).copied().unwrap_or(0.0), 0.5);
        }
    }

    mod tech_diffusion_tests {
        use super::*;

        #[test]
        fn test_simulator_new() {
            let ports = vec![
                make_test_port(1, "A", None, Some(0.0), Some(0.0)),
                make_test_port(2, "B", None, Some(1.0), Some(1.0)),
            ];
            let voyages = vec![make_test_voyage(1, 1, 2, 100, "cargo")];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let config = default_test_config();
            let simulator = TechDiffusionSimulator::new(graph, config);

            assert_eq!(simulator.activated_ports().len(), 0);
        }

        #[test]
        fn test_seed_origin() {
            let ports = vec![
                make_test_port(1, "A", None, Some(0.0), Some(0.0)),
                make_test_port(2, "B", None, Some(1.0), Some(1.0)),
            ];
            let voyages = vec![make_test_voyage(1, 1, 2, 100, "cargo")];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let config = default_test_config();
            let mut simulator = TechDiffusionSimulator::new(graph, config);

            simulator.seed_origin(1, 100);

            let activated = simulator.activated_ports();
            assert_eq!(activated.len(), 1);
            assert!(activated.contains(&1));
            assert_eq!(simulator.get_activation_year(1), Some(100));
        }

        #[test]
        fn test_simulate_single_step_diffusion() {
            let ports = vec![
                make_test_port(1, "A", None, Some(0.0), Some(0.0)),
                make_test_port(2, "B", None, Some(0.0), Some(1.0)),
            ];
            let voyages = vec![make_test_voyage(1, 1, 2, 100, "cargo")];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let mut config = default_test_config();
            config.min_spread_threshold = 0.0;
            config.diffusion_decay_rate = 2.0;
            config.max_propagation_steps = 100;
            let mut simulator = TechDiffusionSimulator::new(graph, config);

            simulator.seed_origin(1, 100);
            simulator.simulate(100000.0);

            let activated = simulator.activated_ports();
            assert!(activated.len() >= 1);
        }

        #[test]
        fn test_diffusion_from_single_source_connected() {
            let ports = vec![
                make_test_port(1, "A", None, Some(0.0), Some(0.0)),
                make_test_port(2, "B", None, Some(0.0), Some(1.0)),
                make_test_port(3, "C", None, Some(0.0), Some(2.0)),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 2, 3, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let mut config = default_test_config();
            config.min_spread_threshold = 0.0;
            config.diffusion_decay_rate = 2.0;
            config.max_propagation_steps = 100;
            let mut simulator = TechDiffusionSimulator::new(graph, config);

            simulator.seed_origin(1, 100);
            simulator.simulate(100000.0);

            let activated = simulator.activated_ports();
            assert!(activated.len() >= 1);
        }

        #[test]
        fn test_diffusion_time_gradient() {
            let ports = vec![
                make_test_port(1, "A", None, Some(0.0), Some(0.0)),
                make_test_port(2, "B", None, Some(0.0), Some(10.0)),
                make_test_port(3, "C", None, Some(0.0), Some(20.0)),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 2, 3, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let mut config = default_test_config();
            config.min_spread_threshold = 0.0;
            config.diffusion_decay_rate = 2.0;
            config.max_propagation_steps = 100;
            let mut simulator = TechDiffusionSimulator::new(graph, config);

            simulator.seed_origin(1, 100);
            simulator.simulate(100.0);

            let year1 = simulator.get_activation_year(1);
            let year2 = simulator.get_activation_year(2);
            let year3 = simulator.get_activation_year(3);

            assert_eq!(year1, Some(100));
            if let Some(y2) = year2 {
                assert!(y2 >= 100);
            }
            if let (Some(y2), Some(y3)) = (year2, year3) {
                assert!(y3 >= y2);
            }
        }

        #[test]
        fn test_diffusion_path() {
            let ports = vec![
                make_test_port(1, "A", None, Some(0.0), Some(0.0)),
                make_test_port(2, "B", None, Some(0.0), Some(1.0)),
            ];
            let voyages = vec![make_test_voyage(1, 1, 2, 100, "cargo")];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let mut config = default_test_config();
            config.min_spread_threshold = 0.0;
            config.diffusion_decay_rate = 2.0;
            let mut simulator = TechDiffusionSimulator::new(graph, config);

            simulator.seed_origin(1, 100);
            simulator.simulate(100000.0);

            let path = simulator.get_diffusion_path(1);
            assert_eq!(path.len(), 1);
            assert_eq!(path[0], 1);
        }
    }

    mod cultural_diversity_tests {
        use super::*;

        #[test]
        fn test_compute_cultural_diversity_multi_region() {
            let ports = vec![
                make_test_port(1, "PortA", Some("Region1"), None, None),
                make_test_port(2, "PortB", Some("Region2"), None, None),
                make_test_port(3, "PortC", Some("Region3"), None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "spice"),
                make_test_voyage(2, 3, 2, 100, "silk"),
            ];
            let diversity = compute_cultural_diversity_index(&voyages, &ports);

            let port2_div = diversity.get(&2).copied().unwrap_or(0.0);
            assert!(port2_div >= 2.0);
        }

        #[test]
        fn test_compute_cultural_diversity_single_region_lower_than_multi() {
            let ports_single = vec![
                make_test_port(1, "PortA", Some("Region1"), None, None),
                make_test_port(2, "PortB", Some("Region1"), None, None),
                make_test_port(3, "PortC", Some("Region1"), None, None),
            ];
            let voyages_single = vec![
                make_test_voyage(1, 1, 2, 100, "spice"),
                make_test_voyage(2, 3, 2, 100, "silk"),
            ];
            let diversity_single = compute_cultural_diversity_index(&voyages_single, &ports_single);

            let ports_multi = vec![
                make_test_port(1, "PortA", Some("Region1"), None, None),
                make_test_port(2, "PortB", Some("Region2"), None, None),
                make_test_port(3, "PortC", Some("Region3"), None, None),
            ];
            let voyages_multi = vec![
                make_test_voyage(1, 1, 2, 100, "spice"),
                make_test_voyage(2, 3, 2, 100, "silk"),
            ];
            let diversity_multi = compute_cultural_diversity_index(&voyages_multi, &ports_multi);

            let port2_single = diversity_single.get(&2).copied().unwrap_or(0.0);
            let port2_multi = diversity_multi.get(&2).copied().unwrap_or(0.0);
            assert!(port2_multi > port2_single);
        }

        #[test]
        fn test_identify_cross_civilization_routes() {
            let ports = vec![
                make_test_port(1, "PortA", Some("Region1"), None, None),
                make_test_port(2, "PortB", Some("Region2"), None, None),
                make_test_port(3, "PortC", Some("Region1"), None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 1, 3, 100, "cargo"),
                make_test_voyage(3, 2, 3, 200, "cargo"),
            ];
            let routes = identify_cross_civilization_routes(&voyages, &ports);

            assert!(routes.len() >= 1);
        }
    }

    mod technology_preset_tests {
        use super::*;

        #[test]
        fn test_get_technology_presets_not_empty() {
            let presets = get_technology_presets();
            assert!(!presets.is_empty());
        }

        #[test]
        fn test_each_preset_has_name_and_keywords() {
            let presets = get_technology_presets();
            for preset in &presets {
                assert!(!preset.name.is_empty());
                assert!(!preset.name_zh.is_empty());
                assert!(!preset.origin_keywords.is_empty());
            }
        }

        #[test]
        fn test_find_origin_port_by_keyword_match_name() {
            let ports = vec![
                make_test_port(1, "Quanzhou Port", Some("China"), None, None),
                make_test_port(2, "Alexandria", Some("Egypt"), None, None),
            ];
            let keywords = vec!["quanzhou".to_string()];
            let result = find_origin_port_by_keyword(&ports, &keywords);
            assert_eq!(result, Some(1));
        }

        #[test]
        fn test_find_origin_port_by_keyword_match_region() {
            let ports = vec![
                make_test_port(1, "PortA", Some("Levant Coast"), None, None),
                make_test_port(2, "PortB", Some("Greece"), None, None),
            ];
            let keywords = vec!["levant".to_string()];
            let result = find_origin_port_by_keyword(&ports, &keywords);
            assert_eq!(result, Some(1));
        }

        #[test]
        fn test_find_origin_port_by_keyword_no_match() {
            let ports = vec![
                make_test_port(1, "PortA", Some("Region1"), None, None),
                make_test_port(2, "PortB", Some("Region2"), None, None),
            ];
            let keywords = vec!["nonexistent".to_string()];
            let result = find_origin_port_by_keyword(&ports, &keywords);
            assert_eq!(result, None);
        }
    }

    mod boundary_tests {
        use super::*;

        #[test]
        fn test_empty_voyages_empty_graph_edges() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
            ];
            let voyages: Vec<VoyageRecord> = Vec::new();
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            assert_eq!(graph.edges().len(), 0);
            assert_eq!(graph.nodes().len(), 2);
        }

        #[test]
        fn test_single_node_no_edges() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
            ];
            let voyages: Vec<VoyageRecord> = Vec::new();
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            assert_eq!(graph.nodes().len(), 1);
            assert_eq!(graph.edges().len(), 0);
        }

        #[test]
        fn test_self_loop_voyage() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 1, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            assert_eq!(graph.edges().len(), 1);
            assert_eq!(graph.get_trade_volume(1), 2.0);
        }

        #[test]
        fn test_duplicate_routes_weight_accumulates() {
            let ports = vec![
                make_test_port(1, "A", None, None, None),
                make_test_port(2, "B", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "cargo"),
                make_test_voyage(2, 1, 2, 100, "cargo"),
                make_test_voyage(3, 1, 2, 100, "cargo"),
            ];
            let graph = SpreadGraph::from_voyages(&voyages, &ports);
            let edges = graph.edges();
            assert_eq!(edges.len(), 1);
            assert_eq!(edges[0].2, 3.0);
        }

        #[test]
        fn test_default_impl() {
            let graph = SpreadGraph::default();
            assert_eq!(graph.nodes().len(), 0);
            assert_eq!(graph.edges().len(), 0);
        }

        #[test]
        fn test_diffusion_node_new() {
            let node = DiffusionNode::new(42);
            assert_eq!(node.port_id, 42);
            assert!(!node.activated);
            assert!(node.activation_year.is_none());
            assert!(node.source_port.is_none());
            assert_eq!(node.diffusion_probability, 0.0);
        }

        #[test]
        fn test_get_cargo_spread_records() {
            let voyages = vec![
                make_test_voyage(1, 1, 2, 200, "spice"),
                make_test_voyage(2, 2, 3, 100, "spice"),
                make_test_voyage(3, 1, 3, 150, "silk"),
            ];
            let records = get_cargo_spread_records(&voyages, "spice");
            assert_eq!(records.len(), 2);
            assert_eq!(records[0].voyage_year, 100);
            assert_eq!(records[1].voyage_year, 200);
        }

        #[test]
        fn test_build_cargo_spread_network() {
            let ports = vec![
                make_test_port(1, "PortA", None, None, None),
                make_test_port(2, "PortB", None, None, None),
            ];
            let voyages = vec![
                make_test_voyage(1, 1, 2, 100, "spice"),
                make_test_voyage(2, 1, 2, 200, "silk"),
            ];
            let network = build_cargo_spread_network(&voyages, &ports, "spice", 5);
            assert_eq!(network.nodes.len(), 2);
            assert_eq!(network.edges.len(), 1);
        }
    }
}
