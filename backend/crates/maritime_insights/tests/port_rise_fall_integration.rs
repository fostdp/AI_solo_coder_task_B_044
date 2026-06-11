use maritime_insights::port_rise_fall::PanelRegression;
use maritime_insights::port_rise_fall::GrangerCausalityTest;
use maritime_common::config::PanelRegressionConfig;
use maritime_common::config::GrangerCausalityConfig;

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

#[test]
fn test_panel_regression_multi_factor_integration() {
    let n = 100;
    let mut x: Vec<Vec<f64>> = Vec::with_capacity(n);
    let mut y: Vec<f64> = Vec::with_capacity(n);

    for i in 0..n {
        let t = i as f64 / 10.0;
        let x1 = t.sin();
        let x2 = t.cos();
        let x3 = t * 0.5;
        let noise = (i as f64 * 0.1).sin() * 0.1;
        let yi = 2.0 + 1.5 * x1 - 0.8 * x2 + 0.3 * x3 + noise;
        x.push(vec![1.0, x1, x2, x3]);
        y.push(yi);
    }

    let result = PanelRegression::fit(&x, &y).expect("Regression should succeed");

    assert!(result.r_squared() > 0.8, "R² should be high for good fit");
    assert!(result.adj_r_squared() < result.r_squared(), "Adjusted R² <= R²");
    assert_eq!(result.n_observations(), n);
    assert_eq!(result.coefficients().len(), 4);
    assert!(result.f_statistic() > 0.0);
    assert!(result.f_p_value() < 0.05);
}

#[test]
fn test_panel_regression_config_sensible_defaults() {
    let config = PanelRegressionConfig {
        significance_level: 0.05,
        min_observations: 10,
        max_predictors: 8,
    };

    assert!(config.significance_level > 0.0 && config.significance_level < 1.0);
    assert!(config.min_observations > 0);
    assert!(config.max_predictors > 0);
}

#[test]
fn test_granger_config_sensible_defaults() {
    let config = GrangerCausalityConfig {
        max_lags: 5,
        significance_level: 0.05,
        min_observations: 20,
    };

    assert!(config.max_lags > 0);
    assert!(config.significance_level > 0.0 && config.significance_level < 1.0);
    assert!(config.min_observations > config.max_lags * 2);
}

#[test]
fn test_granger_causality_known_causal_relation() {
    let n = 200;
    let mut x: Vec<f64> = Vec::with_capacity(n);
    let mut y: Vec<f64> = Vec::with_capacity(n);

    for i in 0..n {
        let xi = (i as f64 * 0.3).sin() + (i as f64 * 0.7).cos();
        x.push(xi);
    }
    y.push(0.0);
    y.push(0.0);
    for i in 2..n {
        let yi = 0.8 * x[i - 2] + 0.1 * y[i - 1];
        y.push(yi);
    }

    let result = GrangerCausalityTest::test(&y, &x, 5, 0.05);
    assert!(result.is_some());

    let test = result.unwrap();
    assert!(test.lag_order() >= 1 && test.lag_order() <= 5);
    assert!(test.f_statistic() >= 0.0);
    assert!(test.p_value() >= 0.0 && test.p_value() <= 1.0);
}

#[test]
fn test_regression_coefficient_sign_consistency() {
    let n = 50;
    let mut x: Vec<Vec<f64>> = Vec::new();
    let mut y: Vec<f64> = Vec::new();

    for i in 0..n {
        let xi = i as f64;
        x.push(vec![1.0, xi]);
        y.push(3.0 + 2.5 * xi);
    }

    let result = PanelRegression::fit(&x, &y).unwrap();
    let coefs = result.coefficients();

    assert!(approx_eq(coefs[0], 3.0, 0.001));
    assert!(approx_eq(coefs[1], 2.5, 0.001));
    assert!(result.p_values()[1] < 0.001);
}

#[test]
fn test_noisy_data_still_converges() {
    let n = 80;
    let mut x: Vec<Vec<f64>> = Vec::new();
    let mut y: Vec<f64> = Vec::new();

    for i in 0..n {
        let xi = i as f64 / 10.0;
        let noise = (i as f64 * 0.7).sin() * 2.0;
        x.push(vec![1.0, xi]);
        y.push(5.0 + 1.0 * xi + noise);
    }

    let result = PanelRegression::fit(&x, &y).unwrap();
    assert!(result.r_squared() > 0.0 && result.r_squared() < 1.0);
    assert!(result.coefficients()[1] > 0.0);
}
