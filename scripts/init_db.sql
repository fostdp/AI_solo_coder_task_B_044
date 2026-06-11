CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

SET work_mem = '64MB';
SET maintenance_work_mem = '256MB';
SET effective_cache_size = '2GB';
SET shared_buffers = '512MB';
SET random_page_cost = 1.1;
SET cpu_tuple_cost = 0.005;
SET min_parallel_table_scan_size = '8MB';

CREATE TABLE IF NOT EXISTS ports (
    id SERIAL PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    name_zh VARCHAR(200),
    region VARCHAR(100),
    geom GEOMETRY(Point, 4326) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS voyage_records (
    id SERIAL PRIMARY KEY,
    departure_port_id INTEGER REFERENCES ports(id),
    arrival_port_id INTEGER REFERENCES ports(id),
    voyage_year INTEGER NOT NULL,
    season VARCHAR(20) NOT NULL CHECK (season IN ('spring','summer','autumn','winter')),
    ship_type VARCHAR(50) NOT NULL,
    cargo_type VARCHAR(50) NOT NULL,
    encountered_storm BOOLEAN DEFAULT FALSE,
    route_geom GEOMETRY(LineString, 4326),
    route_points JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS climate_periods (
    id SERIAL PRIMARY KEY,
    period_start INTEGER NOT NULL,
    period_end INTEGER NOT NULL,
    avg_temperature NUMERIC(5,2),
    avg_wind_speed NUMERIC(5,2),
    avg_rainfall NUMERIC(6,2),
    storm_frequency NUMERIC(5,4),
    nao_index NUMERIC(5,2),
    description TEXT,
    CONSTRAINT chk_period_range CHECK (period_start <= period_end)
);

CREATE TABLE IF NOT EXISTS ocean_currents (
    id SERIAL PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    period_id INTEGER REFERENCES climate_periods(id),
    season VARCHAR(20) NOT NULL,
    direction_deg NUMERIC(5,2),
    speed_knots NUMERIC(5,2),
    geom GEOMETRY(LineString, 4326),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS wind_fields (
    id SERIAL PRIMARY KEY,
    period_id INTEGER REFERENCES climate_periods(id),
    season VARCHAR(20) NOT NULL,
    region VARCHAR(100),
    avg_direction_deg NUMERIC(5,2),
    avg_speed_knots NUMERIC(5,2),
    variability NUMERIC(5,2),
    geom GEOMETRY(Polygon, 4326)
);

CREATE TABLE IF NOT EXISTS port_aliases (
    id SERIAL PRIMARY KEY,
    port_id INTEGER REFERENCES ports(id),
    alias_name VARCHAR(200) NOT NULL,
    alias_name_zh VARCHAR(200),
    period_start INTEGER,
    period_end INTEGER,
    language VARCHAR(50),
    source VARCHAR(200)
);

CREATE TABLE IF NOT EXISTS network_analysis_results (
    id SERIAL PRIMARY KEY,
    period_start INTEGER NOT NULL,
    period_end INTEGER NOT NULL,
    port_id INTEGER REFERENCES ports(id),
    betweenness_centrality NUMERIC(10,6),
    degree_centrality NUMERIC(10,6),
    trade_flow NUMERIC(12,2),
    community_id INTEGER,
    is_hub BOOLEAN DEFAULT FALSE,
    computed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS storm_risk_results (
    id SERIAL PRIMARY KEY,
    departure_port_id INTEGER REFERENCES ports(id),
    arrival_port_id INTEGER REFERENCES ports(id),
    season VARCHAR(20) NOT NULL,
    period_id INTEGER REFERENCES climate_periods(id),
    risk_score NUMERIC(5,4),
    sample_size INTEGER,
    model_type VARCHAR(50),
    confidence NUMERIC(5,4),
    computed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_ports_geom ON ports USING GIST(geom) WITH (fillfactor = 90);
CREATE INDEX idx_ports_region ON ports(region);
CREATE INDEX idx_ports_name_trgm ON ports USING GIN (name gin_trgm_ops);
CREATE INDEX idx_ports_name_zh_trgm ON ports USING GIN (name_zh gin_trgm_ops);

CREATE INDEX idx_voyage_route_geom ON voyage_records USING GIST(route_geom) WITH (fillfactor = 90);
CREATE INDEX idx_voyage_departure ON voyage_records(departure_port_id);
CREATE INDEX idx_voyage_arrival ON voyage_records(arrival_port_id);
CREATE INDEX idx_voyage_year ON voyage_records(voyage_year);
CREATE INDEX idx_voyage_season ON voyage_records(season);
CREATE INDEX idx_voyage_cargo ON voyage_records(cargo_type);
CREATE INDEX idx_voyage_storm ON voyage_records(encountered_storm);
CREATE INDEX idx_voyage_year_season ON voyage_records(voyage_year, season);
CREATE INDEX idx_voyage_route_year ON voyage_records(departure_port_id, arrival_port_id, voyage_year);

CREATE INDEX idx_climate_period_range ON climate_periods(period_start, period_end);
CREATE INDEX idx_climate_storm ON climate_periods(storm_frequency);

CREATE INDEX idx_current_geom ON ocean_currents USING GIST(geom) WITH (fillfactor = 90);
CREATE INDEX idx_current_period_season ON ocean_currents(period_id, season);
CREATE INDEX idx_current_name ON ocean_currents(name);

CREATE INDEX idx_wind_geom ON wind_fields USING GIST(geom) WITH (fillfactor = 90);
CREATE INDEX idx_wind_period_season ON wind_fields(period_id, season);
CREATE INDEX idx_wind_region ON wind_fields(region);

CREATE INDEX idx_network_port ON network_analysis_results(port_id);
CREATE INDEX idx_network_period ON network_analysis_results(period_start, period_end);
CREATE INDEX idx_network_community ON network_analysis_results(community_id);
CREATE INDEX idx_network_hub ON network_analysis_results(is_hub);

CREATE INDEX idx_storm_risk_route ON storm_risk_results(departure_port_id, arrival_port_id);
CREATE INDEX idx_storm_risk_model ON storm_risk_results(model_type);
CREATE INDEX idx_storm_risk_score ON storm_risk_results(risk_score);

CREATE INDEX idx_port_aliases_name ON port_aliases(alias_name);
CREATE INDEX idx_port_aliases_name_trgm ON port_aliases USING GIN (alias_name gin_trgm_ops);
CREATE INDEX idx_port_aliases_name_zh ON port_aliases(alias_name_zh);
CREATE INDEX idx_port_aliases_port ON port_aliases(port_id);
CREATE INDEX idx_port_aliases_period ON port_aliases(port_id, period_start, period_end);

ANALYZE ports;
ANALYZE voyage_records;
ANALYZE climate_periods;
ANALYZE ocean_currents;
ANALYZE wind_fields;
ANALYZE port_aliases;
ANALYZE network_analysis_results;
ANALYZE storm_risk_results;

-- ============================================================
-- 扩展表：港口兴衰分析、货物传播、航线规划、现代航运对比
-- ============================================================

CREATE TABLE IF NOT EXISTS historical_events (
    id SERIAL PRIMARY KEY,
    event_name VARCHAR(200) NOT NULL,
    event_name_zh VARCHAR(200),
    event_type VARCHAR(50) NOT NULL,
    region VARCHAR(100),
    start_year INTEGER NOT NULL,
    end_year INTEGER,
    severity NUMERIC(3,2),
    affected_port_ids INTEGER[],
    description TEXT,
    source VARCHAR(200),
    geom GEOMETRY(Point, 4326),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_hist_events_type ON historical_events(event_type);
CREATE INDEX idx_hist_events_region ON historical_events(region);
CREATE INDEX idx_hist_events_year ON historical_events(start_year);
CREATE INDEX idx_hist_events_geom ON historical_events USING GIST(geom);

CREATE TABLE IF NOT EXISTS port_yearly_flow (
    id SERIAL PRIMARY KEY,
    port_id INTEGER REFERENCES ports(id),
    year INTEGER NOT NULL,
    departure_count INTEGER DEFAULT 0,
    arrival_count INTEGER DEFAULT 0,
    total_flow INTEGER DEFAULT 0,
    storm_count INTEGER DEFAULT 0,
    storm_rate NUMERIC(5,4),
    unique_cargo_types INTEGER DEFAULT 0,
    unique_destinations INTEGER DEFAULT 0,
    flow_rank INTEGER,
    UNIQUE(port_id, year)
);

CREATE INDEX idx_port_flow_year ON port_yearly_flow(year);
CREATE INDEX idx_port_flow_port ON port_yearly_flow(port_id);
CREATE INDEX idx_port_flow_rank ON port_yearly_flow(flow_rank);

CREATE TABLE IF NOT EXISTS cargo_spread_records (
    id SERIAL PRIMARY KEY,
    cargo_type VARCHAR(50) NOT NULL,
    from_port_id INTEGER REFERENCES ports(id),
    to_port_id INTEGER REFERENCES ports(id),
    voyage_year INTEGER NOT NULL,
    spread_direction VARCHAR(20) NOT NULL,
    quantity_estimate NUMERIC(10,2),
    cultural_significance TEXT,
    spread_path_id INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_cargo_spread_type ON cargo_spread_records(cargo_type);
CREATE INDEX idx_cargo_spread_year ON cargo_spread_records(voyage_year);
CREATE INDEX idx_cargo_spread_from ON cargo_spread_records(from_port_id);
CREATE INDEX idx_cargo_spread_to ON cargo_spread_records(to_port_id);

CREATE TABLE IF NOT EXISTS tech_diffusion_paths (
    id SERIAL PRIMARY KEY,
    tech_name VARCHAR(100) NOT NULL,
    tech_name_zh VARCHAR(100),
    tech_category VARCHAR(50),
    origin_port_id INTEGER REFERENCES ports(id),
    spread_route INTEGER[],
    estimated_start_year INTEGER,
    estimated_end_year INTEGER,
    diffusion_speed_km_yr NUMERIC(8,2),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_tech_diff_cat ON tech_diffusion_paths(tech_category);
CREATE INDEX idx_tech_diff_start ON tech_diffusion_paths(estimated_start_year);

CREATE TABLE IF NOT EXISTS route_planning_results (
    id SERIAL PRIMARY KEY,
    departure_port_id INTEGER REFERENCES ports(id),
    arrival_port_id INTEGER REFERENCES ports(id),
    departure_port_name VARCHAR(200),
    arrival_port_name VARCHAR(200),
    season VARCHAR(20) NOT NULL,
    ship_type VARCHAR(50) NOT NULL,
    method VARCHAR(50) NOT NULL,
    route_points JSONB,
    route_geom GEOMETRY(LineString, 4326),
    distance_nautical_miles NUMERIC(10,2),
    estimated_days NUMERIC(8,2),
    avg_speed_knots NUMERIC(5,2),
    storm_risk NUMERIC(5,4),
    historical_deviation_pct NUMERIC(5,2),
    historical_correlation NUMERIC(5,4),
    computed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_route_plan_dep ON route_planning_results(departure_port_id);
CREATE INDEX idx_route_plan_arr ON route_planning_results(arrival_port_id);
CREATE INDEX idx_route_plan_season ON route_planning_results(season);
CREATE INDEX idx_route_plan_geom ON route_planning_results USING GIST(route_geom);

CREATE TABLE IF NOT EXISTS modern_ships (
    id SERIAL PRIMARY KEY,
    ship_name VARCHAR(100),
    mmsi VARCHAR(20) UNIQUE,
    ship_type VARCHAR(50),
    gross_tonnage NUMERIC(10,2),
    length_m NUMERIC(6,2),
    beam_m NUMERIC(5,2),
    max_speed_knots NUMERIC(5,2),
    flag VARCHAR(50),
    home_port VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_modern_ships_type ON modern_ships(ship_type);
CREATE INDEX idx_modern_ships_mmsi ON modern_ships(mmsi);

CREATE TABLE IF NOT EXISTS modern_weather_forecasts (
    id SERIAL PRIMARY KEY,
    forecast_date DATE NOT NULL,
    region VARCHAR(100),
    wind_direction_deg NUMERIC(5,2),
    wind_speed_knots NUMERIC(5,2),
    wave_height_m NUMERIC(4,2),
    current_direction_deg NUMERIC(5,2),
    current_speed_knots NUMERIC(5,2),
    visibility_nm NUMERIC(5,1),
    storm_probability NUMERIC(5,4),
    geom GEOMETRY(Polygon, 4326),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_modern_weather_date ON modern_weather_forecasts(forecast_date);
CREATE INDEX idx_modern_weather_region ON modern_weather_forecasts(region);
CREATE INDEX idx_modern_weather_geom ON modern_weather_forecasts USING GIST(geom);

CREATE TABLE IF NOT EXISTS modern_risk_results (
    id SERIAL PRIMARY KEY,
    departure_port_id INTEGER REFERENCES ports(id),
    arrival_port_id INTEGER REFERENCES ports(id),
    forecast_date DATE,
    model_type VARCHAR(50),
    risk_score NUMERIC(5,4),
    risk_level VARCHAR(20),
    estimated_delay_hours NUMERIC(6,1),
    alternative_route_suggestion TEXT,
    ancient_comparison_score NUMERIC(5,4),
    geom GEOMETRY(LineString, 4326),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_modern_risk_route ON modern_risk_results(departure_port_id, arrival_port_id);
CREATE INDEX idx_modern_risk_score ON modern_risk_results(risk_score);
CREATE INDEX idx_modern_risk_date ON modern_risk_results(forecast_date);
CREATE INDEX idx_modern_risk_geom ON modern_risk_results USING GIST(geom);

CREATE TABLE IF NOT EXISTS panel_regression_results (
    id SERIAL PRIMARY KEY,
    port_id INTEGER REFERENCES ports(id),
    dependent_var VARCHAR(50),
    model_type VARCHAR(50),
    period_start INTEGER,
    period_end INTEGER,
    coefficients JSONB,
    r_squared NUMERIC(5,4),
    adj_r_squared NUMERIC(5,4),
    f_statistic NUMERIC(8,4),
    p_value NUMERIC(8,4),
    n_observations INTEGER,
    computed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_panel_reg_port ON panel_regression_results(port_id);
CREATE INDEX idx_panel_reg_period ON panel_regression_results(period_start, period_end);

CREATE TABLE IF NOT EXISTS granger_causality_results (
    id SERIAL PRIMARY KEY,
    port_id INTEGER REFERENCES ports(id),
    cause_variable VARCHAR(50),
    effect_variable VARCHAR(50),
    lag_order INTEGER,
    f_statistic NUMERIC(8,4),
    p_value NUMERIC(8,4),
    is_significant BOOLEAN,
    direction VARCHAR(20),
    period_start INTEGER,
    period_end INTEGER,
    computed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_granger_port ON granger_causality_results(port_id);
CREATE INDEX idx_granger_signif ON granger_causality_results(is_significant);

ANALYZE historical_events;
ANALYZE port_yearly_flow;
ANALYZE cargo_spread_records;
ANALYZE tech_diffusion_paths;
ANALYZE route_planning_results;
ANALYZE modern_ships;
ANALYZE modern_weather_forecasts;
ANALYZE modern_risk_results;
ANALYZE panel_regression_results;
ANALYZE granger_causality_results;
