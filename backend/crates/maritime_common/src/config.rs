use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub voyage_loader: VoyageLoaderConfig,
    pub network_analyzer: NetworkAnalyzerConfig,
    pub storm_risk_modeler: StormRiskModelerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VoyageLoaderConfig {
    pub port: u16,
    pub max_query_limit: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkAnalyzerConfig {
    pub port: u16,
    pub betweenness_sample_size: usize,
    pub community_max_iterations: usize,
    pub hub_top_k: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StormRiskModelerConfig {
    pub port: u16,
    pub logistic_regression: LogisticRegressionConfig,
    pub random_forest: RandomForestConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogisticRegressionConfig {
    pub learning_rate: f64,
    pub iterations: usize,
    pub l2_lambda_sparse: f64,
    pub l2_lambda_medium: f64,
    pub l2_lambda_dense: f64,
    pub sparse_threshold: usize,
    pub medium_threshold: usize,
    pub prior_variance: f64,
    pub prior_storm_rate: f64,
    pub prediction_shrinkage_k: f64,
    pub smoothing_k: f64,
}

impl LogisticRegressionConfig {
    pub fn l2_lambda_for_count(&self, positive_count: usize) -> f64 {
        if positive_count < self.sparse_threshold {
            self.l2_lambda_sparse
        } else if positive_count < self.medium_threshold {
            self.l2_lambda_medium
        } else {
            self.l2_lambda_dense
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RandomForestConfig {
    pub n_trees: usize,
    pub max_depth: usize,
    pub min_samples: usize,
    pub sample_ratio: f64,
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
        Self::load(&path).unwrap_or_else(|e| {
            eprintln!("Failed to load config from {}: {}, using defaults", path, e);
            Self::default()
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            database: DatabaseConfig {
                url: "postgres://postgres:postgres@localhost:5432/ancient_maritime".to_string(),
                max_connections: 20,
            },
            voyage_loader: VoyageLoaderConfig {
                port: 3001,
                max_query_limit: 2000,
            },
            network_analyzer: NetworkAnalyzerConfig {
                port: 3002,
                betweenness_sample_size: 20,
                community_max_iterations: 10,
                hub_top_k: 5,
            },
            storm_risk_modeler: StormRiskModelerConfig {
                port: 3003,
                logistic_regression: LogisticRegressionConfig {
                    learning_rate: 0.01,
                    iterations: 500,
                    l2_lambda_sparse: 1.0,
                    l2_lambda_medium: 0.5,
                    l2_lambda_dense: 0.1,
                    sparse_threshold: 50,
                    medium_threshold: 200,
                    prior_variance: 4.0,
                    prior_storm_rate: 0.15,
                    prediction_shrinkage_k: 5.0,
                    smoothing_k: 10.0,
                },
                random_forest: RandomForestConfig {
                    n_trees: 10,
                    max_depth: 5,
                    min_samples: 5,
                    sample_ratio: 0.7,
                },
            },
        }
    }
}
