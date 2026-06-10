use std::collections::{HashMap, HashSet};
use crate::models::{NetworkResult, TradeEdge, VoyageRecord, Port};

pub struct TradeNetwork {
    adjacency: HashMap<i32, HashMap<i32, f64>>,
    ports: HashMap<i32, Port>,
}

impl TradeNetwork {
    pub fn from_voyages(voyages: &[VoyageRecord], ports: &[Port]) -> Self {
        let mut adjacency: HashMap<i32, HashMap<i32, f64>> = HashMap::new();
        let ports_map: HashMap<i32, Port> = ports.iter().map(|p| (p.id, p.clone())).collect();

        for v in voyages {
            let entry = adjacency.entry(v.departure_port_id).or_insert_with(HashMap::new);
            *entry.entry(v.arrival_port_id).or_insert(0.0) += 1.0;
            let entry2 = adjacency.entry(v.arrival_port_id).or_insert_with(HashMap::new);
            *entry2.entry(v.departure_port_id).or_insert(0.0) += 0.5;
        }

        TradeNetwork {
            adjacency,
            ports: ports_map,
        }
    }

    pub fn compute_degree_centrality(&self) -> HashMap<i32, f64> {
        let n = self.ports.len().max(1) as f64;
        let mut centrality = HashMap::new();
        for (&port_id, neighbors) in &self.adjacency {
            let degree = neighbors.len() as f64;
            centrality.insert(port_id, degree / (n - 1.0));
        }
        for &port_id in self.ports.keys() {
            centrality.entry(port_id).or_insert(0.0);
        }
        centrality
    }

    pub fn compute_betweenness_centrality(&self) -> HashMap<i32, f64> {
        let all_ports: Vec<i32> = self.ports.keys().cloned().collect();
        let mut betweenness: HashMap<i32, f64> = HashMap::new();
        for &p in &all_ports {
            betweenness.insert(p, 0.0);
        }

        let sample_size = all_ports.len().min(20);
        let sources: Vec<i32> = all_ports.iter()
            .step_by((all_ports.len().max(1) / sample_size).max(1))
            .cloned()
            .collect();

        for s in &sources {
            let (dist, sigma, pred) = self.bfs(*s);

            let mut delta: HashMap<i32, f64> = HashMap::new();
            for &p in &all_ports {
                delta.insert(p, 0.0);
            }

            let mut sorted: Vec<(i32, f64)> = dist.iter()
                .filter(|(_, &d)| d.is_finite())
                .map(|(&p, &d)| (p, d))
                .collect();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (w, _) in &sorted {
                if w != s {
                    let sigma_w = sigma.get(w).copied().unwrap_or(0);
                    if sigma_w > 0 {
                        for &v in pred.get(w).unwrap_or(&Vec::new()) {
                            let sigma_v = sigma.get(&v).copied().unwrap_or(1).max(1);
                            let delta_v = delta.get(&v).copied().unwrap_or(0.0);
                            let delta_w = delta.get(w).copied().unwrap_or(0.0);
                            delta.insert(v, delta_v + (sigma_v as f64 / sigma_w as f64) * (1.0 + delta_w));
                        }
                    }
                    if *w != *s {
                        let bw = betweenness.get(w).copied().unwrap_or(0.0);
                        betweenness.insert(*w, bw + delta.get(w).copied().unwrap_or(0.0));
                    }
                }
            }
        }

        let n = all_ports.len() as f64;
        let scale = if n > 2 { 2.0 / ((n - 1.0) * (n - 2.0)) } else { 1.0 };
        for (_, v) in betweenness.iter_mut() {
            *v *= scale;
        }

        betweenness
    }

    fn bfs(&self, source: i32) -> (HashMap<i32, f64>, HashMap<i32, i32>, HashMap<i32, Vec<i32>>) {
        let mut dist: HashMap<i32, f64> = HashMap::new();
        let mut sigma: HashMap<i32, i32> = HashMap::new();
        let mut pred: HashMap<i32, Vec<i32>> = HashMap::new();

        dist.insert(source, 0.0);
        sigma.insert(source, 1);

        let mut queue = vec![source];
        let mut visited = HashSet::new();
        visited.insert(source);

        while !queue.is_empty() {
            let v = queue.remove(0);
            let neighbors = self.adjacency.get(&v);
            if let Some(neigh) = neighbors {
                for (&w, &weight) in neigh {
                    let new_dist = dist.get(&v).copied().unwrap_or(f64::INFINITY) + 1.0 / weight.max(0.01);
                    let current_dist = dist.get(&w).copied().unwrap_or(f64::INFINITY);
                    if new_dist < current_dist {
                        dist.insert(w, new_dist);
                        sigma.insert(w, sigma.get(&v).copied().unwrap_or(0));
                        pred.entry(w).or_insert_with(Vec::new).push(v);
                        if !visited.contains(&w) {
                            visited.insert(w);
                            queue.push(w);
                        }
                    } else if (new_dist - current_dist).abs() < 1e-10 {
                        let s = sigma.get(&v).copied().unwrap_or(0) + sigma.get(&w).copied().unwrap_or(0);
                        sigma.insert(w, s);
                        pred.entry(w).or_insert_with(Vec::new).push(v);
                    }
                }
            }
        }

        (dist, sigma, pred)
    }

    pub fn detect_communities(&self) -> HashMap<i32, i32> {
        let mut community: HashMap<i32, i32> = HashMap::new();
        let all_ports: Vec<i32> = self.ports.keys().cloned().collect();
        for (i, &p) in all_ports.iter().enumerate() {
            community.insert(p, i as i32);
        }

        for _ in 0..10 {
            let mut changed = false;
            for &p in &all_ports {
                let neighbors = self.adjacency.get(&p);
                if neighbors.is_none() || neighbors.unwrap().is_empty() {
                    continue;
                }
                let mut comm_weights: HashMap<i32, f64> = HashMap::new();
                for (&n, &w) in neighbors.unwrap() {
                    let c = community.get(&n).copied().unwrap_or(n);
                    *comm_weights.entry(c).or_insert(0.0) += w;
                }
                if let Some((&best_comm, _)) = comm_weights.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)) {
                    let current = community.get(&p).copied().unwrap_or(p);
                    if best_comm != current {
                        community.insert(p, best_comm);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let mut remap: HashMap<i32, i32> = HashMap::new();
        let mut next_id = 0i32;
        for &c in community.values() {
            if !remap.contains_key(&c) {
                remap.insert(c, next_id);
                next_id += 1;
            }
        }
        for (_, v) in community.iter_mut() {
            *v = remap[v];
        }

        community
    }

    pub fn compute_trade_flow(&self) -> HashMap<i32, f64> {
        let mut flow: HashMap<i32, f64> = HashMap::new();
        for &p in self.ports.keys() {
            flow.insert(p, 0.0);
        }
        for (&src, neighbors) in &self.adjacency {
            for (&dst, &w) in neighbors {
                *flow.entry(src).or_insert(0.0) += w;
                *flow.entry(dst).or_insert(0.0) += w * 0.5;
            }
        }
        flow
    }

    pub fn get_edges(&self) -> Vec<TradeEdge> {
        let mut seen = HashSet::new();
        let mut edges = Vec::new();
        for (&src, neighbors) in &self.adjacency {
            for (&dst, &w) in neighbors {
                let key = if src < dst { (src, dst) } else { (dst, src) };
                if seen.insert(key) {
                    edges.push(TradeEdge {
                        source: src,
                        target: dst,
                        weight: w,
                    });
                }
            }
        }
        edges
    }

    pub fn analyze(&self, period_start: i32, period_end: i32) -> (Vec<NetworkResult>, Vec<TradeEdge>) {
        let bc = self.compute_betweenness_centrality();
        let dc = self.compute_degree_centrality();
        let flow = self.compute_trade_flow();
        let communities = self.detect_communities();

        let bc_values: Vec<f64> = bc.values().copied().collect();
        let hub_threshold = if bc_values.is_empty() { 0.0 } else {
            let mut sorted = bc_values.clone();
            sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            sorted[sorted.len().min(5).saturating_sub(1)]
        };

        let mut nodes = Vec::new();
        for (&port_id, port) in &self.ports {
            let betweenness = bc.get(&port_id).copied().unwrap_or(0.0);
            let degree = dc.get(&port_id).copied().unwrap_or(0.0);
            let trade_flow = flow.get(&port_id).copied().unwrap_or(0.0);
            let community = communities.get(&port_id).copied().unwrap_or(0);
            let is_hub = betweenness >= hub_threshold && betweenness > 0.0;

            nodes.push(NetworkResult {
                port_id,
                port_name: port.name.clone(),
                port_name_zh: port.name_zh.clone(),
                lat: port.lat.unwrap_or(0.0),
                lon: port.lon.unwrap_or(0.0),
                betweenness_centrality: betweenness,
                degree_centrality: degree,
                trade_flow,
                community_id: community,
                is_hub,
            });
        }

        let edges = self.get_edges();
        (nodes, edges)
    }
}
