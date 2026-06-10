import json
import random
import math
try:
    import psycopg2
    from psycopg2.extras import execute_values
    HAS_PG = True
except ImportError:
    HAS_PG = False

PORTS = [
    (1, "Alexandria", "亚历山大港", "Mediterranean", 31.2001, 29.9187),
    (2, "Carthage", "迦太基", "Mediterranean", 36.8489, 10.3263),
    (3, "Rome_Ostia", "奥斯提亚", "Mediterranean", 41.7333, 12.2833),
    (4, "Athens_Piraeus", "比雷埃夫斯", "Mediterranean", 37.9394, 23.6436),
    (5, "Constantinople", "君士坦丁堡", "Mediterranean", 41.0082, 28.9784),
    (6, "Tyre", "推罗", "Mediterranean", 33.2711, 35.2033),
    (7, "Byblos", "比布鲁斯", "Mediterranean", 34.1214, 35.6497),
    (8, "Rhodes", "罗德岛", "Mediterranean", 36.4341, 28.2176),
    (9, "Crete_Knossos", "克里特-克诺索斯", "Mediterranean", 35.2956, 25.1639),
    (10, "Syracuse", "叙拉古", "Mediterranean", 37.0755, 15.2861),
    (11, "Massalia", "马西利亚", "Mediterranean", 43.2965, 5.3698),
    (12, "Gades", "加的斯", "Mediterranean", 36.5298, -6.2926),
    (13, "Mogador", "莫加多尔", "Atlantic", 31.5085, -9.7696),
    (14, "Mombasa", "蒙巴萨", "Indian Ocean", -4.0500, 39.6667),
    (15, "Kilwa", "基尔瓦", "Indian Ocean", -8.9400, 39.5067),
    (16, "Aden", "亚丁", "Indian Ocean", 12.7794, 45.0367),
    (17, "Muziris", "穆吉里斯", "Indian Ocean", 10.1593, 76.2214),
    (18, "Barigaza", "婆卢羯车", "Indian Ocean", 21.7645, 72.1538),
    (19, "Tamralipti", "耽摩栗底", "Indian Ocean", 21.8912, 87.7886),
    (20, "Gwangju_Beopseongpo", "法圣浦", "East Asia", 35.1269, 126.8167),
    (21, "Quanzhou", "泉州", "East Asia", 24.8741, 118.6758),
    (22, "Guangzhou", "广州", "East Asia", 23.1291, 113.2644),
    (23, "Calicut", "卡利卡特", "Indian Ocean", 11.2588, 75.7804),
    (24, "Hormuz", "霍尔木兹", "Indian Ocean", 27.0461, 56.4611),
    (25, "Basra", "巴士拉", "Indian Ocean", 30.5028, 47.8275),
    (26, "Jeddah", "吉达", "Red Sea", 21.5433, 39.1728),
    (27, "Aila", "埃拉", "Red Sea", 29.5234, 34.9739),
    (28, "Berenice", "贝雷尼塞", "Red Sea", 23.9117, 35.4794),
    (29, "Malacca", "马六甲", "East Asia", 2.1896, 102.2501),
    (30, "Srivijaya", "室利佛逝", "East Asia", -2.9500, 104.7500),
    (31, "Zanzibar", "桑给巴尔", "Indian Ocean", -6.1659, 39.1989),
    (32, "Sofala", "索法拉", "Indian Ocean", -20.1500, 34.7500),
    (33, "Canton_Han", "广州(汉)", "East Asia", 23.1291, 113.2644),
    (34, "Nagasaki", "长崎", "East Asia", 32.7503, 129.8779),
    (35, "Lisbon", "里斯本", "Atlantic", 38.7223, -9.1393),
    (36, "Bristol", "布里斯托尔", "Atlantic", 51.4545, -2.5879),
    (37, "Venice", "威尼斯", "Mediterranean", 45.4408, 12.3155),
    (38, "Genoa", "热那亚", "Mediterranean", 44.4056, 8.9463),
    (39, "Hamburg", "汉堡", "Atlantic", 53.5511, 9.9937),
    (40, "Bergen", "卑尔根", "Atlantic", 60.3913, 5.3221),
]

SEASONS = ["spring", "summer", "autumn", "winter"]
SHIP_TYPES = ["trireme", "merchant_round_ship", "dhow", "junk", "carrack", "longship", "galley", "treasure_ship"]
CARGO_TYPES = ["grain", "olive_oil", "wine", "spices", "silk", "ceramics", "ivory", "gold", "timber", "salt", "textiles", "glass", "incense", "precious_stones", "copper"]

PORT_ALIASES = [
    (1, "Alexandria", "亚历山大", None, None, "Greek", "Strabo"),
    (1, "Raqote", "拉科特", -1000, -300, "Egyptian", "Egyptian records"),
    (1, "Iskandariyya", "伊斯坎达利亚", 600, 1800, "Arabic", "Arabic geographers"),
    (2, "Carthago", "迦太基(拉丁)", -800, 150, "Latin", "Roman records"),
    (2, "Kart-hadasht", "卡尔特-哈达什特", -800, -100, "Punic", "Punic inscriptions"),
    (3, "Portus", "波尔图", -100, 500, "Latin", "Roman records"),
    (3, "Ostia", "奥斯提亚(古)", -400, 100, "Latin", "Roman records"),
    (4, "Piraeus", "比雷埃夫斯(古)", -500, 200, "Greek", "Greek records"),
    (4, "Kantharos", "坎塔罗斯", -500, -200, "Greek", "Thucydides"),
    (5, "Byzantium", "拜占庭", -600, 330, "Greek", "Greek sources"),
    (5, "Byzantion", "拜占庭(希腊)", -600, 330, "Greek", "Greek sources"),
    (5, "Istanbul", "伊斯坦布尔", 1453, 1800, "Turkish", "Ottoman records"),
    (5, "Konstantinoupoli", "君士坦丁堡(希腊)", 330, 1453, "Greek", "Byzantine records"),
    (6, "Tsor", "推罗(希伯来)", -1000, -300, "Hebrew", "Biblical records"),
    (6, "Sur", "苏尔", 600, 1800, "Arabic", "Arabic geographers"),
    (7, "Gubla", "古布拉", -2000, -500, "Phoenician", "Phoenician records"),
    (7, "Jbeil", "朱拜勒", 600, 1800, "Arabic", "Arabic geographers"),
    (9, "Knossos", "克诺索斯", -2000, -1000, "Greek", "Minoan records"),
    (9, "Kaptara", "卡普塔拉", -2000, -1400, "Minoan", "Egyptian records"),
    (10, "Syrakousai", "叙拉古(希腊)", -700, 200, "Greek", "Greek records"),
    (10, "Siracusa", "锡拉库萨", 200, 1800, "Italian", "Italian records"),
    (11, "Massalia", "马西利亚(希腊)", -600, 0, "Greek", "Greek records"),
    (11, "Marseille", "马赛", 0, 1800, "French", "French records"),
    (12, "Gadir", "加的尔", -1000, -200, "Phoenician", "Phoenician records"),
    (12, "Gades", "加的斯(拉丁)", -200, 500, "Latin", "Roman records"),
    (17, "Muziris", "穆吉里斯(古)", -100, 500, "Tamil", "Sangam literature"),
    (17, "Muchiri", "穆奇里", -100, 500, "Tamil", "Sangam literature"),
    (18, "Barygaza", "婆卢羯车(古)", -100, 500, "Sanskrit", "Periplus"),
    (19, "Tamralipti", "耽摩栗底(古)", -300, 800, "Sanskrit", "Indian records"),
    (21, "Zaiton", "刺桐", 1000, 1400, "Arabic", "Ibn Battuta"),
    (21, "Citong", "刺桐(元)", 1200, 1400, "Chinese", "Yuan Dynasty records"),
    (22, "Canton", "广州(英)", 1500, 1800, "English", "British East India"),
    (22, "Khanfu", "广府", 700, 1000, "Arabic", "Arabic geographers"),
    (23, "Kozhikode", "科泽科德", 1200, 1800, "Malayalam", "Kerala records"),
    (24, "Hormuz", "霍尔木兹(新)", 1300, 1800, "Persian", "Persian records"),
    (24, "Ormus", "奥尔穆兹", 1500, 1700, "Portuguese", "Portuguese records"),
    (25, "Basra", "巴士拉(古)", 600, 1800, "Arabic", "Arabic geographers"),
    (25, "Ubullah", "乌布拉", 600, 1000, "Arabic", "Arabic geographers"),
    (29, "Melaka", "马六甲(马来)", 1400, 1800, "Malay", "Malay Annals"),
    (30, "Palembang", "巨港", 600, 1400, "Malay", "Srivijaya records"),
    (33, "Panyu", "番禺", -200, 600, "Chinese", "Chinese records"),
    (35, "Olisipo", "奥利西波", -200, 500, "Latin", "Roman records"),
    (35, "Lishbuna", "里斯本(阿拉伯)", 700, 1100, "Arabic", "Arabic geographers"),
    (37, "Venezia", "威尼斯(意)", 800, 1800, "Italian", "Venetian records"),
]

STORM_PRONE_SEASONS = {"autumn": 0.25, "winter": 0.30, "spring": 0.12, "summer": 0.08}

REGION_CONNECTIONS = {
    "Mediterranean": ["Mediterranean", "Atlantic", "Red Sea", "Indian Ocean"],
    "Atlantic": ["Mediterranean", "Atlantic"],
    "Red Sea": ["Mediterranean", "Indian Ocean", "Red Sea"],
    "Indian Ocean": ["Red Sea", "Indian Ocean", "East Asia"],
    "East Asia": ["Indian Ocean", "East Asia"],
}

def generate_route_points(lat1, lon1, lat2, lon2, num_points=8):
    points = []
    for i in range(num_points + 1):
        t = i / num_points
        lat = lat1 + (lat2 - lat1) * t
        lon = lon1 + (lon2 - lon1) * t
        offset_lat = random.gauss(0, 0.5) * math.sin(t * math.pi)
        offset_lon = random.gauss(0, 0.5) * math.sin(t * math.pi)
        if 0 < t < 1:
            lat += offset_lat
            lon += offset_lon
        points.append([round(lon, 4), round(lat, 4)])
    return points

def get_connected_ports(port, all_ports):
    port_region = port[3]
    connected_regions = REGION_CONNECTIONS.get(port_region, [port_region])
    connected = [p for p in all_ports if p[3] in connected_regions and p[0] != port[0]]
    return connected

def determine_storm(season, region, year):
    base_prob = STORM_PRONE_SEASONS.get(season, 0.10)
    if region in ["Atlantic"]:
        base_prob *= 1.3
    if year < 0:
        base_prob *= 0.9
    if 1300 <= year <= 1800:
        base_prob *= 1.15
    return random.random() < base_prob

def pick_ship_and_cargo(year, region):
    if year < 0:
        ships = ["trireme", "galley", "dhow"]
    elif year < 500:
        ships = ["galley", "dhow", "merchant_round_ship"]
    elif year < 1000:
        ships = ["dhow", "merchant_round_ship", "junk"]
    elif year < 1400:
        ships = ["dhow", "junk", "carrack", "merchant_round_ship"]
    else:
        ships = ["carrack", "junk", "treasure_ship", "merchant_round_ship"]

    if region == "East Asia":
        if "junk" in ships:
            ships = ["junk"] * 3 + ships
    elif region == "Indian Ocean":
        if "dhow" in ships:
            ships = ["dhow"] * 3 + ships

    ship = random.choice(ships)

    if region == "East Asia":
        cargo_pool = ["silk", "ceramics", "spices", "tea", "textiles"]
    elif region == "Indian Ocean":
        cargo_pool = ["spices", "ivory", "incense", "precious_stones", "textiles"]
    elif region == "Mediterranean":
        cargo_pool = ["olive_oil", "wine", "grain", "glass", "ceramics"]
    else:
        cargo_pool = ["timber", "salt", "copper", "grain", "textiles"]

    cargo = random.choice(cargo_pool + CARGO_TYPES[:5])
    return ship, cargo

def generate_voyages(num_records=1500):
    voyages = []
    for _ in range(num_records):
        port = random.choice(PORTS)
        connected = get_connected_ports(port, PORTS)
        if not connected:
            continue
        dest = random.choice(connected)
        year = random.randint(-1000, 1800)
        season = random.choice(SEASONS)
        ship, cargo = pick_ship_and_cargo(year, port[3])
        storm = determine_storm(season, port[3], year)
        route = generate_route_points(port[4], port[5], dest[4], dest[5])
        voyages.append({
            "departure_port_id": port[0],
            "arrival_port_id": dest[0],
            "voyage_year": year,
            "season": season,
            "ship_type": ship,
            "cargo_type": cargo,
            "encountered_storm": storm,
            "route_points": route,
            "departure_name": port[1],
            "arrival_name": dest[1],
            "departure_coords": [port[5], port[4]],
            "arrival_coords": [dest[5], dest[4]],
        })
    return voyages

def insert_ports(cur):
    sql = "INSERT INTO ports (id, name, name_zh, region, geom) VALUES %s"
    values = []
    for p in PORTS:
        geom = f"SRID=4326;POINT({p[5]} {p[4]})"
        values.append((p[0], p[1], p[2], p[3], geom))
    execute_values(cur, sql, values)

def insert_port_aliases(cur):
    sql = """INSERT INTO port_aliases
        (port_id, alias_name, alias_name_zh, period_start, period_end, language, source)
        VALUES %s"""
    values = []
    for a in PORT_ALIASES:
        values.append((a[0], a[1], a[2], a[3], a[4], a[5], a[6]))
    execute_values(cur, sql, values)

def insert_voyages(cur, voyages):
    sql = """INSERT INTO voyage_records
        (departure_port_id, arrival_port_id, voyage_year, season, ship_type, cargo_type,
         encountered_storm, route_geom, route_points)
        VALUES %s"""
    values = []
    for v in voyages:
        linestring = "SRID=4326;LINESTRING(" + ",".join(f"{p[0]} {p[1]}" for p in v["route_points"]) + ")"
        route_json = json.dumps(v["route_points"])
        values.append((
            v["departure_port_id"], v["arrival_port_id"], v["voyage_year"],
            v["season"], v["ship_type"], v["cargo_type"],
            v["encountered_storm"], linestring, route_json
        ))
    execute_values(cur, sql, values)

def export_voyages_json(voyages, filepath):
    ports_dict = {p[0]: {"name": p[1], "name_zh": p[2], "lat": p[4], "lon": p[5]} for p in PORTS}
    export_data = []
    for v in voyages:
        export_data.append({
            "id": voyages.index(v) + 1,
            "departure_port": v["departure_name"],
            "departure_port_zh": ports_dict[v["departure_port_id"]]["name_zh"],
            "arrival_port": v["arrival_name"],
            "arrival_port_zh": ports_dict[v["arrival_port_id"]]["name_zh"],
            "voyage_year": v["voyage_year"],
            "season": v["season"],
            "ship_type": v["ship_type"],
            "cargo_type": v["cargo_type"],
            "encountered_storm": v["encountered_storm"],
            "route_points": v["route_points"],
        })
    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(export_data, f, ensure_ascii=False, indent=2)

def export_ports_json(filepath):
    data = []
    for p in PORTS:
        data.append({
            "id": p[0], "name": p[1], "name_zh": p[2],
            "region": p[3], "lat": p[4], "lon": p[5]
        })
    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

def main():
    random.seed(42)
    voyages = generate_voyages(1500)

    export_ports_json("scripts/ports.json")
    export_voyages_json(voyages, "scripts/voyages.json")

    if HAS_PG:
        try:
            conn = psycopg2.connect(
                host="localhost", port=5432, dbname="ancient_maritime",
                user="postgres", password="postgres"
            )
            cur = conn.cursor()
            insert_ports(cur)
            insert_port_aliases(cur)
            insert_voyages(cur, voyages)
            conn.commit()
            cur.close()
            conn.close()
            print(f"Successfully inserted {len(voyages)} voyage records into database.")
        except Exception as e:
            print(f"Database connection failed: {e}")
            print("Data exported to JSON files only.")
    else:
        print("psycopg2 not installed. Data exported to JSON files only.")

    storm_count = sum(1 for v in voyages if v["encountered_storm"])
    print(f"Total voyages: {len(voyages)}")
    print(f"Storm encounters: {storm_count} ({storm_count/len(voyages)*100:.1f}%)")

if __name__ == "__main__":
    main()
