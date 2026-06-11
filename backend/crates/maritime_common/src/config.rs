use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub voyage_loader: VoyageLoaderConfig,
    pub network_analyzer: NetworkAnalyzerConfig,
    pub storm_risk_modeler: StormRiskModelerConfig,
    pub maritime_insights: MaritimeInsightsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaritimeInsightsConfig {
    pub port: u16,
    pub metrics_port: u16,
    pub panel_regression: PanelRegressionConfig,
    pub granger_causality: GrangerCausalityConfig,
    pub route_planning: RoutePlanningConfig,
    pub cargo_spread: CargoSpreadConfig,
    pub modern_comparison: ModernComparisonConfig,
}

impl Default for MaritimeInsightsConfig {
    fn default() -> Self {
        MaritimeInsightsConfig {
            port: 3004,
            metrics_port: 9004,
            panel_regression: PanelRegressionConfig::default(),
            granger_causality: GrangerCausalityConfig::default(),
            route_planning: RoutePlanningConfig::default(),
            cargo_spread: CargoSpreadConfig::default(),
            modern_comparison: ModernComparisonConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PanelRegressionConfig {
    pub significance_level: f64,
    pub min_observations: usize,
    pub max_predictors: usize,
}

impl Default for PanelRegressionConfig {
    fn default() -> Self {
        PanelRegressionConfig {
            significance_level: 0.05,
            min_observations: 30,
            max_predictors: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrangerCausalityConfig {
    pub max_lags: usize,
    pub significance_level: f64,
    pub min_observations: usize,
}

impl Default for GrangerCausalityConfig {
    fn default() -> Self {
        GrangerCausalityConfig {
            max_lags: 5,
            significance_level: 0.05,
            min_observations: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutePlanningConfig {
    pub grid_resolution_km: f64,
    pub max_iterations: usize,
    pub current_weight: f64,
    pub wind_weight: f64,
    pub storm_risk_weight: f64,
    pub distance_weight: f64,
    pub storm_risk_hard_threshold: f64,
    pub storm_risk_soft_threshold: f64,
    pub max_detour_ratio: f64,
}

impl Default for RoutePlanningConfig {
    fn default() -> Self {
        RoutePlanningConfig {
            grid_resolution_km: 50.0,
            max_iterations: 10000,
            current_weight: 1.0,
            wind_weight: 0.5,
            storm_risk_weight: 10.0,
            distance_weight: 1.0,
            storm_risk_hard_threshold: 0.8,
            storm_risk_soft_threshold: 0.5,
            max_detour_ratio: 1.5,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CargoSpreadConfig {
    pub min_spread_threshold: f64,
    pub diffusion_decay_rate: f64,
    pub max_propagation_steps: usize,
}

impl Default for CargoSpreadConfig {
    fn default() -> Self {
        CargoSpreadConfig {
            min_spread_threshold: 0.01,
            diffusion_decay_rate: 0.5,
            max_propagation_steps: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModernComparisonConfig {
    pub modern_risk_multiplier: f64,
    pub tech_improvement_factor: f64,
    pub weather_forecast_accuracy: f64,
    pub stream_batch_size: usize,
    pub stream_flush_interval_ms: u64,
    pub spatial_grid_km: f64,
    pub enable_streaming: bool,
}

impl Default for ModernComparisonConfig {
    fn default() -> Self {
        ModernComparisonConfig {
            modern_risk_multiplier: 0.8,
            tech_improvement_factor: 0.3,
            weather_forecast_accuracy: 0.8,
            stream_batch_size: 1000,
            stream_flush_interval_ms: 5000,
            spatial_grid_km: 100.0,
            enable_streaming: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/ancient_maritime".to_string(),
            max_connections: 20,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VoyageLoaderConfig {
    pub port: u16,
    pub max_query_limit: i64,
}

impl Default for VoyageLoaderConfig {
    fn default() -> Self {
        VoyageLoaderConfig {
            port: 3001,
            max_query_limit: 2000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkAnalyzerConfig {
    pub port: u16,
    pub betweenness_sample_size: usize,
    pub community_max_iterations: usize,
    pub hub_top_k: usize,
}

impl Default for NetworkAnalyzerConfig {
    fn default() -> Self {
        NetworkAnalyzerConfig {
            port: 3002,
            betweenness_sample_size: 20,
            community_max_iterations: 10,
            hub_top_k: 5,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StormRiskModelerConfig {
    pub port: u16,
    pub logistic_regression: LogisticRegressionConfig,
    pub random_forest: RandomForestConfig,
}

impl Default for StormRiskModelerConfig {
    fn default() -> Self {
        StormRiskModelerConfig {
            port: 3003,
            logistic_regression: LogisticRegressionConfig::default(),
            random_forest: RandomForestConfig::default(),
        }
    }
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

impl Default for LogisticRegressionConfig {
    fn default() -> Self {
        LogisticRegressionConfig {
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

impl Default for RandomForestConfig {
    fn default() -> Self {
        RandomForestConfig {
            n_trees: 10,
            max_depth: 5,
            min_samples: 5,
            sample_ratio: 0.7,
        }
    }
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
            database: DatabaseConfig::default(),
            voyage_loader: VoyageLoaderConfig::default(),
            network_analyzer: NetworkAnalyzerConfig::default(),
            storm_risk_modeler: StormRiskModelerConfig::default(),
            maritime_insights: MaritimeInsightsConfig::default(),
        }
    }
}
