use std::collections::HashMap;
use crate::models::{StormRiskResult, StormHeatmapPoint, VoyageRecord};

#[derive(Debug, Clone)]
struct StormFeature {
    season_encoded: [f64; 4],
    route_distance: f64,
    is_autumn: f64,
    is_winter: f64,
    ship_size_factor: f64,
    cargo_risk_factor: f64,
    climate_storm_freq: f64,
}

impl StormFeature {
    fn to_vector(&self) -> Vec<f64> {
        vec![
            self.season_encoded[0],
            self.season_encoded[1],
            self.season_encoded[2],
            self.season_encoded[3],
            self.route_distance,
            self.is_autumn,
            self.is_winter,
            self.ship_size_factor,
            self.cargo_risk_factor,
            self.climate_storm_freq,
        ]
    }
}

fn encode_season(season: &str) -> [f64; 4] {
    match season {
        "spring" => [1.0, 0.0, 0.0, 0.0],
        "summer" => [0.0, 1.0, 0.0, 0.0],
        "autumn" => [0.0, 0.0, 1.0, 0.0],
        "winter" => [0.0, 0.0, 0.0, 1.0],
        _ => [0.25, 0.25, 0.25, 0.25],
    }
}

fn ship_size_factor(ship_type: &str) -> f64 {
    match ship_type {
        "trireme" => 0.7,
        "galley" => 0.6,
        "longship" => 0.65,
        "dhow" => 0.8,
        "merchant_round_ship" => 0.85,
        "junk" => 0.9,
        "carrack" => 1.0,
        "treasure_ship" => 1.1,
        _ => 0.8,
    }
}

fn cargo_risk_factor(cargo_type: &str) -> f64 {
    match cargo_type {
        "grain" | "timber" | "salt" => 0.6,
        "olive_oil" | "wine" => 0.7,
        "textiles" | "ceramics" => 0.8,
        "spices" | "incense" => 0.9,
        "gold" | "precious_stones" | "ivory" => 1.0,
        _ => 0.75,
    }
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

pub struct LogisticRegression {
    weights: Vec<f64>,
    bias: f64,
    learning_rate: f64,
    iterations: usize,
    l2_lambda: f64,
    prior_mean: Vec<f64>,
    prior_variance: f64,
}

impl LogisticRegression {
    pub fn new(learning_rate: f64, iterations: usize) -> Self {
        LogisticRegression {
            weights: Vec::new(),
            bias: 0.0,
            learning_rate,
            iterations,
            l2_lambda: 0.1,
            prior_mean: Vec::new(),
            prior_variance: 4.0,
        }
    }

    pub fn with_regularization(mut self, l2_lambda: f64) -> Self {
        self.l2_lambda = l2_lambda;
        self
    }

    pub fn with_bayesian_prior(mut self, prior_mean: Vec<f64>, prior_variance: f64) -> Self {
        self.prior_mean = prior_mean;
        self.prior_variance = prior_variance;
        self
    }

    pub fn fit(&mut self, features: &[Vec<f64>], labels: &[bool]) {
        if features.is_empty() {
            return;
        }
        let n_features = features[0].len();
        let n = features.len() as f64;

        let pos_count = labels.iter().filter(|&&l| l).count() as f64;
        let neg_count = n - pos_count;
        let prior_bias = if pos_count > 0.0 && neg_count > 0.0 {
            (pos_count / neg_count).ln()
        } else {
            0.0
        };

        if self.prior_mean.is_empty() {
            self.prior_mean = vec![0.0; n_features];
        }
        while self.prior_mean.len() < n_features {
            self.prior_mean.push(0.0);
        }

        self.weights = self.prior_mean.clone();
        self.bias = prior_bias;

        let effective_n = n.max(20.0);
        let prior_strength = effective_n / self.prior_variance;

        for _ in 0..self.iterations {
            let mut dw = vec![0.0; n_features];
            let mut db = 0.0;

            for (i, feat) in features.iter().enumerate() {
                let z = feat.iter().zip(self.weights.iter()).map(|(f, w)| f * w).sum::<f64>() + self.bias;
                let pred = sigmoid(z);
                let error = pred - if labels[i] { 1.0 } else { 0.0 };

                for (j, f) in feat.iter().enumerate() {
                    dw[j] += error * f;
                }
                db += error;
            }

            for (j, w) in self.weights.iter_mut().enumerate() {
                let l2_grad = self.l2_lambda * *w;
                let prior_grad = prior_strength * (*w - self.prior_mean[j]) / effective_n;
                *w -= self.learning_rate * (dw[j] / n + l2_grad / n + prior_grad);
            }
            self.bias -= self.learning_rate * db / n;
        }
    }

    pub fn predict_proba(&self, feature: &[f64]) -> f64 {
        let z = feature.iter().zip(self.weights.iter()).map(|(f, w)| f * w).sum::<f64>() + self.bias;
        let raw = sigmoid(z);
        let prior_prob = 0.15;
        let shrinkage = 1.0 / (1.0 + 5.0 / (feature.len() as f64).max(1.0));
        shrinkage * raw + (1.0 - shrinkage) * prior_prob
    }
}

struct DecisionTreeNode {
    feature_idx: Option<usize>,
    threshold: Option<f64>,
    left: Option<Box<DecisionTreeNode>>,
    right: Option<Box<DecisionTreeNode>>,
    probability: Option<f64>,
}

impl DecisionTreeNode {
    fn leaf(prob: f64) -> Self {
        DecisionTreeNode {
            feature_idx: None,
            threshold: None,
            left: None,
            right: None,
            probability: Some(prob),
        }
    }

    fn predict(&self, features: &[f64]) -> f64 {
        if let Some(prob) = self.probability {
            return prob;
        }
        let idx = self.feature_idx.unwrap();
        let thresh = self.threshold.unwrap();
        if features[idx] <= thresh {
            self.left.as_ref().unwrap().predict(features)
        } else {
            self.right.as_ref().unwrap().predict(features)
        }
    }
}

pub struct RandomForest {
    trees: Vec<DecisionTreeNode>,
    n_trees: usize,
    max_depth: usize,
    min_samples: usize,
}

impl RandomForest {
    pub fn new(n_trees: usize, max_depth: usize, min_samples: usize) -> Self {
        RandomForest {
            trees: Vec::new(),
            n_trees,
            max_depth,
            min_samples,
        }
    }

    pub fn fit(&mut self, features: &[Vec<f64>], labels: &[bool]) {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        let n = features.len();
        if n == 0 {
            return;
        }

        for _ in 0..self.n_trees {
            let mut rng = thread_rng();
            let sample_size = (n as f64 * 0.7) as usize;
            let mut indices: Vec<usize> = (0..n).collect();
            indices.shuffle(&mut rng);
            let sample_indices: Vec<usize> = indices.iter().take(sample_size).cloned().collect();

            let sample_features: Vec<Vec<f64>> = sample_indices.iter().map(|&i| features[i].clone()).collect();
            let sample_labels: Vec<bool> = sample_indices.iter().map(|&i| labels[i]).collect();

            let n_features = features[0].len();
            let feature_indices: Vec<usize> = {
                let mut fi: Vec<usize> = (0..n_features).collect();
                fi.shuffle(&mut rng);
                fi.iter().take((n_features as f64).sqrt() as usize + 1).cloned().collect()
            };

            let tree = self.build_tree(&sample_features, &sample_labels, &feature_indices, 0);
            self.trees.push(tree);
        }
    }

    fn build_tree(
        &self,
        features: &[Vec<f64>],
        labels: &[bool],
        feature_indices: &[usize],
        depth: usize,
    ) -> DecisionTreeNode {
        let n = labels.len();
        let n_positive = labels.iter().filter(|&&l| l).count();

        if n == 0 {
            return DecisionTreeNode::leaf(0.5);
        }

        let prob = n_positive as f64 / n as f64;

        if depth >= self.max_depth || n < self.min_samples || n_positive == 0 || n_positive == n {
            return DecisionTreeNode::leaf(prob);
        }

        let mut best_gini = f64::INFINITY;
        let mut best_idx = 0;
        let mut best_thresh = 0.0;

        for &idx in feature_indices {
            let mut values: Vec<f64> = features.iter().map(|f| f[idx]).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            for i in 0..values.len().saturating_sub(1) {
                let thresh = (values[i] + values[i + 1]) / 2.0;
                let (mut left_pos, mut left_neg, mut right_pos, mut right_neg) = (0, 0, 0, 0);
                for (j, feat) in features.iter().enumerate() {
                    if feat[idx] <= thresh {
                        if labels[j] { left_pos += 1; } else { left_neg += 1; }
                    } else {
                        if labels[j] { right_pos += 1; } else { right_neg += 1; }
                    }
                }

                let n_left = (left_pos + left_neg) as f64;
                let n_right = (right_pos + right_neg) as f64;
                if n_left < 1.0 || n_right < 1.0 {
                    continue;
                }

                let gini_left = 1.0 - (left_pos as f64 / n_left).powi(2) - (left_neg as f64 / n_left).powi(2);
                let gini_right = 1.0 - (right_pos as f64 / n_right).powi(2) - (right_neg as f64 / n_right).powi(2);
                let gini = (n_left * gini_left + n_right * gini_right) / n as f64;

                if gini < best_gini {
                    best_gini = gini;
                    best_idx = idx;
                    best_thresh = thresh;
                }
            }
        }

        let (left_features, left_labels, right_features, right_labels) = {
            let mut lf = Vec::new();
            let mut ll = Vec::new();
            let mut rf = Vec::new();
            let mut rl = Vec::new();
            for (i, feat) in features.iter().enumerate() {
                if feat[best_idx] <= best_thresh {
                    lf.push(feat.clone());
                    ll.push(labels[i]);
                } else {
                    rf.push(feat.clone());
                    rl.push(labels[i]);
                }
            }
            (lf, ll, rf, rl)
        };

        let left = self.build_tree(&left_features, &left_labels, feature_indices, depth + 1);
        let right = self.build_tree(&right_features, &right_labels, feature_indices, depth + 1);

        DecisionTreeNode {
            feature_idx: Some(best_idx),
            threshold: Some(best_thresh),
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
            probability: None,
        }
    }

    pub fn predict_proba(&self, feature: &[f64]) -> f64 {
        if self.trees.is_empty() {
            return 0.5;
        }
        let sum: f64 = self.trees.iter().map(|t| t.predict(feature)).sum();
        sum / self.trees.len() as f64
    }
}

fn extract_features(voyages: &[VoyageRecord]) -> Vec<StormFeature> {
    voyages.iter().map(|v| {
        let season_enc = encode_season(&v.season);
        let is_autumn = if v.season == "autumn" { 1.0 } else { 0.0 };
        let is_winter = if v.season == "winter" { 1.0 } else { 0.0 };
        let ship_f = ship_size_factor(&v.ship_type);
        let cargo_f = cargo_risk_factor(&v.cargo_type);
        let route_dist = if v.route_points.is_some() {
            let pts = v.route_points.as_ref().unwrap();
            if let Some(arr) = pts.as_array() {
                arr.len() as f64 * 50.0
            } else {
                500.0
            }
        } else {
            500.0
        };

        StormFeature {
            season_encoded: season_enc,
            route_distance: route_dist,
            is_autumn,
            is_winter,
            ship_size_factor: ship_f,
            cargo_risk_factor: cargo_f,
            climate_storm_freq: if is_autumn > 0.0 || is_winter > 0.0 { 0.2 } else { 0.1 },
        }
    }).collect()
}

pub fn analyze_storm_risk(
    voyages: &[VoyageRecord],
    model_type: &str,
) -> (Vec<StormRiskResult>, Vec<StormHeatmapPoint>) {
    let features = extract_features(voyages);
    let feature_vectors: Vec<Vec<f64>> = features.iter().map(|f| f.to_vector()).collect();
    let labels: Vec<bool> = voyages.iter().map(|v| v.encountered_storm).collect();

    let route_season_key = |v: &VoyageRecord| {
        let lo = v.departure_port_id.min(v.arrival_port_id);
        let hi = v.departure_port_id.max(v.arrival_port_id);
        (lo, hi, v.season.clone())
    };

    let mut route_groups: HashMap<(i32, i32, String), Vec<usize>> = HashMap::new();
    for (i, v) in voyages.iter().enumerate() {
        route_groups.entry(route_season_key(v)).or_default().push(i);
    }

    let probabilities: Vec<f64> = if model_type == "random_forest" {
        let mut rf = RandomForest::new(10, 5, 5);
        rf.fit(&feature_vectors, &labels);
        feature_vectors.iter().map(|f| rf.predict_proba(f)).collect()
    } else {
        let n_positive = labels.iter().filter(|&&l| l).count();
        let l2_lambda = if n_positive < 50 { 1.0 } else if n_positive < 200 { 0.5 } else { 0.1 };
        let mut lr = LogisticRegression::new(0.01, 500)
            .with_regularization(l2_lambda)
            .with_bayesian_prior(vec![0.0; feature_vectors[0].len()], 4.0);
        lr.fit(&feature_vectors, &labels);
        feature_vectors.iter().map(|f| lr.predict_proba(f)).collect()
    };

    let total_positive = labels.iter().filter(|&&l| l).count() as f64;
    let total_n = labels.len() as f64;
    let global_rate = if total_n > 0.0 { total_positive / total_n } else { 0.15 };

    let mut risks = Vec::new();
    let mut heatmap_points = Vec::new();

    for ((dep_id, arr_id, season), indices) in &route_groups {
        let n = indices.len();
        let avg_prob: f64 = indices.iter().map(|&i| probabilities[i]).sum::<f64>() / n as f64;
        let storm_count = indices.iter().filter(|&&i| voyages[i].encountered_storm).count();
        let observed_rate = storm_count as f64 / n as f64;

        let shrinkage_weight = (n as f64) / (n as f64 + 10.0);
        let smoothed_risk = shrinkage_weight * ((avg_prob + observed_rate) / 2.0) + (1.0 - shrinkage_weight) * global_rate;

        let confidence = if n > 20 { 0.9 } else if n > 10 { 0.7 } else if n > 5 { 0.5 } else if n > 2 { 0.3 } else { 0.1 };

        risks.push(StormRiskResult {
            departure_port_id: *dep_id,
            arrival_port_id: *arr_id,
            departure_port_name: String::new(),
            arrival_port_name: String::new(),
            season: season.clone(),
            risk_score: smoothed_risk,
            sample_size: n as i32,
            model_type: model_type.to_string(),
            confidence,
        });

        for &i in indices {
            if voyages[i].encountered_storm {
                if let Some(ref pts) = voyages[i].route_points {
                    if let Some(arr) = pts.as_array() {
                        let mid = arr.len() / 2;
                        if let Some(mid_pt) = arr.get(mid) {
                            if let Some(coord_arr) = mid_pt.as_array() {
                                let lon = coord_arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let lat = coord_arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                heatmap_points.push(StormHeatmapPoint {
                                    lat,
                                    lon,
                                    intensity: probabilities[i],
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    (risks, heatmap_points)
}
