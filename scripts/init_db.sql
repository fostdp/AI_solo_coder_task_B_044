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
