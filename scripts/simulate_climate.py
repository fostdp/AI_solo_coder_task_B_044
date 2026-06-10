import json
import random
import math
try:
    import psycopg2
    from psycopg2.extras import execute_values
    HAS_PG = True
except ImportError:
    HAS_PG = False

OCEAN_REGIONS = [
    {"name": "Mediterranean_Western", "bounds": [30, -10, 45, 15]},
    {"name": "Mediterranean_Eastern", "bounds": [30, 15, 42, 36]},
    {"name": "Red_Sea", "bounds": [12, 32, 30, 44]},
    {"name": "Indian_Ocean_Western", "bounds": [-30, 30, 15, 55]},
    {"name": "Indian_Ocean_Northern", "bounds": [0, 55, 30, 80]},
    {"name": "South_China_Sea", "bounds": [0, 100, 25, 120]},
    {"name": "East_China_Sea", "bounds": [25, 120, 40, 130]},
    {"name": "Atlantic_North", "bounds": [40, -30, 65, 0]},
    {"name": "Atlantic_South", "bounds": [0, -30, 40, 10]},
    {"name": "Bay_of_Bengal", "bounds": [5, 78, 22, 95]},
]

SEASONS = ["spring", "summer", "autumn", "winter"]

CURRENT_SYSTEMS = [
    {"name": "North Atlantic Drift", "region": "Atlantic_North", "base_dir": 60, "base_speed": 0.8},
    {"name": "Canary Current", "region": "Atlantic_South", "base_dir": 180, "base_speed": 0.6},
    {"name": "Mediterranean Gyre West", "region": "Mediterranean_Western", "base_dir": 90, "base_speed": 0.3},
    {"name": "Mediterranean Gyre East", "region": "Mediterranean_Eastern", "base_dir": 270, "base_speed": 0.3},
    {"name": "Monsoon Current NE", "region": "Indian_Ocean_Northern", "base_dir": 45, "base_speed": 1.2},
    {"name": "Monsoon Current SW", "region": "Indian_Ocean_Northern", "base_dir": 225, "base_speed": 1.0},
    {"name": "Agulhas Current", "region": "Indian_Ocean_Western", "base_dir": 180, "base_speed": 1.5},
    {"name": "Kuroshio Current", "region": "East_China_Sea", "base_dir": 30, "base_speed": 1.3},
    {"name": "South China Sea Current", "region": "South_China_Sea", "base_dir": 340, "base_speed": 0.5},
    {"name": "Red Sea Current", "region": "Red_Sea", "base_dir": 160, "base_speed": 0.4},
]

CLIMATE_DESCRIPTIONS = {
    (-1000, -950): "Late Bronze Age collapse period, cooler and drier",
    (-950, -500): "Subatlantic cool period, increased storm activity",
    (-500, -200): "Roman Warm Period onset, milder conditions",
    (-200, 150): "Roman Warm Period peak, stable conditions",
    (150, 400): "Late Roman transition, increasing variability",
    (400, 700): "Late Antique Little Ice Age, cooler and stormier",
    (700, 900): "Medieval Warm Period onset, warming trend",
    (900, 1200): "Medieval Warm Period peak, favorable sailing",
    (1200, 1350): "Medieval Warm Period decline",
    (1350, 1500): "Little Ice Age onset, harsher conditions",
    (1500, 1650): "Little Ice Age, Maunder Minimum approach",
    (1650, 1800): "Little Ice Age, Maunder Minimum, cold and stormy",
}

def generate_climate_periods():
    periods = []
    period_id = 1
    for start in range(-1000, 1801, 50):
        end = start + 49
        if start < -500:
            base_temp = 13.5 + random.gauss(0, 0.3)
            base_wind = 12.0 + random.gauss(0, 0.5)
            storm_freq = 0.15 + random.gauss(0, 0.02)
        elif start < 150:
            base_temp = 14.5 + random.gauss(0, 0.3)
            base_wind = 10.0 + random.gauss(0, 0.5)
            storm_freq = 0.10 + random.gauss(0, 0.02)
        elif start < 700:
            base_temp = 13.0 + random.gauss(0, 0.4)
            base_wind = 13.0 + random.gauss(0, 0.5)
            storm_freq = 0.18 + random.gauss(0, 0.02)
        elif start < 1200:
            base_temp = 14.8 + random.gauss(0, 0.3)
            base_wind = 9.5 + random.gauss(0, 0.5)
            storm_freq = 0.08 + random.gauss(0, 0.02)
        else:
            base_temp = 12.5 + random.gauss(0, 0.4)
            base_wind = 14.0 + random.gauss(0, 0.6)
            storm_freq = 0.22 + random.gauss(0, 0.03)

        nao = random.gauss(0, 1.5)
        rainfall = 800 + random.gauss(0, 100)

        desc = None
        for (ds, de), d in CLIMATE_DESCRIPTIONS.items():
            if ds <= start <= de:
                desc = d
                break

        periods.append({
            "id": period_id,
            "period_start": start,
            "period_end": end,
            "avg_temperature": round(base_temp, 2),
            "avg_wind_speed": round(base_wind, 2),
            "avg_rainfall": round(rainfall, 2),
            "storm_frequency": round(max(0.01, storm_freq), 4),
            "nao_index": round(nao, 2),
            "description": desc,
        })
        period_id += 1
    return periods

def generate_wind_fields(periods):
    fields = []
    for period in periods:
        for region in OCEAN_REGIONS:
            for season in SEASONS:
                season_factor = {"spring": 1.0, "summer": 0.8, "autumn": 1.2, "winter": 1.4}
                factor = season_factor[season]
                wind_speed = period["avg_wind_speed"] * factor + random.gauss(0, 1)
                wind_dir = random.gauss(180, 45) % 360
                variability = random.gauss(15, 5)

                b = region["bounds"]
                polygon = (f"SRID=4326;POLYGON(({b[1]} {b[0]},{b[3]} {b[0]},"
                          f"{b[3]} {b[2]},{b[1]} {b[2]},{b[1]} {b[0]}))")

                fields.append({
                    "period_id": period["id"],
                    "season": season,
                    "region": region["name"],
                    "avg_direction_deg": round(wind_dir, 2),
                    "avg_speed_knots": round(max(0.1, wind_speed), 2),
                    "variability": round(max(1, variability), 2),
                    "geom": polygon,
                })
    return fields

def generate_ocean_currents(periods):
    currents = []
    for period in periods:
        for cs in CURRENT_SYSTEMS:
            for season in SEASONS:
                speed = cs["base_speed"] * (1 + random.gauss(0, 0.1))
                direction = cs["base_dir"] + random.gauss(0, 10)

                region_data = next(r for r in OCEAN_REGIONS if r["name"] == cs["region"])
                b = region_data["bounds"]
                mid_lat = (b[0] + b[2]) / 2
                mid_lon = (b[1] + b[3]) / 2
                dlat = (b[2] - b[0]) * 0.3
                dlon = (b[3] - b[1]) * 0.3

                rad = math.radians(direction)
                end_lat = mid_lat + dlat * math.cos(rad)
                end_lon = mid_lon + dlon * math.sin(rad)

                linestring = f"SRID=4326;LINESTRING({mid_lon} {mid_lat},{end_lon} {end_lat})"

                currents.append({
                    "name": cs["name"],
                    "period_id": period["id"],
                    "season": season,
                    "direction_deg": round(direction % 360, 2),
                    "speed_knots": round(max(0.1, speed), 2),
                    "geom": linestring,
                })
    return currents

def insert_climate_periods(cur, periods):
    sql = """INSERT INTO climate_periods
        (id, period_start, period_end, avg_temperature, avg_wind_speed,
         avg_rainfall, storm_frequency, nao_index, description)
        VALUES %s"""
    values = [(
        p["id"], p["period_start"], p["period_end"],
        p["avg_temperature"], p["avg_wind_speed"],
        p["avg_rainfall"], p["storm_frequency"], p["nao_index"], p["description"]
    ) for p in periods]
    execute_values(cur, sql, values)

def insert_wind_fields(cur, fields):
    sql = """INSERT INTO wind_fields
        (period_id, season, region, avg_direction_deg, avg_speed_knots, variability, geom)
        VALUES %s"""
    values = [(
        f["period_id"], f["season"], f["region"],
        f["avg_direction_deg"], f["avg_speed_knots"], f["variability"], f["geom"]
    ) for f in fields]
    execute_values(cur, sql, values, page_size=500)

def insert_ocean_currents(cur, currents):
    sql = """INSERT INTO ocean_currents
        (name, period_id, season, direction_deg, speed_knots, geom)
        VALUES %s"""
    values = [(
        c["name"], c["period_id"], c["season"],
        c["direction_deg"], c["speed_knots"], c["geom"]
    ) for c in currents]
    execute_values(cur, sql, values, page_size=500)

def export_json(data, filepath):
    export_data = []
    for item in data:
        d = dict(item)
        if "geom" in d:
            del d["geom"]
        export_data.append(d)
    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(export_data, f, ensure_ascii=False, indent=2)

def main():
    random.seed(42)
    periods = generate_climate_periods()
    wind_fields = generate_wind_fields(periods)
    ocean_currents = generate_ocean_currents(periods)

    export_json(periods, "scripts/climate_periods.json")
    export_json(wind_fields, "scripts/wind_fields.json")
    export_json(ocean_currents, "scripts/ocean_currents.json")

    print(f"Climate periods: {len(periods)}")
    print(f"Wind fields: {len(wind_fields)}")
    print(f"Ocean currents: {len(ocean_currents)}")

    if HAS_PG:
        try:
            conn = psycopg2.connect(
                host="localhost", port=5432, dbname="ancient_maritime",
                user="postgres", password="postgres"
            )
            cur = conn.cursor()
            insert_climate_periods(cur, periods)
            print("Climate periods inserted.")
            insert_wind_fields(cur, wind_fields)
            print("Wind fields inserted.")
            insert_ocean_currents(cur, ocean_currents)
            print("Ocean currents inserted.")
            conn.commit()
            cur.close()
            conn.close()
            print("All climate data successfully inserted into database.")
        except Exception as e:
            print(f"Database connection failed: {e}")
            print("Data exported to JSON files only.")
    else:
        print("psycopg2 not installed. Data exported to JSON files only.")

if __name__ == "__main__":
    main()
