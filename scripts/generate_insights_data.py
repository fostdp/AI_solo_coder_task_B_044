#!/usr/bin/env python3
"""
生成古代航海贸易扩展数据：
1. 历史事件（战争、政权更迭、瘟疫、重大航海事件）
2. 港口年度贸易流量面板数据
3. 技术扩散路径
4. 现代船舶AIS模拟数据
5. 现代气象预报模拟数据
"""

import psycopg2
import random
import math
import json
from datetime import date, timedelta

DB_CONFIG = {
    'host': 'postgres',
    'database': 'ancient_maritime',
    'user': 'postgres',
    'password': 'postgres',
    'port': 5432
}

HISTORICAL_EVENTS = [
    # 战争事件
    {'name': 'Trojan War', 'name_zh': '特洛伊战争', 'type': 'war', 'region': 'mediterranean',
     'start': -1184, 'end': -1184, 'severity': 0.9, 'lat': 39.7, 'lon': 26.2,
     'desc': '希腊城邦与特洛伊之间的战争，影响爱琴海贸易', 'source': '荷马史诗'},
    {'name': 'Persian Wars', 'name_zh': '希波战争', 'type': 'war', 'region': 'mediterranean',
     'start': -499, 'end': -449, 'severity': 0.8, 'lat': 37.8, 'lon': 27.0,
     'desc': '希腊城邦与波斯帝国的战争，中断爱琴海贸易', 'source': '希罗多德历史'},
    {'name': 'Peloponnesian War', 'name_zh': '伯罗奔尼撒战争', 'type': 'war', 'region': 'mediterranean',
     'start': -431, 'end': -404, 'severity': 0.85, 'lat': 37.6, 'lon': 23.7,
     'desc': '雅典与斯巴达的战争，导致希腊城邦衰落', 'source': '修昔底德'},
    {'name': 'Punic Wars', 'name_zh': '布匿战争', 'type': 'war', 'region': 'mediterranean',
     'start': -264, 'end': -146, 'severity': 0.9, 'lat': 36.8, 'lon': 10.2,
     'desc': '罗马与迦太基的三次战争，罗马控制西地中海', 'source': '李维罗马史'},
    {'name': 'Mithridatic Wars', 'name_zh': '米特拉达梯战争', 'type': 'war', 'region': 'black_sea',
     'start': -88, 'end': -63, 'severity': 0.7, 'lat': 41.0, 'lon': 30.0,
     'desc': '本都王国与罗马的战争，影响黑海贸易', 'source': '阿庇安'},
    {'name': 'Roman Civil Wars', 'name_zh': '罗马内战', 'type': 'war', 'region': 'mediterranean',
     'start': -49, 'end': -31, 'severity': 0.85, 'lat': 41.9, 'lon': 12.5,
     'desc': '共和国末期内战，终结于阿克提姆海战', 'source': '普鲁塔克'},
    {'name': 'Vandal Invasions', 'name_zh': '汪达尔人入侵', 'type': 'war', 'region': 'mediterranean',
     'start': 429, 'end': 476, 'severity': 0.8, 'lat': 36.8, 'lon': 10.2,
     'desc': '汪达尔人占领北非，破坏地中海贸易', 'source': '普洛科皮乌斯'},
    {'name': 'Arab Conquests', 'name_zh': '阿拉伯征服', 'type': 'war', 'region': 'indian_ocean',
     'start': 632, 'end': 750, 'severity': 0.75, 'lat': 21.5, 'lon': 39.2,
     'desc': '伊斯兰扩张，控制红海与波斯湾贸易', 'source': '塔巴里年代记'},
    {'name': 'Crusades', 'name_zh': '十字军东征', 'type': 'war', 'region': 'mediterranean',
     'start': 1096, 'end': 1291, 'severity': 0.7, 'lat': 31.8, 'lon': 35.2,
     'desc': '十字军与穆斯林的战争，刺激地中海贸易', 'source': '多个史料'},
    {'name': 'Mongol Conquests', 'name_zh': '蒙古西征', 'type': 'war', 'region': 'asia',
     'start': 1206, 'end': 1279, 'severity': 0.7, 'lat': 34.0, 'lon': 108.0,
     'desc': '蒙古帝国扩张初期破坏，之后实现蒙古和平', 'source': '史集'},
    {'name': 'Hundred Years War', 'name_zh': '百年战争', 'type': 'war', 'region': 'atlantic',
     'start': 1337, 'end': 1453, 'severity': 0.65, 'lat': 48.9, 'lon': 2.3,
     'desc': '英法百年战争，影响大西洋沿岸贸易', 'source': '傅华萨编年史'},
    {'name': 'Ottoman Conquest of Constantinople', 'name_zh': '奥斯曼攻陷君士坦丁堡', 'type': 'war',
     'region': 'black_sea', 'start': 1453, 'end': 1453, 'severity': 0.85, 'lat': 41.0, 'lon': 29.0,
     'desc': '拜占庭灭亡，传统丝绸之路受阻，刺激大航海', 'source': '克利托布罗斯'},

    # 政权更迭
    {'name': 'Founding of Rome Republic', 'name_zh': '罗马共和国建立', 'type': 'regime_change',
     'region': 'mediterranean', 'start': -509, 'end': -509, 'severity': 0.6, 'lat': 41.9, 'lon': 12.5,
     'desc': '罗马推翻王政建立共和国，开始地中海扩张', 'source': '李维'},
    {'name': 'Han Dynasty Silk Road', 'name_zh': '汉代丝绸之路开辟', 'type': 'regime_change',
     'region': 'east_asia', 'start': -138, 'end': -126, 'severity': 0.7, 'lat': 34.3, 'lon': 108.9,
     'desc': '张骞出使西域，丝路贸易兴盛', 'source': '史记'},
    {'name': 'Roman Empire Augustan Peace', 'name_zh': '罗马帝国奥古斯都和平', 'type': 'regime_change',
     'region': 'mediterranean', 'start': -27, 'end': 180, 'severity': -0.5, 'lat': 41.9, 'lon': 12.5,
     'desc': '罗马和平时期，地中海贸易高度繁荣', 'source': '塔西佗'},
    {'name': 'Fall of Western Roman Empire', 'name_zh': '西罗马帝国灭亡', 'type': 'regime_change',
     'region': 'mediterranean', 'start': 476, 'end': 476, 'severity': 0.9, 'lat': 42.0, 'lon': 12.5,
     'desc': '西罗马灭亡，地中海贸易体系瓦解', 'source': '多个史料'},
    {'name': 'Tang Dynasty Maritime Trade', 'name_zh': '唐代海上贸易兴盛', 'type': 'regime_change',
     'region': 'east_asia', 'start': 618, 'end': 907, 'severity': -0.6, 'lat': 34.3, 'lon': 108.9,
     'desc': '唐代广州、泉州成为国际贸易大港', 'source': '旧唐书'},
    {'name': 'Song Dynasty Maritime Peak', 'name_zh': '宋代航海鼎盛', 'type': 'regime_change',
     'region': 'east_asia', 'start': 960, 'end': 1279, 'severity': -0.7, 'lat': 30.3, 'lon': 120.2,
     'desc': '宋代指南针应用，海上贸易达到高峰', 'source': '宋史'},
    {'name': 'Mongol Peace Period', 'name_zh': '蒙古和平时期', 'type': 'regime_change',
     'region': 'asia', 'start': 1271, 'end': 1368, 'severity': -0.5, 'lat': 39.9, 'lon': 116.4,
     'desc': '元朝统一，欧亚大陆贸易畅通', 'source': '马可波罗游记'},
    {'name': 'Ming Dynasty Maritime Policy', 'name_zh': '明代海禁与开海', 'type': 'regime_change',
     'region': 'east_asia', 'start': 1371, 'end': 1567, 'severity': 0.6, 'lat': 32.0, 'lon': 118.8,
     'desc': '明初海禁政策，后期隆庆开关', 'source': '明史'},
    {'name': 'Zheng He Voyages', 'name_zh': '郑和下西洋', 'type': 'regime_change',
     'region': 'indian_ocean', 'start': 1405, 'end': 1433, 'severity': -0.6, 'lat': 32.0, 'lon': 118.8,
     'desc': '明朝郑和七次下西洋，影响印度洋贸易', 'source': '瀛涯胜览'},
    {'name': 'Portuguese Age of Discovery', 'name_zh': '葡萄牙大航海时代', 'type': 'regime_change',
     'region': 'indian_ocean', 'start': 1488, 'end': 1580, 'severity': -0.5, 'lat': 38.7, 'lon': -9.1,
     'desc': '迪亚士、达伽马开辟新航路，葡萄牙控制印度洋', 'source': '多个史料'},
    {'name': 'Spanish Colonial Empire', 'name_zh': '西班牙殖民帝国', 'type': 'regime_change',
     'region': 'atlantic', 'start': 1492, 'end': 1598, 'severity': -0.6, 'lat': 40.4, 'lon': -3.7,
     'desc': '哥伦布发现美洲，西班牙建立殖民贸易帝国', 'source': '多个史料'},
    {'name': 'Dutch Golden Age', 'name_zh': '荷兰黄金时代', 'type': 'regime_change',
     'region': 'atlantic', 'start': 1588, 'end': 1672, 'severity': -0.7, 'lat': 52.4, 'lon': 4.9,
     'desc': '荷兰成为海上马车夫，建立全球贸易网络', 'source': '多个史料'},

    # 瘟疫
    {'name': 'Justinian Plague', 'name_zh': '查士丁尼瘟疫', 'type': 'plague',
     'region': 'mediterranean', 'start': 541, 'end': 542, 'severity': 0.85, 'lat': 41.0, 'lon': 29.0,
     'desc': '东罗马帝国大瘟疫，人口锐减影响贸易', 'source': '普洛科皮乌斯'},
    {'name': 'Black Death', 'name_zh': '黑死病', 'type': 'plague',
     'region': 'mediterranean', 'start': 1347, 'end': 1351, 'severity': 0.9, 'lat': 43.0, 'lon': 11.0,
     'desc': '欧洲黑死病，人口减少1/3，经济剧烈变化', 'source': '薄伽丘十日谈'},

    # 重大航海事件
    {'name': 'Phoenician Circumnavigation of Africa', 'name_zh': '腓尼基环航非洲', 'type': 'expedition',
     'region': 'atlantic', 'start': -600, 'end': -600, 'severity': -0.4, 'lat': 31.2, 'lon': 29.9,
     'desc': '法老尼科派腓尼基人环航非洲，证实海洋连通', 'source': '希罗多德'},
    {'name': 'Vasco da Gama to India', 'name_zh': '达伽马到达印度', 'type': 'expedition',
     'region': 'indian_ocean', 'start': 1497, 'end': 1499, 'severity': -0.6, 'lat': 38.7, 'lon': -9.1,
     'desc': '葡萄牙航海家达伽马开辟欧亚新航路', 'source': '卡蒙斯卢济塔尼亚人之歌'},
    {'name': 'Magellan Circumnavigation', 'name_zh': '麦哲伦环球航行', 'type': 'expedition',
     'region': 'pacific', 'start': 1519, 'end': 1522, 'severity': -0.5, 'lat': -33.9, 'lon': -71.6,
     'desc': '麦哲伦船队首次环球航行，证明地球圆形', 'source': '皮加费塔航行记'},

    # 技术事件
    {'name': 'Mariner Compass Invention', 'name_zh': '航海指南针发明', 'type': 'technology',
     'region': 'east_asia', 'start': 1040, 'end': 1100, 'severity': -0.7, 'lat': 30.3, 'lon': 120.2,
     'desc': '宋代将指南针应用于航海，革命性提升航行能力', 'source': '萍洲可谈'},
    {'name': 'Lateen Sail Adoption', 'name_zh': '三角帆普及', 'type': 'technology',
     'region': 'mediterranean', 'start': 200, 'end': 800, 'severity': -0.5, 'lat': 36.8, 'lon': 10.2,
     'desc': '三角帆在地中海和印度洋普及，提升逆风航行能力', 'source': '多个史料'},
    {'name': 'Astrolabe Marine Use', 'name_zh': '航海星盘应用', 'type': 'technology',
     'region': 'mediterranean', 'start': 900, 'end': 1100, 'severity': -0.4, 'lat': 36.8, 'lon': 10.2,
     'desc': '阿拉伯天文学家将星盘用于航海纬度测量', 'source': '比鲁尼著作'},
    {'name': 'Caravel Ship Design', 'name_zh': '卡拉维尔帆船设计', 'type': 'technology',
     'region': 'atlantic', 'start': 1440, 'end': 1480, 'severity': -0.5, 'lat': 38.7, 'lon': -9.1,
     'desc': '葡萄牙发展卡拉维尔帆船，适宜远洋探险', 'source': '多个史料'},
    {'name': 'Portolan Charts', 'name_zh': '波托兰航海图', 'type': 'technology',
     'region': 'mediterranean', 'start': 1290, 'end': 1350, 'severity': -0.4, 'lat': 43.0, 'lon': 11.0,
     'desc': '精确航海图出现，地中海航行精度大幅提升', 'source': '维斯康特航海图'},
]

TECH_DIFFUSION_DATA = [
    {'name': 'Iron Smelting', 'name_zh': '冶铁技术', 'category': 'metallurgy',
     'origin_ports': ['Tyre', 'Sidon'], 'start_year': -1500, 'end_year': -500,
     'speed': 2.5, 'desc': '从黎凡特向地中海世界传播的冶铁技术'},
    {'name': 'Porcelain Making', 'name_zh': '瓷器制造', 'category': 'ceramics',
     'origin_ports': ['Guangzhou', 'Quanzhou'], 'start_year': 600, 'end_year': 1500,
     'speed': 1.8, 'desc': '从中国南方港口向世界传播的制瓷技术'},
    {'name': 'Shipbuilding Tech', 'name_zh': '造船技术', 'category': 'maritime',
     'origin_ports': ['Corinth', 'Alexandria'], 'start_year': -1000, 'end_year': 1400,
     'speed': 3.0, 'desc': '从地中海向全球扩散的先进造船技术'},
    {'name': 'Navigation Science', 'name_zh': '航海术', 'category': 'navigation',
     'origin_ports': ['Sohar', 'Aden'], 'start_year': 800, 'end_year': 1450,
     'speed': 4.0, 'desc': '阿拉伯航海家发展的天文导航与季风航海知识'},
    {'name': 'Papermaking', 'name_zh': '造纸术', 'category': 'technology',
     'origin_ports': ['Guangzhou'], 'start_year': 200, 'end_year': 1200,
     'speed': 1.5, 'desc': '从中国经由海上丝绸之路传播的造纸技术'},
    {'name': 'Coinage System', 'name_zh': '铸币技术', 'category': 'economy',
     'origin_ports': ['Ephesus'], 'start_year': -700, 'end_year': -300,
     'speed': 2.0, 'desc': '从吕底亚发源的标准化铸币制度'},
    {'name': 'Sternpost Rudder', 'name_zh': '尾舵技术', 'category': 'maritime',
     'origin_ports': ['Guangzhou', 'Hangzhou'], 'start_year': 150, 'end_year': 1100,
     'speed': 1.2, 'desc': '中国发明的尾舵大幅提升船舶操纵性'},
    {'name': 'Waterproof Caulking', 'name_zh': '水密隔舱', 'category': 'maritime',
     'origin_ports': ['Quanzhou'], 'start_year': 300, 'end_year': 1300,
     'speed': 1.0, 'desc': '中国古代造船的水密舱壁技术'},
]

MODERN_SHIPS = [
    {'name': 'MSC Gülsün', 'mmsi': '477965100', 'type': 'container',
     'gross_tonnage': 232618, 'length': 399.9, 'beam': 61.5, 'max_speed': 22.5,
     'flag': 'Panama', 'home_port': 'Shanghai'},
    {'name': 'Prelude FLNG', 'mmsi': '477965200', 'type': 'LNG',
     'gross_tonnage': 674000, 'length': 488.0, 'beam': 74.0, 'max_speed': 18.0,
     'flag': 'Bahamas', 'home_port': 'Singapore'},
    {'name': 'TI Europe', 'mmsi': '538003440', 'type': 'tanker',
     'gross_tonnage': 234006, 'length': 380.0, 'beam': 68.0, 'max_speed': 16.5,
     'flag': 'Marshall Islands', 'home_port': 'Dubai'},
    {'name': 'Diamond Princess', 'mmsi': '477965300', 'type': 'cruise',
     'gross_tonnage': 115875, 'length': 290.0, 'beam': 37.5, 'max_speed': 24.0,
     'flag': 'Bermuda', 'home_port': 'Singapore'},
    {'name': 'HMM Algeciras', 'mmsi': '477965400', 'type': 'container',
     'gross_tonnage': 228283, 'length': 399.9, 'beam': 61.0, 'max_speed': 22.0,
     'flag': 'Panama', 'home_port': 'Busan'},
    {'name': 'Ever Given', 'mmsi': '477965500', 'type': 'container',
     'gross_tonnage': 224000, 'length': 400.0, 'beam': 59.0, 'max_speed': 22.5,
     'flag': 'Panama', 'home_port': 'Kaohsiung'},
    {'name': 'MV Berge Istra', 'mmsi': '477965600', 'type': 'bulk_carrier',
     'gross_tonnage': 159534, 'length': 340.0, 'beam': 57.0, 'max_speed': 15.5,
     'flag': 'Hong Kong', 'home_port': 'Hong Kong'},
    {'name': 'Symphony of the Seas', 'mmsi': '477965700', 'type': 'cruise',
     'gross_tonnage': 228081, 'length': 361.0, 'beam': 66.0, 'max_speed': 22.0,
     'flag': 'Bahamas', 'home_port': 'Miami'},
    {'name': 'CSCL Globe', 'mmsi': '477965800', 'type': 'container',
     'gross_tonnage': 184605, 'length': 400.0, 'beam': 59.0, 'max_speed': 20.5,
     'flag': 'Hong Kong', 'home_port': 'Shanghai'},
    {'name': 'Knock Nevis', 'mmsi': '477965900', 'type': 'tanker',
     'gross_tonnage': 260941, 'length': 458.45, 'beam': 68.8, 'max_speed': 16.0,
     'flag': 'Norway', 'home_port': 'Oslo'},
]

MODERN_WEATHER_REGIONS = [
    {'name': 'Strait of Gibraltar', 'lat': 35.9, 'lon': -5.7,
     'wind_dir': 270, 'wind_speed': 15, 'wave_height': 1.5,
     'current_dir': 90, 'current_speed': 1.5, 'visibility': 10, 'storm_prob': 0.05},
    {'name': 'Bay of Biscay', 'lat': 45.0, 'lon': -5.0,
     'wind_dir': 315, 'wind_speed': 20, 'wave_height': 3.0,
     'current_dir': 180, 'current_speed': 1.0, 'visibility': 8, 'storm_prob': 0.15},
    {'name': 'English Channel', 'lat': 50.5, 'lon': 1.0,
     'wind_dir': 270, 'wind_speed': 18, 'wave_height': 2.0,
     'current_dir': 90, 'current_speed': 2.0, 'visibility': 6, 'storm_prob': 0.12},
    {'name': 'Balearic Sea', 'lat': 39.5, 'lon': 3.0,
     'wind_dir': 180, 'wind_speed': 10, 'wave_height': 0.8,
     'current_dir': 180, 'current_speed': 0.8, 'visibility': 12, 'storm_prob': 0.03},
    {'name': 'Ionian Sea', 'lat': 37.5, 'lon': 20.0,
     'wind_dir': 315, 'wind_speed': 12, 'wave_height': 1.0,
     'current_dir': 270, 'current_speed': 0.6, 'visibility': 11, 'storm_prob': 0.04},
    {'name': 'Levant Sea', 'lat': 33.5, 'lon': 34.0,
     'wind_dir': 270, 'wind_speed': 8, 'wave_height': 0.5,
     'current_dir': 270, 'current_speed': 0.5, 'visibility': 10, 'storm_prob': 0.02},
    {'name': 'Red Sea', 'lat': 21.5, 'lon': 37.5,
     'wind_dir': 315, 'wind_speed': 14, 'wave_height': 0.8,
     'current_dir': 180, 'current_speed': 1.0, 'visibility': 10, 'storm_prob': 0.03},
    {'name': 'Arabian Sea', 'lat': 18.0, 'lon': 62.0,
     'wind_dir': 225, 'wind_speed': 18, 'wave_height': 2.0,
     'current_dir': 180, 'current_speed': 1.5, 'visibility': 9, 'storm_prob': 0.08},
    {'name': 'Bay of Bengal', 'lat': 13.0, 'lon': 88.0,
     'wind_dir': 270, 'wind_speed': 12, 'wave_height': 1.2,
     'current_dir': 270, 'current_speed': 0.8, 'visibility': 8, 'storm_prob': 0.10},
    {'name': 'South China Sea', 'lat': 15.0, 'lon': 112.0,
     'wind_dir': 225, 'wind_speed': 16, 'wave_height': 1.5,
     'current_dir': 180, 'current_speed': 1.2, 'visibility': 7, 'storm_prob': 0.10},
    {'name': 'East China Sea', 'lat': 28.0, 'lon': 124.0,
     'wind_dir': 270, 'wind_speed': 14, 'wave_height': 1.8,
     'current_dir': 270, 'current_speed': 1.0, 'visibility': 6, 'storm_prob': 0.09},
    {'name': 'Philippine Sea', 'lat': 15.0, 'lon': 130.0,
     'wind_dir': 90, 'wind_speed': 20, 'wave_height': 2.5,
     'current_dir': 270, 'current_speed': 2.0, 'visibility': 8, 'storm_prob': 0.15},
    {'name': 'Gulf of Aden', 'lat': 12.5, 'lon': 47.5,
     'wind_dir': 270, 'wind_speed': 16, 'wave_height': 1.0,
     'current_dir': 270, 'current_speed': 1.5, 'visibility': 10, 'storm_prob': 0.04},
    {'name': 'Persian Gulf', 'lat': 26.0, 'lon': 52.5,
     'wind_dir': 135, 'wind_speed': 14, 'wave_height': 0.6,
     'current_dir': 90, 'current_speed': 0.5, 'visibility': 5, 'storm_prob': 0.02},
    {'name': 'Cape of Good Hope', 'lat': -34.0, 'lon': 18.5,
     'wind_dir': 270, 'wind_speed': 25, 'wave_height': 5.0,
     'current_dir': 90, 'current_speed': 2.5, 'visibility': 7, 'storm_prob': 0.25},
    {'name': 'Agulhas Plateau', 'lat': -38.0, 'lon': 25.0,
     'wind_dir': 270, 'wind_speed': 30, 'wave_height': 6.0,
     'current_dir': 90, 'current_speed': 3.0, 'visibility': 5, 'storm_prob': 0.35},
    {'name': 'Canary Current', 'lat': 28.0, 'lon': -15.0,
     'wind_dir': 45, 'wind_speed': 16, 'wave_height': 1.5,
     'current_dir': 180, 'current_speed': 1.5, 'visibility': 10, 'storm_prob': 0.06},
    {'name': 'Gulf Stream', 'lat': 35.0, 'lon': -72.0,
     'wind_dir': 45, 'wind_speed': 18, 'wave_height': 2.0,
     'current_dir': 45, 'current_speed': 3.0, 'visibility': 7, 'storm_prob': 0.12},
    {'name': 'Java Sea', 'lat': -5.0, 'lon': 109.0,
     'wind_dir': 135, 'wind_speed': 10, 'wave_height': 0.5,
     'current_dir': 90, 'current_speed': 0.5, 'visibility': 6, 'storm_prob': 0.03},
    {'name': 'Mozambique Channel', 'lat': -18.0, 'lon': 42.0,
     'wind_dir': 45, 'wind_speed': 12, 'wave_height': 1.5,
     'current_dir': 225, 'current_speed': 1.5, 'visibility': 8, 'storm_prob': 0.07},
]


def connect_db():
    return psycopg2.connect(**DB_CONFIG)


def insert_historical_events(cur):
    print("Inserting historical events...")
    for evt in HISTORICAL_EVENTS:
        cur.execute("""
            INSERT INTO historical_events
            (event_name, event_name_zh, event_type, region, start_year, end_year,
             severity, description, source, geom)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s,
             ST_SetSRID(ST_MakePoint(%s, %s), 4326))
        """, (
            evt['name'], evt['name_zh'], evt['type'], evt['region'],
            evt['start'], evt['end'], evt['severity'],
            evt['desc'], evt['source'],
            evt['lon'], evt['lat']
        ))
    print(f"  Inserted {len(HISTORICAL_EVENTS)} historical events")


def compute_yearly_port_flow(cur):
    print("Computing port yearly flow data...")
    cur.execute("SELECT id FROM ports ORDER BY id")
    port_ids = [row[0] for row in cur.fetchall()]

    total_inserted = 0
    for year in range(-1000, 1801, 10):
        for port_id in port_ids:
            cur.execute("""
                SELECT COUNT(*) FROM voyage_records
                WHERE departure_port_id = %s
                AND voyage_year BETWEEN %s AND %s
            """, (port_id, year - 5, year + 5))
            dep_count = cur.fetchone()[0]

            cur.execute("""
                SELECT COUNT(*) FROM voyage_records
                WHERE arrival_port_id = %s
                AND voyage_year BETWEEN %s AND %s
            """, (port_id, year - 5, year + 5))
            arr_count = cur.fetchone()[0]

            cur.execute("""
                SELECT COUNT(*) FROM voyage_records
                WHERE departure_port_id = %s
                AND encountered_storm = TRUE
                AND voyage_year BETWEEN %s AND %s
            """, (port_id, year - 5, year + 5))
            storm_count = cur.fetchone()[0]

            cur.execute("""
                SELECT COUNT(DISTINCT cargo_type) FROM voyage_records
                WHERE departure_port_id = %s
                AND voyage_year BETWEEN %s AND %s
            """, (port_id, year - 5, year + 5))
            cargo_types = cur.fetchone()[0]

            cur.execute("""
                SELECT COUNT(DISTINCT arrival_port_id) FROM voyage_records
                WHERE departure_port_id = %s
                AND voyage_year BETWEEN %s AND %s
            """, (port_id, year - 5, year + 5))
            destinations = cur.fetchone()[0]

            total_flow = dep_count + arr_count
            storm_rate = storm_count / total_flow if total_flow > 0 else None

            cur.execute("""
                INSERT INTO port_yearly_flow
                (port_id, year, departure_count, arrival_count, total_flow,
                 storm_count, storm_rate, unique_cargo_types, unique_destinations)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (port_id, year) DO UPDATE
                SET departure_count = EXCLUDED.departure_count,
                    arrival_count = EXCLUDED.arrival_count,
                    total_flow = EXCLUDED.total_flow,
                    storm_count = EXCLUDED.storm_count,
                    storm_rate = EXCLUDED.storm_rate,
                    unique_cargo_types = EXCLUDED.unique_cargo_types,
                    unique_destinations = EXCLUDED.unique_destinations
            """, (
                port_id, year, dep_count, arr_count, total_flow,
                storm_count, storm_rate, cargo_types, destinations
            ))
            total_inserted += 1

    cur.execute("""
        UPDATE port_yearly_flow pf
        SET flow_rank = (
            SELECT COUNT(DISTINCT total_flow) + 1
            FROM port_yearly_flow pf2
            WHERE pf2.year = pf.year AND pf2.total_flow > pf.total_flow
        )
    """)

    print(f"  Computed {total_inserted} port-year flow records")


def insert_tech_diffusion(cur):
    print("Inserting technology diffusion paths...")
    for tech in TECH_DIFFUSION_DATA:
        origin_ports = []
        for port_name in tech['origin_ports']:
            cur.execute("SELECT id FROM ports WHERE name ILIKE %s LIMIT 1", (port_name,))
            row = cur.fetchone()
            if row:
                origin_ports.append(row[0])

        if not origin_ports:
            continue

        spread_route = []
        cur.execute("""
            SELECT id FROM ports
            ORDER BY ST_Distance(geom, ST_SetSRID(ST_MakePoint(
                (SELECT ST_X(geom) FROM ports WHERE id = %s),
                (SELECT ST_Y(geom) FROM ports WHERE id = %s)
            ), 4326))
            LIMIT 8
        """, (origin_ports[0], origin_ports[0]))
        spread_route = [row[0] for row in cur.fetchall()]

        cur.execute("""
            INSERT INTO tech_diffusion_paths
            (tech_name, tech_name_zh, tech_category, origin_port_id,
             spread_route, estimated_start_year, estimated_end_year,
             diffusion_speed_km_yr, description)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
        """, (
            tech['name'], tech['name_zh'], tech['category'], origin_ports[0],
            spread_route, tech['start_year'], tech['end_year'],
            tech['speed'], tech['desc']
        ))
    print(f"  Inserted {len(TECH_DIFFUSION_DATA)} tech diffusion paths")


def insert_cargo_spread_records(cur):
    print("Computing cargo spread records...")
    cur.execute("""
        INSERT INTO cargo_spread_records
        (cargo_type, from_port_id, to_port_id, voyage_year,
         spread_direction, quantity_estimate)
        SELECT
            vr.cargo_type,
            vr.departure_port_id,
            vr.arrival_port_id,
            vr.voyage_year,
            CASE
                WHEN vr.voyage_year < 0 THEN 'early_antique'
                WHEN vr.voyage_year < 500 THEN 'late_antique'
                WHEN vr.voyage_year < 1000 THEN 'early_medieval'
                WHEN vr.voyage_year < 1500 THEN 'high_medieval'
                ELSE 'early_modern'
            END as spread_direction,
            (10 + random() * 100) * CASE
                WHEN vr.cargo_type IN ('spices', 'porcelain', 'gemstones') THEN 1.0
                WHEN vr.cargo_type IN ('silk', 'incense', 'ivory') THEN 0.8
                ELSE 0.5
            END as quantity_estimate
        FROM voyage_records vr
        WHERE random() < 0.3
    """)
    cur.execute("SELECT COUNT(*) FROM cargo_spread_records")
    count = cur.fetchone()[0]
    print(f"  Inserted {count} cargo spread records")


def insert_modern_ships(cur):
    print("Inserting modern ships...")
    for ship in MODERN_SHIPS:
        cur.execute("""
            INSERT INTO modern_ships
            (ship_name, mmsi, ship_type, gross_tonnage, length_m,
             beam_m, max_speed_knots, flag, home_port)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (mmsi) DO NOTHING
        """, (
            ship['name'], ship['mmsi'], ship['type'],
            ship['gross_tonnage'], ship['length'], ship['beam'],
            ship['max_speed'], ship['flag'], ship['home_port']
        ))
    print(f"  Inserted {len(MODERN_SHIPS)} modern ships")


def insert_modern_weather(cur):
    print("Generating modern weather forecasts...")
    today = date.today()
    total_inserted = 0
    for day_offset in range(7):
        forecast_date = today + timedelta(days=day_offset)
        for region in MODERN_WEATHER_REGIONS:
            wind_var = random.uniform(-3, 3)
            wave_var = random.uniform(-0.3, 0.3)
            storm_var = random.uniform(-0.02, 0.02)

            storm_prob = max(0.0, min(1.0, region['storm_prob'] + storm_var))

            size = 2.0
            lat = region['lat']
            lon = region['lon']
            polygon_str = (
                f"POLYGON(({lon - size} {lat - size}, "
                f"{lon + size} {lat - size}, "
                f"{lon + size} {lat + size}, "
                f"{lon - size} {lat + size}, "
                f"{lon - size} {lat - size}))"
            )

            cur.execute("""
                INSERT INTO modern_weather_forecasts
                (forecast_date, region, wind_direction_deg, wind_speed_knots,
                 wave_height_m, current_direction_deg, current_speed_knots,
                 visibility_nm, storm_probability, geom)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s,
                 ST_SetSRID(ST_GeomFromText(%s), 4326))
            """, (
                forecast_date, region['name'],
                region['wind_dir'] + random.uniform(-10, 10),
                max(0, region['wind_speed'] + wind_var),
                max(0, region['wave_height'] + wave_var),
                region['current_dir'] + random.uniform(-15, 15),
                max(0, region['current_speed'] + random.uniform(-0.3, 0.3)),
                max(1, region['visibility'] + random.uniform(-2, 2)),
                storm_prob,
                polygon_str
            ))
            total_inserted += 1
    print(f"  Generated {total_inserted} weather forecast records")


def main():
    print("=" * 60)
    print("Ancient Maritime Trade Extension Data Generator")
    print("=" * 60)

    conn = connect_db()
    cur = conn.cursor()

    try:
        insert_historical_events(cur)
        compute_yearly_port_flow(cur)
        insert_tech_diffusion(cur)
        insert_cargo_spread_records(cur)
        insert_modern_ships(cur)
        insert_modern_weather(cur)

        conn.commit()
        print("\n" + "=" * 60)
        print("All extension data generated successfully!")
        print("=" * 60)
    except Exception as e:
        conn.rollback()
        print(f"Error: {e}")
        raise
    finally:
        cur.close()
        conn.close()


if __name__ == '__main__':
    main()
