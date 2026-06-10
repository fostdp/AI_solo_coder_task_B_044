use std::collections::HashMap;
use axum::{
    extract::{Query, State},
    response::Json,
};
use sqlx::PgPool;
use maritime_common::models::*;
use maritime_common::config::StormRiskModelerConfig;

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
    prior_storm_rate: f64,
    prediction_shrinkage_k: f64,
}

impl LogisticRegression {
    pub fn new_from_config(
        lr_config: &maritime_common::config::LogisticRegressionConfig,
        l2_lambda: f64,
        n_features: usize,
    ) -> Self {
        LogisticRegression {
            weights: Vec::new(),
            bias: 0.0,
            learning_rate: lr_config.learning_rate,
            iterations: lr_config.iterations,
            l2_lambda,
            prior_mean: vec![0.0; n_features],
            prior_variance: lr_config.prior_variance,
            prior_storm_rate: lr_config.prior_storm_rate,
            prediction_shrinkage_k: lr_config.prediction_shrinkage_k,
        }
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
        let shrinkage = 1.0 / (1.0 + self.prediction_shrinkage_k / (feature.len() as f64).max(1.0));
        shrinkage * raw + (1.0 - shrinkage) * self.prior_storm_rate
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
    sample_ratio: f64,
}

impl RandomForest {
    pub fn new_from_config(rf_config: &maritime_common::config::RandomForestConfig) -> Self {
        RandomForest {
            trees: Vec::new(),
            n_trees: rf_config.n_trees,
            max_depth: rf_config.max_depth,
            min_samples: rf_config.min_samples,
            sample_ratio: rf_config.sample_ratio,
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
            let sample_size = (n as f64 * self.sample_ratio) as usize;
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

fn extract_features(voyages: &[VoyageRecord], storm_freq: f64) -> Vec<StormFeature> {
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
            climate_storm_freq: storm_freq,
        }
    }).collect()
}

fn analyze_storm_risk_with_config(
    voyages: &[VoyageRecord],
    model_type: &str,
    config: &StormRiskModelerConfig,
    storm_freq: f64,
) -> (Vec<StormRiskResult>, Vec<StormHeatmapPoint>) {
    let features = extract_features(voyages, storm_freq);
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
        let mut rf = RandomForest::new_from_config(&config.random_forest);
        rf.fit(&feature_vectors, &labels);
        feature_vectors.iter().map(|f| rf.predict_proba(f)).collect()
    } else {
        let n_positive = labels.iter().filter(|&&l| l).count();
        let l2_lambda = config.logistic_regression.l2_lambda_for_count(n_positive);
        let n_features = feature_vectors.first().map(|v| v.len()).unwrap_or(10);
        let mut lr = LogisticRegression::new_from_config(&config.logistic_regression, l2_lambda, n_features);
        lr.fit(&feature_vectors, &labels);
        feature_vectors.iter().map(|f| lr.predict_proba(f)).collect()
    };

    let total_positive = labels.iter().filter(|&&l| l).count() as f64;
    let total_n = labels.len() as f64;
    let global_rate = if total_n > 0.0 { total_positive / total_n } else { config.logistic_regression.prior_storm_rate };
    let smoothing_k = config.logistic_regression.smoothing_k;

    let mut risks = Vec::new();
    let mut heatmap_points = Vec::new();

    for ((dep_id, arr_id, season), indices) in &route_groups {
        let n = indices.len();
        let avg_prob: f64 = indices.iter().map(|&i| probabilities[i]).sum::<f64>() / n as f64;
        let storm_count = indices.iter().filter(|&&i| voyages[i].encountered_storm).count();
        let observed_rate = storm_count as f64 / n as f64;

        let shrinkage_weight = (n as f64) / (n as f64 + smoothing_k);
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

pub async fn get_storm_risk(
    State((pool, config)): State<(PgPool, StormRiskModelerConfig)>,
    Query(params): Query<StormRiskQuery>,
) -> Json<StormAnalysisResponse> {
    let year_start = params.year_start.unwrap_or(-1000);
    let year_end = params.year_end.unwrap_or(1800);
    let model_type = params.model_type.unwrap_or_else(|| "logistic_regression".to_string());

    let voyages = sqlx::query_as!(
        VoyageRecord,
        "SELECT id, departure_port_id, arrival_port_id, voyage_year, season, \
         ship_type, cargo_type, encountered_storm, route_points, created_at \
         FROM voyage_records WHERE voyage_year >= $1 AND voyage_year <= $2",
        year_start, year_end
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let ports = sqlx::query_as!(
        Port,
        "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon FROM ports"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let aliases = sqlx::query_as!(
        PortAlias,
        "SELECT id, port_id, alias_name, alias_name_zh, period_start, period_end, language, source \
         FROM port_aliases"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let name_index = PortNameIndex::build(&ports, &aliases);

    let mid_year = (year_start + year_end) / 2;
    let storm_freq = sqlx::query_scalar!(
        "SELECT storm_frequency FROM climate_periods WHERE period_start <= $1 AND period_end >= $1 LIMIT 1",
        mid_year
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(0.15);

    let (mut risks, heatmap) = analyze_storm_risk_with_config(&voyages, &model_type, &config, storm_freq);

    let mut port_map: HashMap<i32, String> = ports.iter().map(|p| (p.id, p.name.clone())).collect();
    for a in &aliases {
        if !port_map.contains_key(&a.port_id) {
            port_map.entry(a.port_id).or_insert_with(|| a.alias_name.clone());
        }
    }

    for risk in &mut risks {
        risk.departure_port_name = port_map.get(&risk.departure_port_id).cloned().unwrap_or_else(|| {
            name_index.lookup(&risk.departure_port_id.to_string())
                .and_then(|id| port_map.get(&id).cloned())
                .unwrap_or_default()
        });
        risk.arrival_port_name = port_map.get(&risk.arrival_port_id).cloned().unwrap_or_else(|| {
            name_index.lookup(&risk.arrival_port_id.to_string())
                .and_then(|id| port_map.get(&id).cloned())
                .unwrap_or_default()
        });
    }

    Json(StormAnalysisResponse {
        risks,
        heatmap,
        model_type,
    })
}
