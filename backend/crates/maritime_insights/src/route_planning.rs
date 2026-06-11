use maritime_common::config::RoutePlanningConfig;
use maritime_common::models::*;
use sqlx::PgPool;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

const EARTH_RADIUS_KM: f64 = 6371.0;
const NAUTICAL_MILE_PER_KM: f64 = 0.539957;
const HOURS_PER_DAY: f64 = 24.0;
const MIN_SPEED_KNOTS: f64 = 0.5;

fn deg_to_rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

fn rad_to_deg(rad: f64) -> f64 {
    rad * 180.0 / std::f64::consts::PI
}

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat = deg_to_rad(lat2 - lat1);
    let dlon = deg_to_rad(lon2 - lon1);
    let a = (dlat / 2.0).sin().powi(2)
        + deg_to_rad(lat1).cos() * deg_to_rad(lat2).cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_KM * c
}

fn haversine_distance_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    haversine_distance(lat1, lon1, lat2, lon2) * NAUTICAL_MILE_PER_KM
}

fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let lat1_r = deg_to_rad(lat1);
    let lat2_r = deg_to_rad(lat2);
    let dlon_r = deg_to_rad(lon2 - lon1);
    let y = dlon_r.sin() * lat2_r.cos();
    let x = lat1_r.cos() * lat2_r.sin() - lat1_r.sin() * lat2_r.cos() * dlon_r.cos();
    let brng = y.atan2(x);
    (rad_to_deg(brng) + 360.0) % 360.0
}

fn direction_to_vector(deg: f64) -> (f64, f64) {
    let rad = deg_to_rad(deg);
    (rad.sin(), rad.cos())
}

#[derive(Debug, Clone)]
struct GridCell {
    lat: f64,
    lon: f64,
    current_speed_knots: f64,
    current_direction_deg: f64,
    wind_speed_knots: f64,
    wind_direction_deg: f64,
    storm_risk: f64,
    base_cost: f64,
}

#[derive(Debug, Clone)]
struct RouteGrid {
    cells: Vec<Vec<GridCell>>,
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    lat_steps: usize,
    lon_steps: usize,
    grid_res_km: f64,
}

impl RouteGrid {
    fn new(lat_min: f64, lat_max: f64, lon_min: f64, lon_max: f64, grid_res_km: f64) -> Self {
        let lat_range = lat_max - lat_min;
        let lon_range = lon_max - lon_min;
        let avg_lat = (lat_min + lat_max) / 2.0;
        let km_per_deg_lat = 111.0;
        let km_per_deg_lon = 111.0 * deg_to_rad(avg_lat).cos();

        let lat_steps = ((lat_range * km_per_deg_lat) / grid_res_km).ceil().max(2.0) as usize;
        let lon_steps = ((lon_range * km_per_deg_lon.abs()) / grid_res_km)
            .ceil()
            .max(2.0) as usize;

        let mut cells = Vec::with_capacity(lat_steps);
        for i in 0..lat_steps {
            let mut row = Vec::with_capacity(lon_steps);
            let lat_frac = i as f64 / (lat_steps - 1) as f64;
            let lat = lat_min + lat_frac * lat_range;
            for j in 0..lon_steps {
                let lon_frac = j as f64 / (lon_steps - 1) as f64;
                let lon = lon_min + lon_frac * lon_range;
                row.push(GridCell {
                    lat,
                    lon,
                    current_speed_knots: 0.0,
                    current_direction_deg: 0.0,
                    wind_speed_knots: 0.0,
                    wind_direction_deg: 0.0,
                    storm_risk: 0.0,
                    base_cost: 1.0,
                });
            }
            cells.push(row);
        }

        RouteGrid {
            cells,
            lat_min,
            lat_max,
            lon_min,
            lon_max,
            lat_steps,
            lon_steps,
            grid_res_km,
        }
    }

    fn cell_at(&self, i: usize, j: usize) -> &GridCell {
        &self.cells[i][j]
    }

    fn cell_at_mut(&mut self, i: usize, j: usize) -> &mut GridCell {
        &mut self.cells[i][j]
    }

    fn lat_lon_to_idx(&self, lat: f64, lon: f64) -> (usize, usize) {
        let lat_frac = ((lat - self.lat_min) / (self.lat_max - self.lat_min)).clamp(0.0, 1.0);
        let lon_frac = ((lon - self.lon_min) / (self.lon_max - self.lon_min)).clamp(0.0, 1.0);
        let i = (lat_frac * (self.lat_steps - 1) as f64).round() as usize;
        let j = (lon_frac * (self.lon_steps - 1) as f64).round() as usize;
        (i.min(self.lat_steps - 1), j.min(self.lon_steps - 1))
    }

    fn idx_to_lat_lon(&self, i: usize, j: usize) -> (f64, f64) {
        let cell = &self.cells[i][j];
        (cell.lat, cell.lon)
    }

    fn cell_distance_km(&self, i1: usize, j1: usize, i2: usize, j2: usize) -> f64 {
        let (lat1, lon1) = self.idx_to_lat_lon(i1, j1);
        let (lat2, lon2) = self.idx_to_lat_lon(i2, j2);
        haversine_distance(lat1, lon1, lat2, lon2)
    }

    fn cell_distance_nm(&self, i1: usize, j1: usize, i2: usize, j2: usize) -> f64 {
        self.cell_distance_km(i1, j1, i2, j2) * NAUTICAL_MILE_PER_KM
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RouteNode {
    f: f64,
    g: f64,
    h: f64,
    i: usize,
    j: usize,
}

impl Eq for RouteNode {}

impl Ord for RouteNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for RouteNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct AStarOpenSet {
    heap: BinaryHeap<RouteNode>,
    best_g: HashMap<(usize, usize), f64>,
}

impl AStarOpenSet {
    fn new() -> Self {
        AStarOpenSet {
            heap: BinaryHeap::new(),
            best_g: HashMap::new(),
        }
    }

    fn push(&mut self, node: RouteNode) {
        let key = (node.i, node.j);
        if let Some(&best) = self.best_g.get(&key) {
            if node.g >= best {
                return;
            }
        }
        self.best_g.insert(key, node.g);
        self.heap.push(node);
    }

    fn pop(&mut self) -> Option<RouteNode> {
        loop {
            let node = self.heap.pop()?;
            let key = (node.i, node.j);
            if let Some(&best) = self.best_g.get(&key) {
                if node.g <= best {
                    return Some(node);
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    fn get_best_g(&self, i: usize, j: usize) -> Option<f64> {
        self.best_g.get(&(i, j)).copied()
    }
}

struct CurrentSample {
    lat: f64,
    lon: f64,
    speed_knots: f64,
    direction_deg: f64,
}

struct WindSample {
    lat: f64,
    lon: f64,
    speed_knots: f64,
    direction_deg: f64,
}

fn bilinear_interpolate(points: &[(f64, f64, f64)], x: f64, y: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    if points.len() == 1 {
        return points[0].2;
    }

    let mut total_weight = 0.0;
    let mut weighted_sum = 0.0;

    for &(px, py, val) in points {
        let dx = px - x;
        let dy = py - y;
        let dist_sq = dx * dx + dy * dy;
        let dist = dist_sq.sqrt().max(0.001);
        let weight = 1.0 / dist;
        weighted_sum += val * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        weighted_sum / total_weight
    } else {
        points[0].2
    }
}

fn speed_projection_on_heading(speed_knots: f64, direction_deg: f64, heading_deg: f64) -> f64 {
    let angle_diff = deg_to_rad(direction_deg - heading_deg);
    speed_knots * angle_diff.cos()
}

fn compute_move_cost(
    grid: &RouteGrid,
    from_i: usize,
    from_j: usize,
    to_i: usize,
    to_j: usize,
    ship_base_speed_knots: f64,
    config: &RoutePlanningConfig,
) -> f64 {
    let distance_nm = grid.cell_distance_nm(from_i, from_j, to_i, to_j);
    let (from_lat, from_lon) = grid.idx_to_lat_lon(from_i, from_j);
    let (to_lat, to_lon) = grid.idx_to_lat_lon(to_i, to_j);
    let heading = bearing(from_lat, from_lon, to_lat, to_lon);

    let from_cell = grid.cell_at(from_i, from_j);
    let to_cell = grid.cell_at(to_i, to_j);

    let avg_current_speed = (from_cell.current_speed_knots + to_cell.current_speed_knots) / 2.0;
    let avg_current_dir = (from_cell.current_direction_deg + to_cell.current_direction_deg) / 2.0;
    let avg_wind_speed = (from_cell.wind_speed_knots + to_cell.wind_speed_knots) / 2.0;
    let avg_wind_dir = (from_cell.wind_direction_deg + to_cell.wind_direction_deg) / 2.0;
    let avg_storm_risk = (from_cell.storm_risk + to_cell.storm_risk) / 2.0;

    if avg_storm_risk >= config.storm_risk_hard_threshold {
        return f64::INFINITY;
    }

    let current_assist = speed_projection_on_heading(avg_current_speed, avg_current_dir, heading)
        * config.current_weight;
    let wind_assist =
        speed_projection_on_heading(avg_wind_speed, avg_wind_dir, heading) * config.wind_weight;

    let risk_penalty = if avg_storm_risk >= config.storm_risk_soft_threshold {
        let normalized_risk = (avg_storm_risk - config.storm_risk_soft_threshold)
            / (config.storm_risk_hard_threshold - config.storm_risk_soft_threshold);
        let exponential_penalty = (normalized_risk * 5.0).exp() - 1.0;
        exponential_penalty * config.storm_risk_weight * ship_base_speed_knots
    } else {
        avg_storm_risk * config.storm_risk_weight * ship_base_speed_knots
    };

    let effective_speed = ship_base_speed_knots + current_assist + wind_assist - risk_penalty;
    let effective_speed = effective_speed.max(MIN_SPEED_KNOTS);

    let time_hours = distance_nm / effective_speed;
    time_hours / HOURS_PER_DAY
}

fn heuristic(
    grid: &RouteGrid,
    i: usize,
    j: usize,
    goal_i: usize,
    goal_j: usize,
    ship_max_speed_knots: f64,
) -> f64 {
    let distance_nm = grid.cell_distance_nm(i, j, goal_i, goal_j);
    let time_hours = distance_nm / ship_max_speed_knots;
    time_hours / HOURS_PER_DAY
}

const NEIGHBOR_OFFSETS: [(isize, isize); 8] = [
    (-1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
];

fn a_star_search(
    grid: &RouteGrid,
    start_i: usize,
    start_j: usize,
    goal_i: usize,
    goal_j: usize,
    ship_base_speed_knots: f64,
    ship_max_speed_knots: f64,
    config: &RoutePlanningConfig,
) -> Option<(Vec<(usize, usize)>, f64)> {
    let mut open_set = AStarOpenSet::new();
    let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut closed_set: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();

    let start_h = heuristic(grid, start_i, start_j, goal_i, goal_j, ship_max_speed_knots);
    open_set.push(RouteNode {
        f: start_h,
        g: 0.0,
        h: start_h,
        i: start_i,
        j: start_j,
    });

    let mut iterations = 0;
    let max_iter = config.max_iterations.max(1000);

    while !open_set.is_empty() && iterations < max_iter {
        iterations += 1;

        let current = open_set.pop().unwrap();

        if current.i == goal_i && current.j == goal_j {
            let mut path = Vec::new();
            let mut ci = current.i;
            let mut cj = current.j;
            path.push((ci, cj));
            while let Some(&prev) = came_from.get(&(ci, cj)) {
                ci = prev.0;
                cj = prev.1;
                path.push((ci, cj));
            }
            path.reverse();
            return Some((path, current.g));
        }

        closed_set.insert((current.i, current.j));

        for &(di, dj) in &NEIGHBOR_OFFSETS {
            let ni = current.i as isize + di;
            let nj = current.j as isize + dj;

            if ni < 0 || nj < 0 {
                continue;
            }
            let ni = ni as usize;
            let nj = nj as usize;

            if ni >= grid.lat_steps || nj >= grid.lon_steps {
                continue;
            }

            if closed_set.contains(&(ni, nj)) {
                continue;
            }

            let move_cost = compute_move_cost(
                grid,
                current.i,
                current.j,
                ni,
                nj,
                ship_base_speed_knots,
                config,
            );
            let tentative_g = current.g + move_cost;

            if let Some(best_g) = open_set.get_best_g(ni, nj) {
                if tentative_g >= best_g {
                    continue;
                }
            }

            came_from.insert((ni, nj), (current.i, current.j));
            let h = heuristic(grid, ni, nj, goal_i, goal_j, ship_max_speed_knots);
            open_set.push(RouteNode {
                f: tentative_g + h,
                g: tentative_g,
                h,
                i: ni,
                j: nj,
            });
        }
    }

    None
}

fn ship_base_speed(ship_type: &str) -> f64 {
    match ship_type {
        "trireme" => 6.0,
        "galley" => 5.0,
        "longship" => 5.5,
        "dhow" => 4.5,
        "merchant_round_ship" => 4.0,
        "junk" => 5.0,
        "carrack" => 4.5,
        "treasure_ship" => 4.0,
        _ => 5.0,
    }
}

fn ship_max_speed(ship_type: &str) -> f64 {
    ship_base_speed(ship_type) * 1.5
}

async fn load_ocean_currents(
    pool: &PgPool,
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    season: &str,
) -> Vec<CurrentSample> {
    let query = r#"
        SELECT 
            ST_Y(ST_StartPoint(geom)) as start_lat,
            ST_X(ST_StartPoint(geom)) as start_lon,
            ST_Y(ST_EndPoint(geom)) as end_lat,
            ST_X(ST_EndPoint(geom)) as end_lon,
            speed_knots,
            direction_deg
        FROM ocean_currents
        WHERE season = $1
          AND geom && ST_MakeEnvelope($2, $3, $4, $5, 4326)
    "#;

    let result: Result<Vec<(f64, f64, f64, f64, Option<f64>, Option<f64>)>, _> =
        sqlx::query_as(query)
            .bind(season)
            .bind(lon_min)
            .bind(lat_min)
            .bind(lon_max)
            .bind(lat_max)
            .fetch_all(pool)
            .await;

    let rows = match result {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut samples = Vec::new();
    for (start_lat, start_lon, end_lat, end_lon, speed_opt, dir_opt) in rows {
        let speed = speed_opt.unwrap_or(1.0);
        let direction = dir_opt.unwrap_or(0.0);
        let mid_lat = (start_lat + end_lat) / 2.0;
        let mid_lon = (start_lon + end_lon) / 2.0;
        samples.push(CurrentSample {
            lat: mid_lat,
            lon: mid_lon,
            speed_knots: speed,
            direction_deg: direction,
        });
        samples.push(CurrentSample {
            lat: start_lat,
            lon: start_lon,
            speed_knots: speed,
            direction_deg: direction,
        });
        samples.push(CurrentSample {
            lat: end_lat,
            lon: end_lon,
            speed_knots: speed,
            direction_deg: direction,
        });
    }

    samples
}

async fn load_wind_fields(
    pool: &PgPool,
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    season: &str,
) -> Vec<WindSample> {
    let query = r#"
        SELECT 
            ST_Y(ST_Centroid(geom)) as center_lat,
            ST_X(ST_Centroid(geom)) as center_lon,
            avg_speed_knots,
            avg_direction_deg
        FROM wind_fields
        WHERE season = $1
          AND geom && ST_MakeEnvelope($2, $3, $4, $5, 4326)
    "#;

    let result: Result<Vec<(f64, f64, Option<f64>, Option<f64>)>, _> = sqlx::query_as(query)
        .bind(season)
        .bind(lon_min)
        .bind(lat_min)
        .bind(lon_max)
        .bind(lat_max)
        .fetch_all(pool)
        .await;

    let rows = match result {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut samples = Vec::new();
    for (center_lat, center_lon, speed_opt, dir_opt) in rows {
        let speed = speed_opt.unwrap_or(10.0);
        let direction = dir_opt.unwrap_or(0.0);
        samples.push(WindSample {
            lat: center_lat,
            lon: center_lon,
            speed_knots: speed,
            direction_deg: direction,
        });
    }

    samples
}

fn populate_grid_with_env_data(
    grid: &mut RouteGrid,
    current_samples: &[CurrentSample],
    wind_samples: &[WindSample],
) {
    let current_speed_points: Vec<(f64, f64, f64)> = current_samples
        .iter()
        .map(|s| (s.lat, s.lon, s.speed_knots))
        .collect();
    let current_dir_points: Vec<(f64, f64, f64)> = current_samples
        .iter()
        .map(|s| (s.lat, s.lon, s.direction_deg))
        .collect();
    let wind_speed_points: Vec<(f64, f64, f64)> = wind_samples
        .iter()
        .map(|s| (s.lat, s.lon, s.speed_knots))
        .collect();
    let wind_dir_points: Vec<(f64, f64, f64)> = wind_samples
        .iter()
        .map(|s| (s.lat, s.lon, s.direction_deg))
        .collect();

    for i in 0..grid.lat_steps {
        for j in 0..grid.lon_steps {
            let cell = grid.cell_at_mut(i, j);
            let lat = cell.lat;
            let lon = cell.lon;

            cell.current_speed_knots = bilinear_interpolate(&current_speed_points, lat, lon);
            cell.current_direction_deg = bilinear_interpolate(&current_dir_points, lat, lon);
            cell.wind_speed_knots = bilinear_interpolate(&wind_speed_points, lat, lon);
            cell.wind_direction_deg = bilinear_interpolate(&wind_dir_points, lat, lon);
        }
    }
}

async fn get_storm_risk_for_region(
    pool: &PgPool,
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    season: &str,
) -> f64 {
    let mid_lat = (lat_min + lat_max) / 2.0;
    let mid_lon = (lon_min + lon_max) / 2.0;

    let query = r#"
        SELECT storm_frequency
        FROM climate_periods
        WHERE period_start <= 1500
          AND period_end >= -500
        ORDER BY id
        LIMIT 1
    "#;

    let result: Result<Option<f64>, _> = sqlx::query_scalar(query).fetch_optional(pool).await;

    match result {
        Ok(Some(freq)) => freq,
        _ => {
            let voyage_query = r#"
                SELECT 
                    COUNT(*) as total,
                    SUM(CASE WHEN encountered_storm THEN 1 ELSE 0 END) as storm_count
                FROM voyage_records
                WHERE season = $1
                  AND departure_port_id IN (
                    SELECT id FROM ports 
                    WHERE ST_Y(geom) BETWEEN $2 AND $3 
                      AND ST_X(geom) BETWEEN $4 AND $5
                  )
            "#;

            let result: Result<Option<(i64, i64)>, _> = sqlx::query_as(voyage_query)
                .bind(season)
                .bind(lat_min)
                .bind(lat_max)
                .bind(lon_min)
                .bind(lon_max)
                .fetch_optional(pool)
                .await;

            match result {
                Ok(Some((total, storm_count))) if total > 0 => storm_count as f64 / total as f64,
                _ => 0.15,
            }
        }
    }
}

fn populate_grid_with_storm_risk(grid: &mut RouteGrid, base_risk: f64) {
    for i in 0..grid.lat_steps {
        for j in 0..grid.lon_steps {
            grid.cell_at_mut(i, j).storm_risk = base_risk;
        }
    }
}

fn path_to_route_points(grid: &RouteGrid, path: &[(usize, usize)]) -> Vec<Vec<f64>> {
    path.iter()
        .map(|&(i, j)| {
            let (lat, lon) = grid.idx_to_lat_lon(i, j);
            vec![lon, lat]
        })
        .collect()
}

fn total_distance_nm(route_points: &[Vec<f64>]) -> f64 {
    if route_points.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    for i in 0..route_points.len() - 1 {
        let p1 = &route_points[i];
        let p2 = &route_points[i + 1];
        total += haversine_distance_nm(p1[1], p1[0], p2[1], p2[0]);
    }
    total
}

async fn load_historical_voyages(
    pool: &PgPool,
    departure_port_id: i32,
    arrival_port_id: i32,
    season: &str,
) -> Vec<VoyageRecord> {
    let query = r#"
        SELECT id, departure_port_id, arrival_port_id, voyage_year, season,
               ship_type, cargo_type, encountered_storm, route_points, created_at
        FROM voyage_records
        WHERE departure_port_id = $1
          AND arrival_port_id = $2
          AND season = $3
        LIMIT 50
    "#;

    sqlx::query_as::<_, VoyageRecord>(query)
        .bind(departure_port_id)
        .bind(arrival_port_id)
        .bind(season)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
}

fn extract_route_points_from_voyage(voyage: &VoyageRecord) -> Vec<Vec<f64>> {
    let mut points = Vec::new();
    if let Some(ref route_json) = voyage.route_points {
        if let Some(arr) = route_json.as_array() {
            for pt in arr {
                if let Some(coord_arr) = pt.as_array() {
                    let lon = coord_arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let lat = coord_arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    points.push(vec![lon, lat]);
                }
            }
        }
    }
    points
}

fn point_to_line_distance(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 0.000001 {
        return haversine_distance_nm(py, px, ay, ax);
    }

    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;

    haversine_distance_nm(py, px, proj_y, proj_x)
}

fn average_distance_deviation(route_a: &[Vec<f64>], route_b: &[Vec<f64>]) -> f64 {
    if route_a.len() < 2 || route_b.len() < 2 {
        return 0.0;
    }

    let total_dist_a = total_distance_nm(route_a);
    if total_dist_a < 0.001 {
        return 0.0;
    }

    let mut total_deviation = 0.0;
    let mut count = 0;

    for point in route_a {
        let px = point[0];
        let py = point[1];

        let mut min_dist = f64::INFINITY;
        for i in 0..route_b.len() - 1 {
            let a = &route_b[i];
            let b = &route_b[i + 1];
            let dist = point_to_line_distance(px, py, a[0], a[1], b[0], b[1]);
            if dist < min_dist {
                min_dist = dist;
            }
        }

        if min_dist.is_finite() {
            total_deviation += min_dist;
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }

    let avg_deviation_nm = total_deviation / count as f64;
    (avg_deviation_nm / total_dist_a) * 100.0 * 10.0
}

fn frechet_distance_approx(route_a: &[Vec<f64>], route_b: &[Vec<f64>]) -> f64 {
    if route_a.is_empty() || route_b.is_empty() {
        return 0.0;
    }

    let n = route_a.len();
    let m = route_b.len();

    let mut dist_matrix = vec![vec![0.0; m]; n];
    for i in 0..n {
        for j in 0..m {
            dist_matrix[i][j] =
                haversine_distance_nm(route_a[i][1], route_a[i][0], route_b[j][1], route_b[j][0]);
        }
    }

    let mut ca = vec![vec![-1.0; m]; n];

    fn c(dist_matrix: &[Vec<f64>], ca: &mut [Vec<f64>], i: usize, j: usize) -> f64 {
        if ca[i][j] > -0.5 {
            return ca[i][j];
        }

        let result = if i == 0 && j == 0 {
            dist_matrix[0][0]
        } else if i > 0 && j == 0 {
            c(dist_matrix, ca, i - 1, 0).max(dist_matrix[i][0])
        } else if i == 0 && j > 0 {
            c(dist_matrix, ca, 0, j - 1).max(dist_matrix[0][j])
        } else if i > 0 && j > 0 {
            let prev = c(dist_matrix, ca, i - 1, j - 1)
                .min(c(dist_matrix, ca, i - 1, j))
                .min(c(dist_matrix, ca, i, j - 1));
            prev.max(dist_matrix[i][j])
        } else {
            f64::INFINITY
        };

        ca[i][j] = result;
        result
    }

    c(&dist_matrix, &mut ca, n - 1, m - 1)
}

fn path_similarity_score(route_a: &[Vec<f64>], route_b: &[Vec<f64>]) -> f64 {
    let total_dist = total_distance_nm(route_a).max(total_distance_nm(route_b));
    if total_dist < 0.001 {
        return 0.0;
    }

    let frechet = frechet_distance_approx(route_a, route_b);
    let similarity = (1.0 - (frechet / total_dist).min(1.0)).max(0.0);
    similarity
}

fn correlation_coefficient(xs: &[f64], ys: &[f64]) -> f64 {
    if xs.len() != ys.len() || xs.len() < 2 {
        return 0.0;
    }

    let n = xs.len() as f64;
    let sum_x: f64 = xs.iter().sum();
    let sum_y: f64 = ys.iter().sum();
    let sum_xy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
    let sum_x2: f64 = xs.iter().map(|x| x * x).sum();
    let sum_y2: f64 = ys.iter().map(|y| y * y).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

    if denominator.abs() < 0.000001 {
        return 0.0;
    }

    numerator / denominator
}

fn compute_historical_correlation(
    optimized_route: &[Vec<f64>],
    historical_routes: &[Vec<Vec<f64>>],
) -> f64 {
    if historical_routes.is_empty() {
        return 0.0;
    }

    let opt_dist = total_distance_nm(optimized_route);
    if opt_dist < 0.001 {
        return 0.0;
    }

    let mut deviations = Vec::new();
    let mut route_lengths = Vec::new();

    for hist_route in historical_routes {
        let dev = average_distance_deviation(optimized_route, hist_route);
        let len = total_distance_nm(hist_route);
        deviations.push(dev);
        route_lengths.push(len);
    }

    -correlation_coefficient(&deviations, &route_lengths).abs() * 0.5 + 0.5
}

async fn get_port_coords(pool: &PgPool, port_id: i32) -> Option<(f64, f64, String)> {
    let query = r#"
        SELECT name, ST_Y(geom) as lat, ST_X(geom) as lon
        FROM ports WHERE id = $1
    "#;

    let result: Result<Option<(String, f64, f64)>, _> = sqlx::query_as(query)
        .bind(port_id)
        .fetch_optional(pool)
        .await;

    match result {
        Ok(Some((name, lat, lon))) => Some((lat, lon, name)),
        _ => None,
    }
}

pub async fn plan_optimal_route(
    pool: &PgPool,
    config: &RoutePlanningConfig,
    departure_port_id: i32,
    arrival_port_id: i32,
    season: &str,
    ship_type: &str,
) -> Option<RoutePlanningResult> {
    let (dep_lat, dep_lon, dep_name) = get_port_coords(pool, departure_port_id).await?;
    let (arr_lat, arr_lon, arr_name) = get_port_coords(pool, arrival_port_id).await?;

    let lat_min = dep_lat.min(arr_lat) - 2.0;
    let lat_max = dep_lat.max(arr_lat) + 2.0;
    let lon_min = dep_lon.min(arr_lon) - 2.0;
    let lon_max = dep_lon.max(arr_lon) + 2.0;

    let mut grid = RouteGrid::new(
        lat_min,
        lat_max,
        lon_min,
        lon_max,
        config.grid_resolution_km,
    );

    let current_samples =
        load_ocean_currents(pool, lat_min, lat_max, lon_min, lon_max, season).await;

    let wind_samples = load_wind_fields(pool, lat_min, lat_max, lon_min, lon_max, season).await;

    populate_grid_with_env_data(&mut grid, &current_samples, &wind_samples);

    let base_storm_risk =
        get_storm_risk_for_region(pool, lat_min, lat_max, lon_min, lon_max, season).await;
    populate_grid_with_storm_risk(&mut grid, base_storm_risk);

    let (start_i, start_j) = grid.lat_lon_to_idx(dep_lat, dep_lon);
    let (goal_i, goal_j) = grid.lat_lon_to_idx(arr_lat, arr_lon);

    let base_speed = ship_base_speed(ship_type);
    let max_speed = ship_max_speed(ship_type);

    let (path, estimated_days) = a_star_search(
        &grid, start_i, start_j, goal_i, goal_j, base_speed, max_speed, config,
    )?;

    let route_points = path_to_route_points(&grid, &path);
    let distance_nm = total_distance_nm(&route_points);
    let avg_speed = if estimated_days > 0.0 {
        distance_nm / (estimated_days * HOURS_PER_DAY)
    } else {
        base_speed
    };

    let historical_voyages =
        load_historical_voyages(pool, departure_port_id, arrival_port_id, season).await;

    let historical_routes: Vec<Vec<Vec<f64>>> = historical_voyages
        .iter()
        .map(|v| extract_route_points_from_voyage(v))
        .filter(|r| r.len() >= 2)
        .collect();

    let (historical_deviation_pct, historical_correlation) = if !historical_routes.is_empty() {
        let mut total_dev = 0.0;
        for hist in &historical_routes {
            total_dev += average_distance_deviation(&route_points, hist);
        }
        let avg_dev = total_dev / historical_routes.len() as f64;
        let correlation = compute_historical_correlation(&route_points, &historical_routes);
        (avg_dev, correlation)
    } else {
        (0.0, 0.0)
    };

    let storm_risk = if !path.is_empty() {
        let mut total_risk = 0.0;
        for &(i, j) in &path {
            total_risk += grid.cell_at(i, j).storm_risk;
        }
        total_risk / path.len() as f64
    } else {
        0.0
    };

    Some(RoutePlanningResult {
        departure_port_id,
        arrival_port_id,
        departure_port_name: dep_name,
        arrival_port_name: arr_name,
        season: season.to_string(),
        ship_type: ship_type.to_string(),
        method: "a_star".to_string(),
        route_points,
        distance_nautical_miles: distance_nm,
        estimated_days,
        avg_speed_knots: avg_speed,
        storm_risk,
        historical_deviation_pct,
        historical_correlation,
    })
}

pub async fn get_historical_route(
    pool: &PgPool,
    departure_port_id: i32,
    arrival_port_id: i32,
    season: &str,
) -> Option<RoutePlanningResult> {
    let voyages = load_historical_voyages(pool, departure_port_id, arrival_port_id, season).await;

    if voyages.is_empty() {
        return None;
    }

    let mut best_voyage = voyages
        .iter()
        .max_by(|a, b| {
            let a_pts = extract_route_points_from_voyage(a);
            let b_pts = extract_route_points_from_voyage(b);
            a_pts.len().cmp(&b_pts.len())
        })
        .unwrap();

    let route_points = extract_route_points_from_voyage(best_voyage);
    let distance_nm = total_distance_nm(&route_points);

    let storm_count = voyages.iter().filter(|v| v.encountered_storm).count();
    let storm_risk = if voyages.is_empty() {
        0.0
    } else {
        storm_count as f64 / voyages.len() as f64
    };

    let avg_speed = distance_nm / (10.0 * HOURS_PER_DAY);

    let (dep_name, arr_name) = {
        let dep = get_port_coords(pool, departure_port_id).await;
        let arr = get_port_coords(pool, arrival_port_id).await;
        (
            dep.map(|(_, _, n)| n).unwrap_or_default(),
            arr.map(|(_, _, n)| n).unwrap_or_default(),
        )
    };

    Some(RoutePlanningResult {
        departure_port_id,
        arrival_port_id,
        departure_port_name: dep_name,
        arrival_port_name: arr_name,
        season: season.to_string(),
        ship_type: best_voyage.ship_type.clone(),
        method: "historical".to_string(),
        route_points,
        distance_nautical_miles: distance_nm,
        estimated_days: 10.0,
        avg_speed_knots: avg_speed,
        storm_risk,
        historical_deviation_pct: 0.0,
        historical_correlation: 1.0,
    })
}

pub fn compare_routes(
    optimized: &RoutePlanningResult,
    historical: Option<&RoutePlanningResult>,
) -> RouteComparison {
    let historical = match historical {
        Some(h) => h,
        None => {
            return RouteComparison {
                distance_diff_pct: 0.0,
                time_diff_pct: 0.0,
                risk_diff_pct: 0.0,
                similarity_score: 0.0,
                waypoints_matched: 0,
                total_waypoints: optimized.route_points.len() as i32,
            }
        }
    };

    let distance_diff_pct = if historical.distance_nautical_miles > 0.0 {
        ((optimized.distance_nautical_miles - historical.distance_nautical_miles)
            / historical.distance_nautical_miles * 100.0
    } else {
        0.0
    };

    let time_diff_pct = if historical.estimated_days > 0.0 {
        ((optimized.estimated_days - historical.estimated_days)
            / historical.estimated_days * 100.0
    } else {
        0.0
    };

    let risk_diff_pct = if historical.storm_risk > 0.0 {
        ((optimized.storm_risk - historical.storm_risk) / historical.storm_risk) * 100.0
    } else {
        0.0
    };

    let similarity = path_similarity_score(&optimized.route_points, &historical.route_points);

    let matched = (similarity * optimized.route_points.len() as f64) as i32;

    RouteComparison {
        distance_diff_pct,
        time_diff_pct,
        risk_diff_pct,
        similarity_score: similarity,
        waypoints_matched: matched,
        total_waypoints: optimized.route_points.len() as i32,
    }
}

#[derive(Debug, Clone)]
struct StormAvoidanceConfig {
    storm_risk_hard_threshold: f64,
    storm_risk_soft_threshold: f64,
    detour_distance_max_km: f64,
    dynamic_risk_weight: f64,
}

impl Default for StormAvoidanceConfig {
    fn default() -> Self {
        StormAvoidanceConfig {
            storm_risk_hard_threshold: 0.8,
            storm_risk_soft_threshold: 0.5,
            detour_distance_max_km: 500.0,
            dynamic_risk_weight: 2.0,
        }
    }
}

#[derive(Debug, Clone)]
struct StormCell {
    i: usize,
    j: usize,
    risk_level: f64,
    radius_km: f64,
    season: String,
}

impl StormAvoidanceConfig {
    fn from_route_config(config: &RoutePlanningConfig) -> Self {
        StormAvoidanceConfig {
            storm_risk_hard_threshold: config.storm_risk_hard_threshold,
            storm_risk_soft_threshold: config.storm_risk_soft_threshold,
            detour_distance_max_km: 500.0,
            dynamic_risk_weight: 2.0,
        }
    }
}

fn detect_extreme_weather_cells(
    grid: &RouteGrid,
    config: &StormAvoidanceConfig,
    season: &str,
) -> Vec<StormCell> {
    let mut storm_cells = Vec::new();
    for i in 0..grid.lat_steps {
        for j in 0..grid.lon_steps {
            let cell = grid.cell_at(i, j);
            if cell.storm_risk >= config.storm_risk_soft_threshold {
                let radius_km = if cell.storm_risk >= config.storm_risk_hard_threshold {
                    grid.grid_res_km * 3.0
                } else {
                    grid.grid_res_km * 1.5
                };
                storm_cells.push(StormCell {
                    i,
                    j,
                    risk_level: cell.storm_risk,
                    radius_km,
                    season: season.to_string(),
                });
            }
        }
    }
    storm_cells
}

fn apply_storm_avoidance_penalty(
    grid: &mut RouteGrid,
    storm_cells: &[StormCell],
    config: &StormAvoidanceConfig,
) {
    for storm in storm_cells {
        let radius_cells = (storm.radius_km / grid.grid_res_km).ceil() as isize;
        for di in -radius_cells..=radius_cells {
            for dj in -radius_cells..=radius_cells {
                let ni = storm.i as isize + di;
                let nj = storm.j as isize + dj;
                if ni < 0 || nj < 0 {
                    continue;
                }
                let ni = ni as usize;
                let nj = nj as usize;
                if ni >= grid.lat_steps || nj >= grid.lon_steps {
                    continue;
                }
                let dist_km = grid.cell_distance_km(storm.i, storm.j, ni, nj);
                if dist_km <= storm.radius_km {
                    let cell = grid.cell_at_mut(ni, nj);
                    if storm.risk_level >= config.storm_risk_hard_threshold {
                        cell.storm_risk = cell.storm_risk.max(config.storm_risk_hard_threshold);
                    } else {
                        let influence = 1.0 - (dist_km / storm.radius_km).min(1.0);
                        let additional_risk = storm.risk_level * influence * config.dynamic_risk_weight * 0.5;
                        cell.storm_risk = (cell.storm_risk + additional_risk).min(0.99);
                    }
                }
            }
        }
    }
}

fn plan_route_with_storm_avoidance(
    grid: &mut RouteGrid,
    start_i: usize,
    start_j: usize,
    goal_i: usize,
    goal_j: usize,
    ship_base_speed_knots: f64,
    ship_max_speed_knots: f64,
    config: &RoutePlanningConfig,
    season: &str,
) -> Option<(Vec<(usize, usize)>, f64)> {
    let storm_config = StormAvoidanceConfig::from_route_config(config);
    let storm_cells = detect_extreme_weather_cells(grid, &storm_config, season);
    apply_storm_avoidance_penalty(grid, &storm_cells, &storm_config);

    let straight_line_km = grid.cell_distance_km(start_i, start_j, goal_i, goal_j);
    let max_detour_km = straight_line_km * config.max_detour_ratio;

    if let Some((path, cost)) = a_star_search(
        grid, start_i, start_j, goal_i, goal_j, ship_base_speed_knots, ship_max_speed_knots, config,
    ) {
        let mut total_km = 0.0;
        for i in 0..path.len().saturating_sub(1) {
            let (i1, j1) = path[i];
            let (i2, j2) = path[i + 1];
            total_km += grid.cell_distance_km(i1, j1, i2, j2);
        }

        if total_km <= max_detour_km {
            let enters_high_risk = path.iter().any(|&(i, j)| {
                grid.cell_at(i, j).storm_risk >= storm_config.storm_risk_hard_threshold
            });

            if !enters_high_risk {
                return Some((path, cost));
            }
        }

        for storm in &storm_cells {
            if storm.risk_level >= storm_config.storm_risk_hard_threshold {
                let local_start_i = start_i;
                let local_start_j = start_j;
                let reroute_result = a_star_search(
                    grid, local_start_i, local_start_j, goal_i, goal_j,
                    ship_base_speed_knots, ship_max_speed_knots, config,
                );
                if let Some((new_path, new_cost)) = reroute_result {
                    let mut new_total_km = 0.0;
                    for i in 0..new_path.len().saturating_sub(1) {
                        let (i1, j1) = new_path[i];
                        let (i2, j2) = new_path[i + 1];
                        new_total_km += grid.cell_distance_km(i1, j1, i2, j2);
                    }
                    if new_total_km <= max_detour_km {
                        let enters_reroute = new_path.iter().any(|&(i, j)| {
                            grid.cell_at(i, j).storm_risk >= storm_config.storm_risk_hard_threshold
                        });
                        if !enters_reroute {
                            return Some((new_path, new_cost));
                        }
                    }
                }
            }
        }

        return Some((path, cost));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    fn test_config() -> RoutePlanningConfig {
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

    fn make_route_result(points: Vec<Vec<f64>>, distance: f64, days: f64, risk: f64) -> RoutePlanningResult {
        RoutePlanningResult {
            departure_port_id: 1,
            arrival_port_id: 2,
            departure_port_name: "A".to_string(),
            arrival_port_name: "B".to_string(),
            season: "summer".to_string(),
            ship_type: "trireme".to_string(),
            method: "test".to_string(),
            route_points: points,
            distance_nautical_miles: distance,
            estimated_days: days,
            avg_speed_knots: 5.0,
            storm_risk: risk,
            historical_deviation_pct: 0.0,
            historical_correlation: 0.0,
        }
    }

    #[test]
    fn test_deg_to_rad() {
        assert!(approx_eq(deg_to_rad(0.0), 0.0, EPSILON));
        assert!(approx_eq(deg_to_rad(90.0), std::f64::consts::PI / 2.0, EPSILON));
        assert!(approx_eq(deg_to_rad(180.0), std::f64::consts::PI, EPSILON));
        assert!(approx_eq(deg_to_rad(360.0), 2.0 * std::f64::consts::PI, EPSILON));
    }

    #[test]
    fn test_rad_to_deg() {
        assert!(approx_eq(rad_to_deg(0.0), 0.0, EPSILON));
        assert!(approx_eq(rad_to_deg(std::f64::consts::PI / 2.0), 90.0, EPSILON));
        assert!(approx_eq(rad_to_deg(std::f64::consts::PI), 180.0, EPSILON));
        assert!(approx_eq(rad_to_deg(2.0 * std::f64::consts::PI), 360.0, EPSILON));
    }

    #[test]
    fn test_haversine_same_location() {
        let dist = haversine_distance(45.0, 10.0, 45.0, 10.0);
        assert!(approx_eq(dist, 0.0, EPSILON));
    }

    #[test]
    fn test_haversine_equator_lon_diff() {
        let dist = haversine_distance(0.0, 0.0, 0.0, 1.0);
        let expected_km = 111.0;
        assert!(approx_eq(dist, expected_km, 2.0));
    }

    #[test]
    fn test_haversine_north_south() {
        let dist = haversine_distance(0.0, 0.0, 1.0, 0.0);
        let expected_km = 111.0;
        assert!(approx_eq(dist, expected_km, 2.0));
    }

    #[test]
    fn test_haversine_distance_nm() {
        let dist_km = haversine_distance(0.0, 0.0, 1.0, 0.0);
        let dist_nm = haversine_distance_nm(0.0, 0.0, 1.0, 0.0);
        assert!(approx_eq(dist_nm, dist_km * NAUTICAL_MILE_PER_KM, EPSILON));
        assert!(dist_nm > 0.0);
    }

    #[test]
    fn test_bearing_north() {
        let brng = bearing(0.0, 0.0, 1.0, 0.0);
        assert!(approx_eq(brng, 0.0, 0.5));
    }

    #[test]
    fn test_bearing_east() {
        let brng = bearing(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(brng, 90.0, 0.5));
    }

    #[test]
    fn test_bearing_south() {
        let brng = bearing(1.0, 0.0, 0.0, 0.0);
        assert!(approx_eq(brng, 180.0, 0.5));
    }

    #[test]
    fn test_bearing_west() {
        let brng = bearing(0.0, 1.0, 0.0, 0.0);
        assert!(approx_eq(brng, 270.0, 0.5));
    }

    #[test]
    fn test_direction_to_vector_0_deg() {
        let (sin, cos) = direction_to_vector(0.0);
        assert!(approx_eq(sin, 0.0, EPSILON));
        assert!(approx_eq(cos, 1.0, EPSILON));
    }

    #[test]
    fn test_direction_to_vector_90_deg() {
        let (sin, cos) = direction_to_vector(90.0);
        assert!(approx_eq(sin, 1.0, EPSILON));
        assert!(approx_eq(cos, 0.0, EPSILON));
    }

    #[test]
    fn test_direction_to_vector_180_deg() {
        let (sin, cos) = direction_to_vector(180.0);
        assert!(approx_eq(sin, 0.0, EPSILON));
        assert!(approx_eq(cos, -1.0, EPSILON));
    }

    #[test]
    fn test_route_grid_new_bounds() {
        let grid = RouteGrid::new(0.0, 10.0, 20.0, 30.0, 50.0);
        assert!(approx_eq(grid.lat_min, 0.0, EPSILON));
        assert!(approx_eq(grid.lat_max, 10.0, EPSILON));
        assert!(approx_eq(grid.lon_min, 20.0, EPSILON));
        assert!(approx_eq(grid.lon_max, 30.0, EPSILON));
    }

    #[test]
    fn test_route_grid_min_steps() {
        let grid = RouteGrid::new(0.0, 0.1, 0.0, 0.1, 1000.0);
        assert!(grid.lat_steps >= 2);
        assert!(grid.lon_steps >= 2);
    }

    #[test]
    fn test_route_grid_cell_corners() {
        let grid = RouteGrid::new(0.0, 10.0, 0.0, 10.0, 50.0);
        let (i, j) = grid.lat_lon_to_idx(0.0, 0.0);
        assert_eq!(i, 0);
        assert_eq!(j, 0);

        let (i, j) = grid.lat_lon_to_idx(10.0, 10.0);
        assert_eq!(i, grid.lat_steps - 1);
        assert_eq!(j, grid.lon_steps - 1);
    }

    #[test]
    fn test_route_grid_clamp_outside() {
        let grid = RouteGrid::new(0.0, 10.0, 0.0, 10.0, 50.0);
        let (i, j) = grid.lat_lon_to_idx(-5.0, -5.0);
        assert_eq!(i, 0);
        assert_eq!(j, 0);

        let (i, j) = grid.lat_lon_to_idx(15.0, 15.0);
        assert_eq!(i, grid.lat_steps - 1);
        assert_eq!(j, grid.lon_steps - 1);
    }

    #[test]
    fn test_route_grid_idx_roundtrip() {
        let grid = RouteGrid::new(0.0, 10.0, 0.0, 10.0, 50.0);
        for i in 0..grid.lat_steps {
            for j in 0..grid.lon_steps {
                let (lat, lon) = grid.idx_to_lat_lon(i, j);
                let (i2, j2) = grid.lat_lon_to_idx(lat, lon);
                assert_eq!(i, i2);
                assert_eq!(j, j2);
            }
        }
    }

    #[test]
    fn test_route_grid_cell_at() {
        let grid = RouteGrid::new(0.0, 10.0, 0.0, 10.0, 50.0);
        let cell = grid.cell_at(0, 0);
        assert!(cell.lat >= grid.lat_min - EPSILON);
        assert!(cell.lon >= grid.lon_min - EPSILON);
    }

    #[test]
    fn test_route_grid_cell_distance() {
        let grid = RouteGrid::new(0.0, 10.0, 0.0, 10.0, 50.0);
        let dist = grid.cell_distance_km(0, 0, 0, 0);
        assert!(approx_eq(dist, 0.0, EPSILON));

        let dist = grid.cell_distance_nm(0, 0, 1, 1);
        assert!(dist > 0.0);
    }

    fn set_obstacle(grid: &mut RouteGrid, i: usize, j: usize) {
        let cell = grid.cell_at_mut(i, j);
        cell.storm_risk = 100.0;
    }

    #[test]
    fn test_a_star_start_is_goal() {
        let grid = RouteGrid::new(0.0, 5.0, 0.0, 5.0, 100.0);
        let config = test_config();
        let result = a_star_search(&grid, 0, 0, 0, 0, 5.0, 7.5, &config);
        assert!(result.is_some());
        let (path, cost) = result.unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0], (0, 0));
        assert!(approx_eq(cost, 0.0, EPSILON));
    }

    #[test]
    fn test_a_star_straight_line_no_obstacles() {
        let grid = RouteGrid::new(0.0, 2.0, 0.0, 0.0, 50.0);
        let config = test_config();
        let result = a_star_search(&grid, 0, 0, grid.lat_steps - 1, 0, 5.0, 7.5, &config);
        assert!(result.is_some());
        let (path, _cost) = result.unwrap();
        assert!(path.len() >= 2);
        assert_eq!(path[0], (0, 0));
        assert_eq!(path[path.len() - 1], (grid.lat_steps - 1, 0));
    }

    #[test]
    fn test_a_star_obstacle_detour() {
        let mut grid = RouteGrid::new(0.0, 2.0, 0.0, 2.0, 50.0);
        let mid_i = grid.lat_steps / 2;
        let mid_j = grid.lon_steps / 2;
        for j in 0..grid.lon_steps {
            if j != mid_j {
                set_obstacle(&mut grid, mid_i, j);
            }
        }
        let config = test_config();
        let result = a_star_search(&grid, 0, 0, grid.lat_steps - 1, 0, 5.0, 7.5, &config);
        assert!(result.is_some());
        let (path, _) = result.unwrap();
        assert!(path.len() > 2);
        assert_eq!(path[0], (0, 0));
        assert_eq!(path[path.len() - 1], (grid.lat_steps - 1, 0));
    }

    #[test]
    fn test_heuristic_admissibility() {
        let grid = RouteGrid::new(0.0, 5.0, 0.0, 5.0, 50.0);
        let config = test_config();
        let max_speed = 7.5;
        let base_speed = 5.0;

        for i in 0..grid.lat_steps {
            for j in 0..grid.lon_steps {
                let h = heuristic(&grid, i, j, grid.lat_steps - 1, grid.lon_steps - 1, max_speed);
                let result = a_star_search(&grid, i, j, grid.lat_steps - 1, grid.lon_steps - 1, base_speed, max_speed, &config);
                if let Some((_, actual_cost)) = result {
                    assert!(h <= actual_cost + EPSILON, "Heuristic overestimates: h={}, actual={}", h, actual_cost);
                }
            }
        }
    }

    #[test]
    fn test_a_star_open_set_push_pop() {
        let mut open_set = AStarOpenSet::new();
        assert!(open_set.is_empty());

        open_set.push(RouteNode { f: 10.0, g: 5.0, h: 5.0, i: 0, j: 0 });
        assert!(!open_set.is_empty());

        let node = open_set.pop();
        assert!(node.is_some());
        assert_eq!(node.unwrap().g, 5.0);
        assert!(open_set.is_empty());
    }

    #[test]
    fn test_a_star_open_set_best_g() {
        let mut open_set = AStarOpenSet::new();
        open_set.push(RouteNode { f: 10.0, g: 5.0, h: 5.0, i: 0, j: 0 });
        open_set.push(RouteNode { f: 8.0, g: 3.0, h: 5.0, i: 0, j: 0 });

        assert_eq!(open_set.get_best_g(0, 0), Some(3.0));

        let node = open_set.pop().unwrap();
        assert_eq!(node.g, 3.0);
    }

    #[test]
    fn test_compare_routes_same_route() {
        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![2.0, 0.0],
        ];
        let route = make_route_result(points.clone(), 100.0, 5.0, 0.2);
        let comparison = compare_routes(&route, Some(&route));
        assert!(approx_eq(comparison.similarity_score, 1.0, EPSILON));
        assert!(approx_eq(comparison.distance_diff_pct, 0.0, EPSILON));
        assert!(approx_eq(comparison.time_diff_pct, 0.0, EPSILON));
        assert!(approx_eq(comparison.risk_diff_pct, 0.0, EPSILON));
    }

    #[test]
    fn test_compare_routes_historical_none() {
        let points = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
        let route = make_route_result(points, 100.0, 5.0, 0.2);
        let comparison = compare_routes(&route, None);
        assert!(approx_eq(comparison.distance_diff_pct, 0.0, EPSILON));
        assert!(approx_eq(comparison.time_diff_pct, 0.0, EPSILON));
        assert!(approx_eq(comparison.risk_diff_pct, 0.0, EPSILON));
        assert!(approx_eq(comparison.similarity_score, 0.0, EPSILON));
        assert_eq!(comparison.waypoints_matched, 0);
    }

    #[test]
    fn test_compare_routes_distance_diff() {
        let points_a = vec![vec![0.0, 0.0], vec![2.0, 0.0]];
        let points_b = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
        let route_a = make_route_result(points_a, 200.0, 10.0, 0.3);
        let route_b = make_route_result(points_b, 100.0, 5.0, 0.2);

        let comparison = compare_routes(&route_a, Some(&route_b));
        assert!(approx_eq(comparison.distance_diff_pct, 100.0, EPSILON));
        assert!(approx_eq(comparison.time_diff_pct, 100.0, EPSILON));
        assert!(approx_eq(comparison.risk_diff_pct, 50.0, EPSILON));
    }

    #[test]
    fn test_path_similarity_same_path() {
        let route = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![2.0, 0.0],
            vec![3.0, 0.0],
        ];
        let score = path_similarity_score(&route, &route);
        assert!(approx_eq(score, 1.0, EPSILON));
    }

    #[test]
    fn test_path_similarity_very_different() {
        let route_a = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![2.0, 0.0],
        ];
        let route_b = vec![
            vec![0.0, 10.0],
            vec![1.0, 10.0],
            vec![2.0, 10.0],
        ];
        let score = path_similarity_score(&route_a, &route_b);
        assert!(score < 0.5);
        assert!(score >= 0.0);
    }

    #[test]
    fn test_path_similarity_partial_overlap() {
        let route_a = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![2.0, 0.0],
            vec![3.0, 0.0],
            vec![4.0, 0.0],
        ];
        let route_b = vec![
            vec![1.0, 0.0],
            vec![2.0, 0.0],
            vec![3.0, 0.0],
        ];
        let score = path_similarity_score(&route_a, &route_b);
        assert!(score > 0.0);
        assert!(score < 1.0);
    }

    #[test]
    fn test_total_distance_empty() {
        let route: Vec<Vec<f64>> = Vec::new();
        assert!(approx_eq(total_distance_nm(&route), 0.0, EPSILON));
    }

    #[test]
    fn test_total_distance_single_point() {
        let route = vec![vec![0.0, 0.0]];
        assert!(approx_eq(total_distance_nm(&route), 0.0, EPSILON));
    }

    #[test]
    fn test_total_distance_two_points() {
        let route = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
        let dist = total_distance_nm(&route);
        assert!(dist > 0.0);
    }

    #[test]
    fn test_empty_path_similarity() {
        let empty: Vec<Vec<f64>> = Vec::new();
        let route = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
        let score = path_similarity_score(&empty, &route);
        assert!(approx_eq(score, 0.0, EPSILON));

        let score = path_similarity_score(&route, &empty);
        assert!(approx_eq(score, 0.0, EPSILON));
    }

    #[test]
    fn test_single_point_similarity() {
        let single = vec![vec![0.0, 0.0]];
        let route = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
        let score = path_similarity_score(&single, &route);
        assert!(approx_eq(score, 0.0, EPSILON));
    }

    #[test]
    fn test_frechet_distance_approx_empty() {
        let empty: Vec<Vec<f64>> = Vec::new();
        let route = vec![vec![0.0, 0.0]];
        assert!(approx_eq(frechet_distance_approx(&empty, &route), 0.0, EPSILON));
        assert!(approx_eq(frechet_distance_approx(&route, &empty), 0.0, EPSILON));
    }

    #[test]
    fn test_average_distance_deviation_empty() {
        let empty: Vec<Vec<f64>> = Vec::new();
        let route = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
        assert!(approx_eq(average_distance_deviation(&empty, &route), 0.0, EPSILON));
        assert!(approx_eq(average_distance_deviation(&route, &empty), 0.0, EPSILON));
    }

    #[test]
    fn test_bilinear_interpolate_empty() {
        let points: Vec<(f64, f64, f64)> = Vec::new();
        assert!(approx_eq(bilinear_interpolate(&points, 0.0, 0.0), 0.0, EPSILON));
    }

    #[test]
    fn test_bilinear_interpolate_single() {
        let points = vec![(1.0, 2.0, 42.0)];
        assert!(approx_eq(bilinear_interpolate(&points, 1.0, 2.0), 42.0, EPSILON));
    }

    #[test]
    fn test_ship_base_speed() {
        assert!(approx_eq(ship_base_speed("trireme"), 6.0, EPSILON));
        assert!(approx_eq(ship_base_speed("galley"), 5.0, EPSILON));
        assert!(approx_eq(ship_base_speed("unknown"), 5.0, EPSILON));
    }

    #[test]
    fn test_ship_max_speed() {
        assert!(approx_eq(ship_max_speed("trireme"), 9.0, EPSILON));
        assert!(approx_eq(ship_max_speed("galley"), 7.5, EPSILON));
    }

    #[test]
    fn test_speed_projection_on_heading() {
        let proj = speed_projection_on_heading(10.0, 0.0, 0.0);
        assert!(approx_eq(proj, 10.0, EPSILON));

        let proj = speed_projection_on_heading(10.0, 90.0, 0.0);
        assert!(approx_eq(proj, 0.0, 0.001));

        let proj = speed_projection_on_heading(10.0, 180.0, 0.0);
        assert!(approx_eq(proj, -10.0, EPSILON));
    }

    #[test]
    fn test_populate_grid_with_storm_risk() {
        let mut grid = RouteGrid::new(0.0, 5.0, 0.0, 5.0, 50.0);
        populate_grid_with_storm_risk(&mut grid, 0.25);
        for i in 0..grid.lat_steps {
            for j in 0..grid.lon_steps {
                assert!(approx_eq(grid.cell_at(i, j).storm_risk, 0.25, EPSILON));
            }
        }
    }

    #[test]
    fn test_correlation_coefficient_empty() {
        assert!(approx_eq(correlation_coefficient(&[], &[]), 0.0, EPSILON));
        assert!(approx_eq(correlation_coefficient(&[1.0], &[2.0]), 0.0, EPSILON));
    }

    #[test]
    fn test_correlation_coefficient_perfect() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = correlation_coefficient(&xs, &ys);
        assert!(approx_eq(r, 1.0, 0.001));
    }

    #[test]
    fn test_correlation_coefficient_negative() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        let r = correlation_coefficient(&xs, &ys);
        assert!(approx_eq(r, -1.0, 0.001));
    }

    #[test]
    fn test_compute_historical_correlation_empty() {
        let route = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
        let historical: Vec<Vec<Vec<f64>>> = Vec::new();
        assert!(approx_eq(compute_historical_correlation(&route, &historical), 0.0, EPSILON));
    }

    #[test]
    fn test_route_node_ord() {
        let a = RouteNode { f: 10.0, g: 5.0, h: 5.0, i: 0, j: 0 };
        let b = RouteNode { f: 5.0, g: 2.0, h: 3.0, i: 1, j: 1 };
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_point_to_line_distance() {
        let d = point_to_line_distance(0.0, 1.0, 0.0, 0.0, 2.0, 0.0);
        assert!(d > 0.0);

        let d = point_to_line_distance(1.0, 0.0, 0.0, 0.0, 2.0, 0.0);
        assert!(approx_eq(d, 0.0, 0.1));
    }

    #[test]
    fn test_path_to_route_points() {
        let grid = RouteGrid::new(0.0, 10.0, 0.0, 10.0, 50.0);
        let path = vec![(0, 0), (grid.lat_steps - 1, grid.lon_steps - 1)];
        let points = path_to_route_points(&grid, &path);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].len(), 2);
    }

    #[test]
    fn test_extract_route_points_from_voyage_empty() {
        let voyage = VoyageRecord {
            id: 1,
            departure_port_id: 1,
            arrival_port_id: 2,
            voyage_year: 100,
            season: "summer".to_string(),
            ship_type: "trireme".to_string(),
            cargo_type: "grain".to_string(),
            encountered_storm: false,
            route_points: None,
            created_at: None,
        };
        let points = extract_route_points_from_voyage(&voyage);
        assert!(points.is_empty());
    }
}
