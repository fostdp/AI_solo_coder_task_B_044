use maritime_common::config::MaritimeInsightsConfig;
use maritime_common::models::{
    ClimatePeriod, FactorWeight, GrangerCausalityResult, HistoricalEvent, PanelRegressionResult,
    Port, PortRiseFallResponse, PortYearlyFlow, RegressionCoefficient,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

fn ln_gamma(z: f64) -> f64 {
    let g = 7.0;
    let c = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];

    if z < 0.5 {
        let pi = std::f64::consts::PI;
        (pi / (std::f64::consts::PI * z).sin()).ln() - ln_gamma(1.0 - z)
    } else {
        let z = z - 1.0;
        let mut x = c[0];
        for i in 1..=g as usize + 1 {
            x += c[i] / (z + i as f64);
        }
        let t = z + g + 0.5;
        0.5 * (2.0 * std::f64::consts::PI).ln() + (z + 0.5) * t.ln() - t + x.ln()
    }
}

fn beta(a: f64, b: f64) -> f64 {
    (ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)).exp()
}

fn betacf(a: f64, b: f64, x: f64) -> f64 {
    let max_iter = 200;
    let eps = 3.0e-7;
    let fpmin = 1.0e-30;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;

    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < fpmin {
        d = fpmin;
    }
    d = 1.0 / d;
    let mut h = d;

    for m in 1..=max_iter {
        let m2 = 2.0 * m as f64;
        let mut aa = m as f64 * (b - m as f64) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < fpmin {
            d = fpmin;
        }
        c = 1.0 + aa / c;
        if c.abs() < fpmin {
            c = fpmin;
        }
        d = 1.0 / d;
        h *= d * c;

        aa = -(a + m as f64) * (qab + m as f64) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < fpmin {
            d = fpmin;
        }
        c = 1.0 + aa / c;
        if c.abs() < fpmin {
            c = fpmin;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;

        if (del - 1.0).abs() < eps {
            break;
        }
    }
    h
}

fn regularized_incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
    if x < 0.0 || x > 1.0 {
        return f64::NAN;
    }
    if x == 0.0 || x == 1.0 {
        return x;
    }

    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();

    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(a, b, x) / a
    } else {
        1.0 - bt * betacf(b, a, 1.0 - x) / b
    }
}

fn t_cdf(t: f64, df: f64) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    if t == 0.0 {
        return 0.5;
    }

    let x = df / (df + t * t);
    let p = 0.5 * regularized_incomplete_beta(df / 2.0, 0.5, x);

    if t > 0.0 {
        1.0 - p
    } else {
        p
    }
}

fn t_pvalue(t_stat: f64, df: f64) -> f64 {
    2.0 * (1.0 - t_cdf(t_stat.abs(), df))
}

fn f_cdf(f: f64, df1: f64, df2: f64) -> f64 {
    if f <= 0.0 || df1 <= 0.0 || df2 <= 0.0 {
        return 0.0;
    }
    let x = df1 * f / (df1 * f + df2);
    regularized_incomplete_beta(df1 / 2.0, df2 / 2.0, x)
}

fn f_pvalue(f_stat: f64, df1: f64, df2: f64) -> f64 {
    1.0 - f_cdf(f_stat, df1, df2)
}

fn mat_transpose(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if a.is_empty() {
        return Vec::new();
    }
    let rows = a.len();
    let cols = a[0].len();
    let mut result = vec![vec![0.0; rows]; cols];
    for i in 0..rows {
        for j in 0..cols {
            result[j][i] = a[i][j];
        }
    }
    result
}

fn mat_multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let a_rows = a.len();
    let a_cols = a[0].len();
    let b_cols = b[0].len();

    let mut result = vec![vec![0.0; b_cols]; a_rows];
    for i in 0..a_rows {
        for k in 0..a_cols {
            let aik = a[i][k];
            if aik.abs() < 1e-15 {
                continue;
            }
            for j in 0..b_cols {
                result[i][j] += aik * b[k][j];
            }
        }
    }
    result
}

fn mat_vec_multiply(a: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
    if a.is_empty() || v.is_empty() {
        return Vec::new();
    }
    let rows = a.len();
    let cols = a[0].len();
    let mut result = vec![0.0; rows];
    for i in 0..rows {
        let mut sum = 0.0;
        for j in 0..cols {
            sum += a[i][j] * v[j];
        }
        result[i] = sum;
    }
    result
}

fn mat_inverse(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    if n == 0 || a[0].len() != n {
        return None;
    }

    let mut aug = vec![vec![0.0; 2 * n]; n];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = a[i][j];
        }
        aug[i][n + i] = 1.0;
    }

    for col in 0..n {
        let mut pivot_row = col;
        let mut max_val = aug[col][col].abs();
        for row in col + 1..n {
            let val = aug[row][col].abs();
            if val > max_val {
                max_val = val;
                pivot_row = row;
            }
        }

        if max_val < 1e-15 {
            return None;
        }

        if pivot_row != col {
            aug.swap(col, pivot_row);
        }

        let pivot_val = aug[col][col];
        for j in col..2 * n {
            aug[col][j] /= pivot_val;
        }

        for row in 0..n {
            if row != col {
                let factor = aug[row][col];
                if factor.abs() > 1e-15 {
                    for j in col..2 * n {
                        aug[row][j] -= factor * aug[col][j];
                    }
                }
            }
        }
    }

    let mut inv = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            inv[i][j] = aug[i][n + j];
        }
    }
    Some(inv)
}

fn mat_diagonal(a: &[Vec<f64>]) -> Vec<f64> {
    a.iter().enumerate().map(|(i, row)| row[i]).collect()
}

pub struct PanelRegression {
    coefficients: Vec<f64>,
    standard_errors: Vec<f64>,
    t_stats: Vec<f64>,
    p_values: Vec<f64>,
    r_squared: f64,
    adj_r_squared: f64,
    f_statistic: f64,
    f_p_value: f64,
    n_obs: usize,
    n_vars: usize,
}

impl PanelRegression {
    pub fn fit(x: &[Vec<f64>], y: &[f64]) -> Option<Self> {
        let n = y.len();
        if n == 0 || x.is_empty() || x.len() != n {
            return None;
        }

        let k = x[0].len();
        if n <= k {
            return None;
        }

        let xt = mat_transpose(x);
        let xtx = mat_multiply(&xt, x);
        let xtx_inv = mat_inverse(&xtx)?;
        let xty = mat_vec_multiply(&xt, y);
        let beta = mat_vec_multiply(&xtx_inv, &xty);

        let y_hat = mat_vec_multiply(x, &beta);
        let mut rss = 0.0;
        let mut tss = 0.0;
        let y_mean = y.iter().sum::<f64>() / n as f64;

        for i in 0..n {
            let e = y[i] - y_hat[i];
            rss += e * e;
            let t = y[i] - y_mean;
            tss += t * t;
        }

        let df_resid = (n - k) as f64;
        let sigma_sq = rss / df_resid;

        let diag_xtx_inv = mat_diagonal(&xtx_inv);
        let se: Vec<f64> = diag_xtx_inv
            .iter()
            .map(|&d| (sigma_sq * d).sqrt())
            .collect();
        let t_stats: Vec<f64> = beta
            .iter()
            .zip(se.iter())
            .map(|(b, s)| if s.abs() < 1e-15 { 0.0 } else { b / s })
            .collect();
        let p_values: Vec<f64> = t_stats.iter().map(|&t| t_pvalue(t, df_resid)).collect();

        let r_squared = if tss < 1e-15 { 0.0 } else { 1.0 - rss / tss };
        let adj_r_squared = if n as f64 - k as f64 - 1.0 < 1e-15 {
            r_squared
        } else {
            1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / (n as f64 - k as f64)
        };

        let df_model = (k - 1) as f64;
        let f_statistic = if df_model <= 0.0 || rss < 1e-15 {
            0.0
        } else {
            ((tss - rss) / df_model) / (rss / df_resid)
        };
        let f_p_value = f_pvalue(f_statistic, df_model, df_resid);

        Some(PanelRegression {
            coefficients: beta,
            standard_errors: se,
            t_stats,
            p_values,
            r_squared,
            adj_r_squared,
            f_statistic,
            f_p_value,
            n_obs: n,
            n_vars: k,
        })
    }

    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }
    pub fn standard_errors(&self) -> &[f64] {
        &self.standard_errors
    }
    pub fn t_stats(&self) -> &[f64] {
        &self.t_stats
    }
    pub fn p_values(&self) -> &[f64] {
        &self.p_values
    }
    pub fn r_squared(&self) -> f64 {
        self.r_squared
    }
    pub fn adj_r_squared(&self) -> f64 {
        self.adj_r_squared
    }
    pub fn f_statistic(&self) -> f64 {
        self.f_statistic
    }
    pub fn f_p_value(&self) -> f64 {
        self.f_p_value
    }
    pub fn n_observations(&self) -> usize {
        self.n_obs
    }
}

pub struct GrangerCausalityTest {
    f_statistic: f64,
    p_value: f64,
    is_significant: bool,
    direction: String,
    lag_order: usize,
}

impl GrangerCausalityTest {
    pub fn test(y: &[f64], x: &[f64], max_lags: usize, significance_level: f64) -> Option<Self> {
        let n = y.len();
        if n < max_lags * 2 + 2 || x.len() != n || max_lags == 0 {
            return None;
        }

        let mut best_lag = 1;
        let mut best_bic = f64::INFINITY;

        for lag in 1..=max_lags {
            let n_eff = n - lag;
            if n_eff < lag + 2 {
                continue;
            }

            let (rss_rest, _) = Self::fit_restricted(y, lag);
            let k = lag + 1;
            let bic =
                (rss_rest / n_eff as f64).ln() + (k as f64) * (n_eff as f64).ln() / n_eff as f64;

            if bic < best_bic {
                best_bic = bic;
                best_lag = lag;
            }
        }

        let (rss_restricted, _) = Self::fit_restricted(y, best_lag);
        let (rss_unrestricted, x_coefs) = Self::fit_unrestricted(y, x, best_lag);

        let n_eff = (n - best_lag) as f64;
        let df_restricted = best_lag as f64 + 1.0;
        let df_unrestricted = (2 * best_lag + 1) as f64;
        let p = best_lag as f64;
        let df2 = n_eff - df_unrestricted;

        if df2 <= 0.0 || rss_unrestricted < 1e-15 {
            return None;
        }

        let f_stat = ((rss_restricted - rss_unrestricted) / p) / (rss_unrestricted / df2);
        let p_val = f_pvalue(f_stat, p, df2);

        let direction = if x_coefs.iter().sum::<f64>() > 0.0 {
            "positive".to_string()
        } else {
            "negative".to_string()
        };

        Some(GrangerCausalityTest {
            f_statistic: f_stat,
            p_value: p_val,
            is_significant: p_val < significance_level,
            direction,
            lag_order: best_lag,
        })
    }

    fn fit_restricted(y: &[f64], lags: usize) -> (f64, Vec<f64>) {
        let n = y.len();
        let n_eff = n - lags;
        if n_eff <= lags {
            return (f64::INFINITY, Vec::new());
        }

        let mut x = vec![vec![1.0; 1 + lags]; n_eff];
        let mut y_eff = vec![0.0; n_eff];

        for i in 0..n_eff {
            y_eff[i] = y[i + lags];
            for j in 0..lags {
                x[i][1 + j] = y[i + lags - 1 - j];
            }
        }

        let model = PanelRegression::fit(&x, &y_eff);
        if let Some(m) = model {
            let rss = (1.0 - m.r_squared())
                * y_eff
                    .iter()
                    .map(|&v| {
                        let mean = y_eff.iter().sum::<f64>() / n_eff as f64;
                        (v - mean).powi(2)
                    })
                    .sum::<f64>();
            (rss, m.coefficients().to_vec())
        } else {
            (f64::INFINITY, Vec::new())
        }
    }

    fn fit_unrestricted(y: &[f64], x: &[f64], lags: usize) -> (f64, Vec<f64>) {
        let n = y.len();
        let n_eff = n - lags;
        if n_eff <= 2 * lags + 1 {
            return (f64::INFINITY, Vec::new());
        }

        let mut xmat = vec![vec![1.0; 1 + 2 * lags]; n_eff];
        let mut y_eff = vec![0.0; n_eff];

        for i in 0..n_eff {
            y_eff[i] = y[i + lags];
            for j in 0..lags {
                xmat[i][1 + j] = y[i + lags - 1 - j];
            }
            for j in 0..lags {
                xmat[i][1 + lags + j] = x[i + lags - 1 - j];
            }
        }

        let model = PanelRegression::fit(&xmat, &y_eff);
        if let Some(m) = model {
            let rss = (1.0 - m.r_squared())
                * y_eff
                    .iter()
                    .map(|&v| {
                        let mean = y_eff.iter().sum::<f64>() / n_eff as f64;
                        (v - mean).powi(2)
                    })
                    .sum::<f64>();
            let x_coefs: Vec<f64> = m
                .coefficients()
                .iter()
                .skip(1 + lags)
                .take(lags)
                .copied()
                .collect();
            (rss, x_coefs)
        } else {
            (f64::INFINITY, Vec::new())
        }
    }

    pub fn f_statistic(&self) -> f64 {
        self.f_statistic
    }
    pub fn p_value(&self) -> f64 {
        self.p_value
    }
    pub fn is_significant(&self) -> bool {
        self.is_significant
    }
    pub fn direction(&self) -> &str {
        &self.direction
    }
    pub fn lag_order(&self) -> usize {
        self.lag_order
    }
}

struct PanelDataPoint {
    year: i32,
    total_flow: f64,
    avg_temperature: f64,
    storm_frequency: f64,
    nao_index: f64,
    war_count: f64,
    regime_changes: f64,
    trade_connections: f64,
    cargo_diversity: f64,
    storm_rate: f64,
}

fn build_climate_year_map(climate_periods: &[ClimatePeriod]) -> HashMap<i32, ClimatePeriod> {
    let mut map = HashMap::new();
    for period in climate_periods {
        for year in period.period_start..=period.period_end {
            map.insert(year, period.clone());
        }
    }
    map
}

fn count_events_per_year(events: &[HistoricalEvent], event_type: &str) -> HashMap<i32, i32> {
    let mut counts = HashMap::new();
    for event in events {
        if event.event_type != event_type {
            continue;
        }
        let start = event.start_year;
        let end = event.end_year.unwrap_or(start);
        for year in start..=end {
            *counts.entry(year).or_insert(0) += 1;
        }
    }
    counts
}

fn build_panel_data(
    flows: &[PortYearlyFlow],
    climate_map: &HashMap<i32, ClimatePeriod>,
    war_counts: &HashMap<i32, i32>,
    regime_counts: &HashMap<i32, i32>,
) -> Vec<PanelDataPoint> {
    let mut points = Vec::new();

    for flow in flows {
        let climate = climate_map.get(&flow.year);
        let war_count = *war_counts.get(&flow.year).unwrap_or(&0) as f64;
        let regime_changes = *regime_counts.get(&flow.year).unwrap_or(&0) as f64;

        let avg_temperature = climate.and_then(|c| c.avg_temperature).unwrap_or(0.0);
        let storm_frequency = climate.and_then(|c| c.storm_frequency).unwrap_or(0.0);
        let nao_index = climate.and_then(|c| c.nao_index).unwrap_or(0.0);

        points.push(PanelDataPoint {
            year: flow.year,
            total_flow: flow.total_flow as f64,
            avg_temperature,
            storm_frequency,
            nao_index,
            war_count,
            regime_changes,
            trade_connections: flow.unique_destinations as f64,
            cargo_diversity: flow.unique_cargo_types as f64,
            storm_rate: flow.storm_rate.unwrap_or(0.0),
        });
    }

    points.sort_by_key(|p| p.year);
    points
}

const VAR_NAMES: [(&str, &str); 8] = [
    ("avg_temperature", "平均温度"),
    ("storm_frequency", "风暴频率"),
    ("nao_index", "北大西洋涛动指数"),
    ("war_count", "战争事件数"),
    ("regime_changes", "政权更迭数"),
    ("trade_connections", "贸易连接数"),
    ("cargo_diversity", "货物多样性"),
    ("storm_rate", "风暴率"),
];

fn build_design_matrix(points: &[PanelDataPoint]) -> (Vec<Vec<f64>>, Vec<f64>) {
    let n = points.len();
    let mut x = vec![vec![1.0; 9]; n];
    let mut y = vec![0.0; n];

    for i in 0..n {
        let p = &points[i];
        y[i] = p.total_flow;
        x[i][1] = p.avg_temperature;
        x[i][2] = p.storm_frequency;
        x[i][3] = p.nao_index;
        x[i][4] = p.war_count;
        x[i][5] = p.regime_changes;
        x[i][6] = p.trade_connections;
        x[i][7] = p.cargo_diversity;
        x[i][8] = p.storm_rate;
    }

    (x, y)
}

fn run_panel_regression_for_port(
    port_id: i32,
    port_name: &str,
    flows: &[PortYearlyFlow],
    climate_map: &HashMap<i32, ClimatePeriod>,
    war_counts: &HashMap<i32, i32>,
    regime_counts: &HashMap<i32, i32>,
    config: &MaritimeInsightsConfig,
) -> Option<PanelRegressionResult> {
    let min_obs = config.panel_regression.min_observations;
    if flows.len() < min_obs {
        return None;
    }

    let points = build_panel_data(flows, climate_map, war_counts, regime_counts);
    if points.len() < min_obs {
        return None;
    }

    let (x, y) = build_design_matrix(&points);
    let model = PanelRegression::fit(&x, &y)?;

    let sig_level = config.panel_regression.significance_level;
    let mut coefficients = Vec::new();

    let intercept_coef = RegressionCoefficient {
        variable: "intercept".to_string(),
        variable_zh: "截距项".to_string(),
        coefficient: model.coefficients()[0],
        standard_error: model.standard_errors()[0],
        t_statistic: model.t_stats()[0],
        p_value: model.p_values()[0],
        is_significant: model.p_values()[0] < sig_level,
    };
    coefficients.push(intercept_coef);

    for i in 0..VAR_NAMES.len() {
        let (name, name_zh) = VAR_NAMES[i];
        let idx = i + 1;
        coefficients.push(RegressionCoefficient {
            variable: name.to_string(),
            variable_zh: name_zh.to_string(),
            coefficient: model.coefficients()[idx],
            standard_error: model.standard_errors()[idx],
            t_statistic: model.t_stats()[idx],
            p_value: model.p_values()[idx],
            is_significant: model.p_values()[idx] < sig_level,
        });
    }

    let period_start = points.first().map(|p| p.year).unwrap_or(0);
    let period_end = points.last().map(|p| p.year).unwrap_or(0);

    Some(PanelRegressionResult {
        port_id,
        port_name: port_name.to_string(),
        dependent_var: "total_flow".to_string(),
        model_type: "ols_panel".to_string(),
        period_start,
        period_end,
        coefficients,
        r_squared: model.r_squared(),
        adj_r_squared: model.adj_r_squared(),
        f_statistic: model.f_statistic(),
        p_value: model.f_p_value(),
        n_observations: model.n_observations() as i32,
    })
}

fn run_granger_tests_for_port(
    port_id: i32,
    port_name: &str,
    flows: &[PortYearlyFlow],
    climate_map: &HashMap<i32, ClimatePeriod>,
    war_counts: &HashMap<i32, i32>,
    regime_counts: &HashMap<i32, i32>,
    config: &MaritimeInsightsConfig,
) -> Vec<GrangerCausalityResult> {
    let mut results = Vec::new();
    let min_obs = config.granger_causality.min_observations;
    let max_lags = config.granger_causality.max_lags;
    let sig_level = config.granger_causality.significance_level;

    if flows.len() < min_obs {
        return results;
    }

    let points = build_panel_data(flows, climate_map, war_counts, regime_counts);
    if points.len() < min_obs {
        return results;
    }

    let y: Vec<f64> = points.iter().map(|p| p.total_flow).collect();

    let period_start = points.first().map(|p| p.year).unwrap_or(0);
    let period_end = points.last().map(|p| p.year).unwrap_or(0);

    let causal_vars: [(&str, &str, fn(&PanelDataPoint) -> f64); 6] = [
        ("avg_temperature", "平均温度", |p| p.avg_temperature),
        ("storm_frequency", "风暴频率", |p| p.storm_frequency),
        ("nao_index", "北大西洋涛动指数", |p| p.nao_index),
        ("war_count", "战争事件数", |p| p.war_count),
        ("regime_changes", "政权更迭数", |p| p.regime_changes),
        ("storm_rate", "风暴率", |p| p.storm_rate),
    ];

    for (cause_var, cause_var_zh, extractor) in &causal_vars {
        let x: Vec<f64> = points.iter().map(|p| extractor(p)).collect();

        if x.iter().all(|&v| v.abs() < 1e-15) {
            continue;
        }

        if let Some(test) = GrangerCausalityTest::test(&y, &x, max_lags, sig_level) {
            results.push(GrangerCausalityResult {
                port_id,
                cause_variable: cause_var.to_string(),
                cause_variable_zh: cause_var_zh.to_string(),
                effect_variable: "total_flow".to_string(),
                effect_variable_zh: "贸易流量".to_string(),
                lag_order: test.lag_order() as i32,
                f_statistic: test.f_statistic(),
                p_value: test.p_value(),
                is_significant: test.is_significant(),
                direction: test.direction().to_string(),
                period_start,
                period_end,
            });
        }
    }

    let _ = port_name;
    results
}

pub async fn get_port_rise_fall_analysis(
    pool: &PgPool,
    config: &MaritimeInsightsConfig,
    year_start: Option<i32>,
    year_end: Option<i32>,
    port_id: Option<i32>,
    region: Option<String>,
) -> PortRiseFallResponse {
    let ys = year_start.unwrap_or(-1000);
    let ye = year_end.unwrap_or(1800);

    let ports: Vec<Port> = if let Some(pid) = port_id {
        sqlx::query_as!(
            Port,
            "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon \
             FROM ports WHERE id = $1",
            pid
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else if let Some(ref reg) = region {
        sqlx::query_as!(
            Port,
            "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon \
             FROM ports WHERE region = $1",
            reg
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            Port,
            "SELECT id, name, name_zh, region, ST_Y(geom) as lat, ST_X(geom) as lon FROM ports"
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let port_ids: Vec<i32> = ports.iter().map(|p| p.id).collect();
    let port_names: HashMap<i32, String> = ports.iter().map(|p| (p.id, p.name.clone())).collect();

    let flows: Vec<PortYearlyFlow> = if port_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as!(
            PortYearlyFlow,
            "SELECT port_id, year, total_flow, departure_count, arrival_count, \
             storm_count, storm_rate, unique_cargo_types, unique_destinations, flow_rank \
             FROM port_yearly_flow WHERE year >= $1 AND year <= $2 AND port_id = ANY($3) \
             ORDER BY port_id, year",
            ys,
            ye,
            &port_ids
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let climate_periods: Vec<ClimatePeriod> = sqlx::query_as!(
        ClimatePeriod,
        "SELECT id, period_start, period_end, avg_temperature, avg_wind_speed, \
         avg_rainfall, storm_frequency, nao_index, description \
         FROM climate_periods WHERE period_end >= $1 AND period_start <= $2",
        ys,
        ye
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let historical_events: Vec<HistoricalEvent> = sqlx::query_as!(
        HistoricalEvent,
        "SELECT id, event_name, event_name_zh, event_type, region, start_year, \
         end_year, severity, description, lat, lon \
         FROM historical_events WHERE end_year >= $1 OR (end_year IS NULL AND start_year >= $1) \
         AND start_year <= $2",
        ys,
        ye
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let climate_map = build_climate_year_map(&climate_periods);
    let war_counts = count_events_per_year(&historical_events, "war");
    let regime_counts = count_events_per_year(&historical_events, "regime_change");

    let mut flows_by_port: HashMap<i32, Vec<PortYearlyFlow>> = HashMap::new();
    for flow in &flows {
        flows_by_port
            .entry(flow.port_id)
            .or_default()
            .push(flow.clone());
    }

    let mut regression_results = Vec::new();
    let mut granger_results = Vec::new();

    for port in &ports {
        let port_flows = flows_by_port.get(&port.id).cloned().unwrap_or_default();
        if port_flows.is_empty() {
            continue;
        }

        if let Some(reg_result) = run_panel_regression_for_port(
            port.id,
            &port.name,
            &port_flows,
            &climate_map,
            &war_counts,
            &regime_counts,
            config,
        ) {
            regression_results.push(reg_result);
        }

        let mut granger_for_port = run_granger_tests_for_port(
            port.id,
            &port.name,
            &port_flows,
            &climate_map,
            &war_counts,
            &regime_counts,
            config,
        );
        granger_results.append(&mut granger_for_port);
    }

    let factor_weights = compute_factor_weights(&regression_results);

    PortRiseFallResponse {
        port_flows: flows,
        historical_events,
        regression_results,
        granger_results,
        factor_weights,
    }
}

fn compute_factor_weights(regression_results: &[PanelRegressionResult]) -> Vec<FactorWeight> {
    if regression_results.is_empty() {
        return Vec::new();
    }

    let n_ports = regression_results.len() as f64;
    let mut factor_stats: HashMap<String, (f64, usize)> = HashMap::new();

    for result in regression_results {
        for coef in &result.coefficients {
            if coef.variable == "intercept" {
                continue;
            }
            let entry = factor_stats
                .entry(coef.variable.clone())
                .or_insert((0.0, 0));
            entry.0 += coef.coefficient.abs();
            if coef.is_significant {
                entry.1 += 1;
            }
        }
    }

    let var_zh_map: HashMap<&str, &str> = VAR_NAMES.iter().cloned().collect();

    let mut weights: Vec<FactorWeight> = factor_stats
        .iter()
        .map(|(factor, (sum_coef, sig_count))| {
            let avg_coef = sum_coef / n_ports;
            let sig_rate = *sig_count as f64 / n_ports;
            FactorWeight {
                factor: factor.clone(),
                factor_zh: var_zh_map
                    .get(factor.as_str())
                    .copied()
                    .unwrap_or("")
                    .to_string(),
                avg_coefficient: avg_coef,
                significance_rate: sig_rate,
                importance_rank: 0,
            }
        })
        .collect();

    weights.sort_by(|a, b| {
        b.significance_rate
            .partial_cmp(&a.significance_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                b.avg_coefficient
                    .partial_cmp(&a.avg_coefficient)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    for (i, w) in weights.iter_mut().enumerate() {
        w.importance_rank = (i + 1) as i32;
    }

    weights
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiseFallQuery {
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
    pub port_id: Option<i32>,
    pub region: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
        assert!(
            (a - b).abs() < epsilon,
            "assertion failed: `(left ≈ right)`\n  left: `{}`\n right: `{}`\n  diff: `{}`",
            a,
            b,
            (a - b).abs()
        );
    }

    #[test]
    fn test_ln_gamma_known_values() {
        assert_approx_eq(ln_gamma(1.0), 0.0, 1e-10);
        assert_approx_eq(ln_gamma(2.0), 0.0, 1e-10);
        assert_approx_eq(ln_gamma(3.0), 2.0_f64.ln(), 1e-10);
        assert_approx_eq(ln_gamma(0.5), 0.5 * std::f64::consts::PI.ln(), 1e-10);
    }

    #[test]
    fn test_t_cdf_at_zero() {
        assert_approx_eq(t_cdf(0.0, 10.0), 0.5, 1e-10);
        assert_approx_eq(t_cdf(0.0, 1.0), 0.5, 1e-10);
    }

    #[test]
    fn test_t_cdf_large_t() {
        assert!(t_cdf(100.0, 10.0) > 0.999);
        assert!(t_cdf(-100.0, 10.0) < 0.001);
    }

    #[test]
    fn test_t_cdf_df_one() {
        let p = t_cdf(1.0, 1.0);
        assert!(p > 0.5 && p < 1.0);
        assert_approx_eq(p, 0.75, 0.01);
    }

    #[test]
    fn test_t_cdf_invalid_df() {
        assert!(t_cdf(1.0, 0.0).is_nan());
        assert!(t_cdf(1.0, -1.0).is_nan());
    }

    #[test]
    fn test_f_cdf_at_zero() {
        assert_approx_eq(f_cdf(0.0, 1.0, 1.0), 0.0, 1e-10);
        assert_approx_eq(f_cdf(0.0, 5.0, 10.0), 0.0, 1e-10);
    }

    #[test]
    fn test_f_cdf_large_f() {
        assert!(f_cdf(1000.0, 5.0, 5.0) > 0.99);
    }

    #[test]
    fn test_f_cdf_invalid_params() {
        assert_approx_eq(f_cdf(1.0, 0.0, 5.0), 0.0, 1e-15);
        assert_approx_eq(f_cdf(1.0, 5.0, 0.0), 0.0, 1e-15);
        assert_approx_eq(f_cdf(-1.0, 5.0, 5.0), 0.0, 1e-15);
    }

    #[test]
    fn test_t_pvalue_basic() {
        assert_approx_eq(t_pvalue(0.0, 10.0), 1.0, 1e-10);
        assert!(t_pvalue(10.0, 10.0) < 0.001);
        assert_approx_eq(t_pvalue(2.0, 10.0), t_pvalue(-2.0, 10.0), 1e-10);
    }

    #[test]
    fn test_f_pvalue_basic() {
        assert!(f_pvalue(100.0, 5.0, 5.0) < 0.001);
        assert_approx_eq(f_pvalue(0.0, 5.0, 5.0), 1.0, 1e-10);
    }

    #[test]
    fn test_mat_transpose_2x3() {
        let a = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let at = mat_transpose(&a);
        assert_eq!(at.len(), 3);
        assert_eq!(at[0].len(), 2);
        assert_approx_eq(at[0][0], 1.0, 1e-15);
        assert_approx_eq(at[0][1], 4.0, 1e-15);
        assert_approx_eq(at[1][0], 2.0, 1e-15);
        assert_approx_eq(at[2][1], 6.0, 1e-15);
    }

    #[test]
    fn test_mat_transpose_empty() {
        let a: Vec<Vec<f64>> = Vec::new();
        assert!(mat_transpose(&a).is_empty());
    }

    #[test]
    fn test_mat_multiply_known_result() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let c = mat_multiply(&a, &b);
        assert_approx_eq(c[0][0], 19.0, 1e-15);
        assert_approx_eq(c[0][1], 22.0, 1e-15);
        assert_approx_eq(c[1][0], 43.0, 1e-15);
        assert_approx_eq(c[1][1], 50.0, 1e-15);
    }

    #[test]
    fn test_mat_multiply_different_sizes() {
        let a = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let b = vec![vec![7.0, 8.0], vec![9.0, 10.0], vec![11.0, 12.0]];
        let c = mat_multiply(&a, &b);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].len(), 2);
        assert_approx_eq(c[0][0], 58.0, 1e-15);
        assert_approx_eq(c[1][1], 154.0, 1e-15);
    }

    #[test]
    fn test_mat_inverse_identity() {
        let i = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let inv = mat_inverse(&i).unwrap();
        for row in 0..3 {
            for col in 0..3 {
                if row == col {
                    assert_approx_eq(inv[row][col], 1.0, 1e-10);
                } else {
                    assert_approx_eq(inv[row][col], 0.0, 1e-10);
                }
            }
        }
    }

    #[test]
    fn test_mat_inverse_diagonal_2x2() {
        let a = vec![vec![2.0, 0.0], vec![0.0, 3.0]];
        let inv = mat_inverse(&a).unwrap();
        assert_approx_eq(inv[0][0], 0.5, 1e-10);
        assert_approx_eq(inv[0][1], 0.0, 1e-10);
        assert_approx_eq(inv[1][0], 0.0, 1e-10);
        assert_approx_eq(inv[1][1], 1.0 / 3.0, 1e-10);
    }

    #[test]
    fn test_mat_inverse_singular_returns_none() {
        let a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        assert!(mat_inverse(&a).is_none());
    }

    #[test]
    fn test_mat_inverse_non_square_returns_none() {
        let a = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        assert!(mat_inverse(&a).is_none());
    }

    #[test]
    fn test_mat_vec_multiply_known() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        let v = vec![7.0, 8.0];
        let result = mat_vec_multiply(&a, &v);
        assert_eq!(result.len(), 3);
        assert_approx_eq(result[0], 23.0, 1e-15);
        assert_approx_eq(result[1], 53.0, 1e-15);
        assert_approx_eq(result[2], 83.0, 1e-15);
    }

    #[test]
    fn test_mat_vec_multiply_empty() {
        let a: Vec<Vec<f64>> = Vec::new();
        let v: Vec<f64> = Vec::new();
        assert!(mat_vec_multiply(&a, &v).is_empty());
    }

    #[test]
    fn test_panel_regression_simple_linear() {
        let x: Vec<Vec<f64>> = (0..10).map(|i| vec![1.0, i as f64]).collect();
        let y: Vec<f64> = (0..10).map(|i| 2.0 * i as f64 + 1.0).collect();
        let model = PanelRegression::fit(&x, &y).unwrap();
        let coefs = model.coefficients();
        assert_approx_eq(coefs[0], 1.0, 1e-10);
        assert_approx_eq(coefs[1], 2.0, 1e-10);
    }

    #[test]
    fn test_panel_regression_perfect_fit_r_squared() {
        let x: Vec<Vec<f64>> = (0..10).map(|i| vec![1.0, i as f64]).collect();
        let y: Vec<f64> = (0..10).map(|i| 3.0 * i as f64 + 5.0).collect();
        let model = PanelRegression::fit(&x, &y).unwrap();
        assert_approx_eq(model.r_squared(), 1.0, 1e-10);
    }

    #[test]
    fn test_panel_regression_empty_data() {
        let x: Vec<Vec<f64>> = Vec::new();
        let y: Vec<f64> = Vec::new();
        assert!(PanelRegression::fit(&x, &y).is_none());
    }

    #[test]
    fn test_panel_regression_obs_less_than_vars() {
        let x = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let y = vec![5.0, 6.0];
        assert!(PanelRegression::fit(&x, &y).is_none());
    }

    #[test]
    fn test_panel_regression_constant_y_r_squared_zero() {
        let x: Vec<Vec<f64>> = (0..5).map(|i| vec![1.0, i as f64]).collect();
        let y = vec![10.0; 5];
        let model = PanelRegression::fit(&x, &y).unwrap();
        assert_approx_eq(model.r_squared(), 0.0, 1e-10);
    }

    #[test]
    fn test_panel_regression_positive_coefficient_sign() {
        let x: Vec<Vec<f64>> = (0..20).map(|i| vec![1.0, i as f64]).collect();
        let y: Vec<f64> = (0..20)
            .map(|i| i as f64 * 0.5 + 2.0 + (i as f64 * 0.1).sin())
            .collect();
        let model = PanelRegression::fit(&x, &y).unwrap();
        assert!(model.coefficients()[1] > 0.0);
    }

    #[test]
    fn test_granger_insufficient_data_returns_none() {
        let y = vec![1.0, 2.0, 3.0];
        let x = vec![4.0, 5.0, 6.0];
        assert!(GrangerCausalityTest::test(&y, &x, 2, 0.05).is_none());
    }

    #[test]
    fn test_granger_max_lags_zero_returns_none() {
        let y: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let x: Vec<f64> = (0..20).map(|i| i as f64 * 0.5).collect();
        assert!(GrangerCausalityTest::test(&y, &x, 0, 0.05).is_none());
    }

    #[test]
    fn test_granger_independent_not_significant() {
        use std::f64::consts::PI;
        let y: Vec<f64> = (0..60).map(|i| (i as f64 * 0.1).sin()).collect();
        let x: Vec<f64> = (0..60)
            .map(|i| (i as f64 * 0.15 + PI / 2.0).cos() * 100.0)
            .collect();
        let test = GrangerCausalityTest::test(&y, &x, 3, 0.05);
        if let Some(t) = test {
            assert!(!t.is_significant());
        }
    }

    #[test]
    fn test_granger_lag_order_selection() {
        let mut x = vec![0.0; 60];
        let mut y = vec![0.0; 60];
        for i in 0..60 {
            x[i] = (i as f64 * 0.3).sin() + (i as f64 * 0.7).cos();
        }
        for i in 2..60 {
            y[i] = 0.8 * x[i - 2] + 0.1 * y[i - 1];
        }
        let test = GrangerCausalityTest::test(&y, &x, 5, 0.05).unwrap();
        assert!(test.lag_order() >= 1 && test.lag_order() <= 5);
    }

    #[test]
    fn test_boundary_empty_inputs() {
        assert!(mat_transpose(&[]).is_empty());
        assert!(mat_multiply(&[], &[]).is_empty());
        assert!(mat_vec_multiply(&[], &[]).is_empty());
        assert!(mat_inverse(&[]).is_none());
        assert!(PanelRegression::fit(&[], &[]).is_none());
    }

    #[test]
    fn test_boundary_single_element() {
        let a = vec![vec![5.0]];
        let inv = mat_inverse(&a).unwrap();
        assert_approx_eq(inv[0][0], 0.2, 1e-10);

        let v = vec![3.0];
        let result = mat_vec_multiply(&a, &v);
        assert_approx_eq(result[0], 15.0, 1e-15);
    }

    #[test]
    fn test_nan_handling() {
        assert!(t_cdf(1.0, 0.0).is_nan());
        assert!(t_cdf(1.0, -1.0).is_nan());
        assert!(regularized_incomplete_beta(1.0, 1.0, -0.1).is_nan());
        assert!(regularized_incomplete_beta(1.0, 1.0, 1.1).is_nan());
    }

    #[test]
    fn test_extreme_values() {
        assert!(ln_gamma(100.0) > 0.0);
        assert!(t_cdf(1e6, 10.0) > 0.999999);
        assert!(t_cdf(-1e6, 10.0) < 0.000001);
        assert!(f_cdf(1e6, 5.0, 5.0) > 0.999);
    }
}
