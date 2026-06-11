# 古代航海贸易网络重建与风暴风险分析系统

基于 Rust (Axum) + PostgreSQL/PostGIS + Leaflet.js 的全栈应用，用于重建公元前 1000 年至公元 1800 年的古代航海贸易网络，并分析贸易航线的风暴风险。

## 目录

- [系统架构](#系统架构)
- [功能模块](#功能模块)
- [快速开始](#快速开始)
- [数据模拟器](#数据模拟器)
- [API 文档](#api-文档)
- [配置](#配置)
- [监控与可观测性](#监控与可观测性)
- [开发与部署](#开发与部署)

---

## 系统架构

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              Web Browser                                 │
│                   Leaflet + Canvas + 时间轴 + 详情面板                    │
└──────────────────────────────────┬───────────────────────────────────────┘
                                   │
                                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                         voyage_loader (:3001)                             │
│              ┌─────────────────┐         ┌──────────────────────┐         │
│              │ 静态文件服务     │         │ 航海/气候数据 API      │         │
│              │ (前端 SPA)       │         │ /api/ports /voyages   │         │
│              └─────────────────┘         │ /api/climate/* /stats │         │
│                                          └──────────────────────┘         │
│                  tracing JSON 日志 · Prometheus :4001 · Gzip              │
└───────────┬───────────────────────────────────────────────┬────────────────┘
            │                                               │
            ▼                                               ▼
┌─────────────────────────────┐         ┌─────────────────────────────────────┐
│   network_analyzer (:3002)  │         │   storm_risk_modeler (:3003)        │
│  ┌────────────────────────┐ │         │  ┌──────────────────────────────┐   │
│  │ Brandes 中介中心性      │ │         │  │ L2 正则化逻辑回归              │   │
│  │ 标签传播社区发现        │ │         │  │ 贝叶斯先验 + 概率收缩          │   │
│  │ 核心枢纽识别            │ │         │  │ 随机森林 (Gini)                │   │
│  └────────────────────────┘ │         │  └──────────────────────────────┘   │
│ /api/network                │         │ /api/storm-risk                    │
│ Prometheus :4002            │         │ Prometheus :4003                   │
└────────────┬────────────────┘         └─────────────┬───────────────────────┘
             │                                        │
             └──────────────────────────┬─────────────┘
                                        ▼
                       ┌────────────────────────────┐
                       │ PostgreSQL + PostGIS (:5432) │
                       │  · 时空索引 (GIST)          │
                       │  · 文本模糊匹配 (pg_trgm)   │
                       │  · 复合 B-Tree 索引         │
                       └────────────┬───────────────┘
                                    │
                                    ▼
                       ┌────────────────────────────┐
                       │       数据模拟器 (Python)    │
                       │ simulate_voyages.py        │
                       │ simulate_climate.py        │
                       └────────────────────────────┘

┌──────────────────────┐    ┌──────────────────────────┐
│  Prometheus (:9090)  │◄───│  Grafana 可视化 (:3000)   │
│  采集 Rust 服务指标   │    │  admin/admin              │
└──────────────────────┘    └──────────────────────────┘
```

### 服务端口

| 服务 | HTTP | Prometheus Metrics | 说明 |
|------|------|--------------------|------|
| voyage_loader | 3001 | 4001 | 航海记录 + 前端静态文件 |
| network_analyzer | 3002 | 4002 | 贸易网络重建 |
| storm_risk_modeler | 3003 | 4003 | 风暴风险分析 |
| maritime_insights | 3004 | 4004 | 综合洞察分析（4大模块） |
| PostgreSQL | 5432 | — | PostGIS 空间数据库 |
| Prometheus | 9090 | — | 指标采集 |
| Grafana | 3000 | — | 可视化面板 |

---

## 功能模块

### 1. voyage_loader — 航海记录导入

- 40 个古代港口 + 43 条历史别名（拜占庭→君士坦丁堡→伊斯坦布尔、刺桐→泉州等）
- 1500+ 条航海记录，时间跨度公元前 1000 年—公元 1800 年
- 港口名三级匹配：精确 → 归一化转写 → Jaro-Winkler 模糊匹配
- 古气候期（50 年分辨率）、洋流场、风场数据

### 2. network_analyzer — 贸易网络重建

- **中介中心性 (Betweenness Centrality)**：Brandes 算法变体，BFS 采样
- **社区发现 (Community Detection)**：标签传播算法 (Label Propagation)
- **枢纽识别 (Hub Identification)**：按中心性 Top-K 标记
- VecDeque 队列优化 O(1) 出队

### 3. storm_risk_modeler — 风暴风险分析

- **逻辑回归 (Logistic Regression)**：自适应 L2 正则化（按正样本数 λ=1.0/0.5/0.1）、贝叶斯权重先验 (μ=0, σ²=4)、预测收缩、贝叶斯平滑
- **随机森林 (Random Forest)**：Gini 不纯度准则，袋装采样
- 特征：季节、区域、洋流速度、风向变率、温度距平、历史风暴频率
- 热力图输出（按航线聚类）

### 4. 前端交互

- Leaflet.js 暗色底图 (CartoDB Dark)
- 航线按季节/货物着色、对数权重聚合、比例透明度
- 风暴标记 2° 网格聚类（红色 ✕）
- 时间轴双滑块（年份范围筛选）
- 风暴风险热力图 (leaflet.heat)
- 航线详情面板

---

## 快速开始

### 前置条件

- Docker 20.10+ 和 Docker Compose v2
- 或本地 Rust 1.75+、PostgreSQL 15+PostGIS 3.3、Python 3.10+

### 方式一：Docker Compose 一键部署

```bash
# 1. 克隆仓库
cd ancient-maritime

# 2. 构建并启动所有服务（首次构建 Rust 镜像需要 5-15 分钟）
docker compose up -d --build

# 3. 查看服务状态
docker compose ps

# 4. 查看数据加载日志（首次启动会自动导入模拟数据）
docker compose logs data-loader -f

# 5. 打开前端
#    http://localhost:3001          → 航海贸易地图
#    http://localhost:9090          → Prometheus
#    http://localhost:3000          → Grafana (admin/admin)
```

### 方式二：本地开发

#### PostgreSQL (Docker)

```bash
docker run -d --name maritime-pg \
  -e POSTGRES_DB=ancient_maritime \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  postgis/postgis:15-3.3

psql -h localhost -U postgres -d ancient_maritime -f scripts/init_db.sql
```

#### 数据导入

```bash
pip install psycopg2-binary
python scripts/simulate_voyages.py --db
python scripts/simulate_climate.py --db
```

#### Rust 服务

```bash
cd backend

# 开发模式启动三个服务（三个终端）
cargo run -p voyage-loader
cargo run -p network-analyzer
cargo run -p storm-risk-modeler

# 或 release 模式
cargo build --release -p voyage-loader -p network-analyzer -p storm-risk-modeler
```

打开 http://localhost:3001

---

## 数据模拟器

### 航海记录模拟器 `simulate_voyages.py`

```
python scripts/simulate_voyages.py [OPTIONS]
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `-n, --num-records` | int | 1500 | 生成记录数 |
| `--year-start` | int | -1000 | 起始年份（负=公元前） |
| `--year-end` | int | 1800 | 结束年份 |
| `--regions` | str | — | 逗号分隔区域：Mediterranean,Indian_Ocean,East_Asia,Red_Sea,Atlantic |
| `--ports` | str | — | 逗号分隔港口 ID 或名称：21,22 或 Quanzhou,Guangzhou |
| `--storm-multiplier` | float | 1.0 | 风暴概率倍率 |
| `--seed` | int | 42 | 随机种子 |
| `--outdir` | str | scripts/ | JSON 输出目录 |
| `--db` | flag | — | 写入 PostgreSQL |
| `--db-host/--db-port/--db-name/--db-user/--db-password` | str | — | 数据库连接（也支持 PGHOST 等环境变量） |

**示例：**

```bash
# 生成 3000 条中世纪地中海+红海高风暴记录
python scripts/simulate_voyages.py -n 3000 \
  --year-start 500 --year-end 1500 \
  --regions Mediterranean,Red_Sea \
  --storm-multiplier 1.5

# 仅泉州宋元时代 200 条
python scripts/simulate_voyages.py -n 200 \
  --year-start 960 --year-end 1368 \
  --ports Quanzhou --seed 123
```

### 古气候模拟器 `simulate_climate.py`

```
python scripts/simulate_climate.py [OPTIONS]
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `--year-start` | int | -1000 | 起始年份 |
| `--year-end` | int | 1800 | 结束年份 |
| `--resolution` | int | 50 | 气候期分辨率（年） |
| `--regions` | str | — | 逗号分隔海洋区域 |
| `--storm-multiplier` | float | 1.0 | 风暴频率倍率 |
| `--temp-offset` | float | 0.0 | 温度偏移 (°C)，模拟小冰期/暖期 |
| `--seed` / `--outdir` / `--db*` | — | 同上 |同上 |

**示例：**

```bash
# 小冰期大西洋+地中海气候
python scripts/simulate_climate.py \
  --year-start 1500 --year-end 1700 \
  --regions Atlantic_North,Atlantic_South,Mediterranean_Western,Mediterranean_Eastern \
  --temp-offset -1.5 --storm-multiplier 1.5 --seed 2024
```

---

## API 文档

所有 API 均返回 JSON，默认开启 Gzip 压缩。所有服务支持 CORS。

### voyage-loader (:3001)

#### `GET /api/ports`
获取所有港口列表。

```json
[
  {"id": 21, "name": "Quanzhou", "name_zh": "泉州", "region": "East Asia",
   "lon": 118.6758, "lat": 24.8741}
]
```

#### `GET /api/voyages`
获取航海记录。支持查询参数：

| 参数 | 说明 |
|------|------|
| `year_start` / `year_end` | 年份范围 |
| `departure_port_id` / `arrival_port_id` | 港口筛选 |
| `season` | spring/summer/autumn/winter |
| `cargo_type` / `ship_type` | 货物/船型 |
| `encountered_storm` | true/false |
| `limit` / `offset` | 分页 |

#### `GET /api/voyages/:id`
单条航海记录详情（含航线点）。

#### `GET /api/climate/periods`
古气候期列表。

#### `GET /api/climate/currents`
`?period_id=&season=` 洋流查询。

#### `GET /api/climate/winds`
`?period_id=&season=` 风场查询。

#### `GET /api/stats`
数据统计摘要（记录数、风暴率、按区域/年代统计）。

---

### network-analyzer (:3002)

#### `GET /api/network`
贸易网络分析。

| 参数 | 默认 | 说明 |
|------|------|------|
| `year_start` | -1000 | 起始年份 |
| `year_end` | 1800 | 结束年份 |
| `analysis_type` | all | `betweenness` / `communities` / `hubs` / `all` |

返回：

```json
{
  "ports": [{"id": 21, "betweenness": 0.154, "community": 2, "is_hub": true, ...}],
  "edges": [{"from": 21, "to": 23, "weight": 42, ...}],
  "communities": [{"id": 1, "members": [...], "avg_betweenness": ...}],
  "hubs": [21, 5, 16, ...]
}
```

---

### storm-risk-modeler (:3003)

#### `GET /api/storm-risk`

| 参数 | 默认 | 说明 |
|------|------|------|
| `model_type` | logistic_regression | `logistic_regression` / `random_forest` |
| `year_start` / `year_end` | -1000/1800 | 年份范围 |
| `season` | — | 季节筛选 |

返回：

```json
{
  "model_type": "logistic_regression",
  "risks": [
    {"departure_port_id": 21, "arrival_port_id": 23, "season": "winter",
     "risk_score": 0.372, "sample_size": 85, "confidence": 0.81}
  ],
  "heatmap": [{"lon": 118.7, "lat": 24.9, "intensity": 0.42}, ...]
}
```

---

## 配置

Rust 服务所有参数外置到 `backend/config.toml`。

```toml
[database]
url = "postgres://postgres:postgres@localhost:5432/ancient_maritime"
max_connections = 20

[voyage_loader]
port = 3001
max_query_limit = 2000

[network_analyzer]
port = 3002
betweenness_sample_size = 20      # Brandes BFS 采样端口数
community_max_iterations = 10     # 标签传播最大迭代
hub_top_k = 5

[storm_risk_modeler]
port = 3003

[storm_risk_modeler.logistic_regression]
learning_rate = 0.01
iterations = 500
l2_lambda_sparse = 1.0            # 正样本 <20
l2_lambda_medium = 0.5            # 正样本 20-100
l2_lambda_dense = 0.1             # 正样本 >100
prior_variance = 4.0              # 权重正态先验 σ²
prior_storm_rate = 0.15           # 先验风暴概率
prediction_shrinkage_k = 5.0      # 向先验收缩强度
smoothing_k = 10.0                # 贝叶斯平滑强度

[storm_risk_modeler.random_forest]
n_trees = 10
max_depth = 5
min_samples = 5
sample_ratio = 0.7
```

可通过 `CONFIG_PATH` 环境变量覆盖路径。

---

## 监控与可观测性

### 结构化日志

所有 Rust 服务输出 JSON 格式结构化日志（tracing）：

```json
{"timestamp":"...","level":"INFO","target":"voyage_loader",
 "port":3001,"metrics_port":4001,"message":"VoyageLoader service starting"}
```

日志级别通过 `RUST_LOG` 环境变量控制：

```bash
RUST_LOG="storm_risk_modeler=debug,sqlx=warn,tower_http=info"
```

### Prometheus 指标

每个服务暴露独立 Prometheus HTTP 端点（服务端口 + 1000）：

- voyage-loader → http://localhost:4001/metrics
- network-analyzer → http://localhost:4002/metrics
- storm-risk-modeler → http://localhost:4003/metrics

指标列表：

| 指标 | 类型 | 标签 | 说明 |
|------|------|------|------|
| `http_requests_total` | Counter | method, path, status | HTTP 请求数 |
| `http_request_duration_seconds` | Histogram | method, path | 请求耗时 |
| `voyage_loader_startups_total` | Counter | — | 服务启动次数 |
| `network_analyzer_startups_total` | Counter | — | 服务启动次数 |
| `storm_risk_modeler_startups_total` | Counter | — | 服务启动次数 |

### Prometheus + Grafana

Docker Compose 已内置：

- **Prometheus**：http://localhost:9090 → Status → Targets 查看三个 Rust 服务采集状态
- **Grafana**：http://localhost:3000 → 用户名/密码 `admin/admin`

配置 Prometheus 数据源：
1. 进入 Grafana → Connections → Data sources → Add data source → Prometheus
2. URL: `http://prometheus:9090`
3. Save & Test

示例 PromQL 查询：

```promql
# 每秒请求速率 (rate 5m)
rate(http_requests_total[5m])

# p95 请求延迟
histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket[5m])) by (le, path))
```

---

## 开发与部署

### 项目结构

```
ancient-maritime/
├── backend/
│   ├── Cargo.toml                  # Cargo workspace
│   ├── config.toml                 # 模型参数外置
│   ├── Dockerfile                  # 多阶段构建 + 多二进制
│   └── crates/
│       ├── maritime_common/        # 共享库：models, db, config
│       │   └── src/{lib,config,db,models}.rs
│       ├── voyage_loader/          # :3001 航海记录服务
│       │   └── src/{main,handlers}.rs
│       ├── network_analyzer/       # :3002 网络分析服务
│       │   └── src/{main,analysis}.rs
│       └── storm_risk_modeler/     # :3003 风暴风险服务
│           └── src/{main,analysis}.rs
├── frontend/
│   ├── index.html
│   ├── css/style.css
│   └── js/
│       ├── app.js                  # 全局状态、API 路由、事件
│       ├── trade_map.js            # 地图/航线/热力图渲染
│       ├── voyage_detail.js        # 详情面板
│       ├── timeline.js             # 时间轴滑块
│       ├── network.js              # 网络分析调用
│       └── storm.js                # 风暴分析调用
├── scripts/
│   ├── init_db.sql                 # PostGIS 表结构 + 索引 + 调优
│   ├── simulate_voyages.py         # 航海记录模拟器
│   └── simulate_climate.py         # 古气候模拟器
├── monitoring/
│   └── prometheus.yml              # Prometheus 抓取配置
└── docker-compose.yml              # 全栈一键部署
```

### Rust Crate 依赖

- `axum 0.7` — Web 框架
- `sqlx 0.7` — PostgreSQL async ORM（编译时查询验证）
- `tower-http` — CORS、Gzip、Trace、ServeDir 中间件
- `tracing + tracing-subscriber` — 结构化 JSON 日志
- `metrics + metrics-exporter-prometheus` — Prometheus 指标
- `serde/serde_json/toml/chrono` — 序列化与配置

### PostgreSQL 空间索引配置

- GIST 空间索引 (fillfactor=90)：ports.geom, voyage_records.route_geom, ocean_currents.geom, wind_fields.geom
- GIN pg_trgm 索引：港口名模糊匹配
- 复合 B-Tree 索引：(year, season), (dep_id, arr_id, year), (port_id, period_start, period_end) 等
- 数据库参数：shared_buffers=512MB, work_mem=64MB, effective_cache_size=2GB, random_page_cost=1.1

### 常见问题

**Q: Rust 编译卡在 `cargo build`?**
A: 首次编译依赖链较长。可以用 `--jobs` 控制并行度，或提前 `cargo fetch`。

**Q: 前端无法调用 API (CORS)?**
A: 本地打开 `file://` 时会触发 CORS。请通过 voyage-loader 的 `http://localhost:3001` 访问前端。

**Q: 模拟器生成的风暴比例不对？**
A: 调整 `--storm-multiplier`，数值>1 增加风暴，<1 减少。默认 1.0 对应 ~20% 风暴率。

---

## License

本项目为研究用途示例。
