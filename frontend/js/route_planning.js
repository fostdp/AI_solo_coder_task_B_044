let routePlanningLayer = null;
let optimizedRouteLine = null;
let historicalRouteLine = null;
let routePlanningData = null;
let routePlanningLegend = null;

function initRoutePlanning() {
    createRoutePlanningUI();
    initRoutePlanningLayer();
    populatePortSelects();
    bindRoutePlanningEvents();
}

function createRoutePlanningUI() {
    const sidebar = document.getElementById('sidebar');

    const section = document.createElement('div');
    section.className = 'sidebar-section';
    section.id = 'route-planning-section';
    section.innerHTML = `
        <h3>🧭 航线规划与验证</h3>
        <div class="filter-group">
            <label>出发港</label>
            <select id="rp-departure-port">
                <option value="">请选择出发港</option>
            </select>
        </div>
        <div class="filter-group">
            <label>目的港</label>
            <select id="rp-arrival-port">
                <option value="">请选择目的港</option>
            </select>
        </div>
        <div class="filter-group">
            <label>季节</label>
            <select id="rp-season">
                <option value="spring">春季</option>
                <option value="summer">夏季</option>
                <option value="autumn">秋季</option>
                <option value="winter">冬季</option>
            </select>
        </div>
        <div class="filter-group">
            <label>船型</label>
            <select id="rp-ship-type">
                <option value="trireme">三列桨座战船</option>
                <option value="merchant_round_ship">商船</option>
                <option value="dhow">单桅三角帆船</option>
                <option value="junk">中国帆船</option>
                <option value="carrack">卡拉维尔帆船</option>
                <option value="longship">长船</option>
                <option value="galley">桨帆船</option>
                <option value="treasure_ship">宝船</option>
            </select>
        </div>
        <div class="filter-group checkbox-group">
            <label>
                <input type="checkbox" id="rp-show-routes" checked /> 显示规划航线
            </label>
        </div>
        <button id="btn-plan-route" class="btn-primary">规划最优航线</button>
        <div id="rp-results" class="analysis-result" style="margin-top:10px; display:none;">
            <div class="rp-comparison-card">
                <div class="rp-card-title">📊 航线对比分析</div>
                <div class="rp-card-row">
                    <span class="rp-label">距离差异</span>
                    <span class="rp-value" id="rp-distance-diff">-</span>
                </div>
                <div class="rp-card-row">
                    <span class="rp-label">时间差异</span>
                    <span class="rp-value" id="rp-time-diff">-</span>
                </div>
                <div class="rp-card-row">
                    <span class="rp-label">风险差异</span>
                    <span class="rp-value" id="rp-risk-diff">-</span>
                </div>
                <div class="rp-card-row highlight">
                    <span class="rp-label">相似度评分</span>
                    <span class="rp-value" id="rp-similarity">-</span>
                </div>
            </div>
            <div class="rp-route-details">
                <div class="rp-route-col">
                    <div class="rp-route-title" style="color:#4a9eff">── 模拟最优航线</div>
                    <div class="rp-route-detail">
                        <div>距离: <span id="rp-opt-distance">-</span> 海里</div>
                        <div>时间: <span id="rp-opt-time">-</span> 天</div>
                        <div>航速: <span id="rp-opt-speed">-</span> 节</div>
                        <div>风险: <span id="rp-opt-risk">-</span></div>
                    </div>
                </div>
                <div class="rp-route-col">
                    <div class="rp-route-title" style="color:#ff8844">── 历史航线</div>
                    <div class="rp-route-detail" id="rp-hist-detail">
                        <div style="color:var(--text-secondary); font-style:italic;">暂无历史数据</div>
                    </div>
                </div>
            </div>
            <div class="rp-validation">
                <div class="rp-validation-title">📜 古代航海家航线选择合理性</div>
                <div class="rp-validation-text" id="rp-validation-text">-</div>
            </div>
            <div class="rp-waypoints">
                <span>路径点匹配: </span>
                <span id="rp-waypoints-matched">-</span> / <span id="rp-total-waypoints">-</span>
            </div>
        </div>
    `;

    const stormSection = document.querySelector('.sidebar-section:nth-of-type(4)');
    if (stormSection) {
        sidebar.insertBefore(section, stormSection.nextSibling);
    } else {
        sidebar.appendChild(section);
    }

    addRoutePlanningStyles();
}

function addRoutePlanningStyles() {
    const style = document.createElement('style');
    style.textContent = `
        .rp-comparison-card {
            background: var(--bg-primary);
            border-radius: 6px;
            padding: 10px;
            margin-bottom: 10px;
            border: 1px solid var(--border-color);
        }
        .rp-card-title {
            font-size: 13px;
            font-weight: 600;
            color: var(--accent-blue);
            margin-bottom: 8px;
        }
        .rp-card-row {
            display: flex;
            justify-content: space-between;
            padding: 4px 0;
            font-size: 12px;
            border-bottom: 1px solid var(--border-color);
        }
        .rp-card-row:last-child {
            border-bottom: none;
        }
        .rp-card-row.highlight {
            background: rgba(74, 158, 255, 0.1);
            margin: 4px -6px;
            padding: 6px;
            border-radius: 4px;
            border-bottom: none;
        }
        .rp-label {
            color: var(--text-secondary);
        }
        .rp-value {
            color: var(--text-primary);
            font-weight: 500;
        }
        .rp-value.positive {
            color: var(--accent-green);
        }
        .rp-value.negative {
            color: var(--accent-red);
        }
        .rp-route-details {
            display: flex;
            gap: 10px;
            margin-bottom: 10px;
        }
        .rp-route-col {
            flex: 1;
            background: var(--bg-primary);
            border-radius: 6px;
            padding: 8px;
            border: 1px solid var(--border-color);
        }
        .rp-route-title {
            font-size: 12px;
            font-weight: 600;
            margin-bottom: 6px;
        }
        .rp-route-detail {
            font-size: 11px;
            color: var(--text-secondary);
            line-height: 1.6;
        }
        .rp-validation {
            background: rgba(255, 215, 0, 0.1);
            border: 1px solid var(--accent-gold);
            border-radius: 6px;
            padding: 10px;
            margin-bottom: 10px;
        }
        .rp-validation-title {
            font-size: 12px;
            font-weight: 600;
            color: var(--accent-gold);
            margin-bottom: 6px;
        }
        .rp-validation-text {
            font-size: 12px;
            color: var(--text-primary);
            line-height: 1.5;
        }
        .rp-waypoints {
            font-size: 11px;
            color: var(--text-secondary);
            text-align: center;
        }
    `;
    document.head.appendChild(style);
}

function initRoutePlanningLayer() {
    routePlanningLayer = L.layerGroup();
}

function populatePortSelects() {
    const ports = window.AppState ? window.AppState.ports : [];
    const depSelect = document.getElementById('rp-departure-port');
    const arrSelect = document.getElementById('rp-arrival-port');

    if (!depSelect || !arrSelect) return;

    const options = ports
        .filter(p => p.lat && p.lon)
        .sort((a, b) => (a.name_zh || a.name).localeCompare(b.name_zh || b.name))
        .map(p => `<option value="${p.id}">${p.name_zh || p.name}${p.region ? ` (${p.region})` : ''}</option>`)
        .join('');

    depSelect.innerHTML = '<option value="">请选择出发港</option>' + options;
    arrSelect.innerHTML = '<option value="">请选择目的港</option>' + options;
}

function bindRoutePlanningEvents() {
    document.getElementById('btn-plan-route').addEventListener('click', () => {
        planOptimalRoute();
    });

    document.getElementById('rp-departure-port').addEventListener('change', () => {
        autoPlanRoute();
    });

    document.getElementById('rp-arrival-port').addEventListener('change', () => {
        autoPlanRoute();
    });

    document.getElementById('rp-season').addEventListener('change', () => {
        autoPlanRoute();
    });

    document.getElementById('rp-ship-type').addEventListener('change', () => {
        autoPlanRoute();
    });

    document.getElementById('rp-show-routes').addEventListener('change', (e) => {
        toggleRoutePlanningLayer(e.target.checked);
    });

    if (window.AppState && window.AppState.layers) {
        window.AppState.layers.routePlanning = true;
    }
}

function autoPlanRoute() {
    const depId = document.getElementById('rp-departure-port').value;
    const arrId = document.getElementById('rp-arrival-port').value;

    if (depId && arrId) {
        planOptimalRoute();
    }
}

async function planOptimalRoute() {
    const departurePortId = document.getElementById('rp-departure-port').value;
    const arrivalPortId = document.getElementById('rp-arrival-port').value;
    const season = document.getElementById('rp-season').value;
    const shipType = document.getElementById('rp-ship-type').value;

    if (!departurePortId || !arrivalPortId) {
        alert('请选择出发港和目的港');
        return;
    }

    if (departurePortId === arrivalPortId) {
        alert('出发港和目的港不能相同');
        return;
    }

    showLoading();

    try {
        const resp = await apiFetch('/insights/route-planning', {
            departure_port_id: parseInt(departurePortId),
            arrival_port_id: parseInt(arrivalPortId),
            season: season,
            ship_type: shipType,
        });

        routePlanningData = resp;
        renderRoutePlanningResults(resp);
        renderRoutePlanningRoutes(resp);
        addRoutePlanningLegend();

        document.getElementById('rp-results').style.display = 'block';
    } catch (e) {
        console.error('Route planning failed:', e);
        document.getElementById('rp-results').innerHTML =
            '<p style="color:var(--accent-red); text-align:center; padding:10px;">航线规划失败，请重试</p>';
        document.getElementById('rp-results').style.display = 'block';
    } finally {
        hideLoading();
    }
}

function renderRoutePlanningResults(data) {
    const { optimized_route: opt, historical_route: hist, comparison } = data;

    const distDiff = comparison.distance_diff_pct;
    const timeDiff = comparison.time_diff_pct;
    const riskDiff = comparison.risk_diff_pct;
    const similarity = comparison.similarity_score;

    const distEl = document.getElementById('rp-distance-diff');
    const timeEl = document.getElementById('rp-time-diff');
    const riskEl = document.getElementById('rp-risk-diff');
    const simEl = document.getElementById('rp-similarity');

    distEl.textContent = formatDiffPct(distDiff);
    distEl.className = 'rp-value ' + getDiffClass(distDiff);

    timeEl.textContent = formatDiffPct(timeDiff);
    timeEl.className = 'rp-value ' + getDiffClass(timeDiff);

    riskEl.textContent = formatDiffPct(riskDiff);
    riskEl.className = 'rp-value ' + getDiffClass(riskDiff, true);

    simEl.textContent = (similarity * 100).toFixed(1) + '%';
    simEl.className = 'rp-value ' + (similarity > 0.7 ? 'positive' : similarity > 0.5 ? '' : 'negative');

    document.getElementById('rp-opt-distance').textContent = opt.distance_nautical_miles.toFixed(1);
    document.getElementById('rp-opt-time').textContent = opt.estimated_days.toFixed(1);
    document.getElementById('rp-opt-speed').textContent = opt.avg_speed_knots.toFixed(2);
    document.getElementById('rp-opt-risk').textContent = (opt.storm_risk * 100).toFixed(1) + '%';

    const histDetailEl = document.getElementById('rp-hist-detail');
    if (hist) {
        histDetailEl.innerHTML = `
            <div>距离: ${hist.distance_nautical_miles.toFixed(1)} 海里</div>
            <div>时间: ${hist.estimated_days.toFixed(1)} 天</div>
            <div>航速: ${hist.avg_speed_knots.toFixed(2)} 节</div>
            <div>风险: ${(hist.storm_risk * 100).toFixed(1)}%</div>
        `;
    } else {
        histDetailEl.innerHTML = '<div style="color:var(--text-secondary); font-style:italic;">暂无历史数据</div>';
    }

    document.getElementById('rp-validation-text').textContent = getValidationText(similarity);

    document.getElementById('rp-waypoints-matched').textContent = comparison.waypoints_matched;
    document.getElementById('rp-total-waypoints').textContent = comparison.total_waypoints;
}

function formatDiffPct(value) {
    const sign = value > 0 ? '+' : '';
    return sign + value.toFixed(1) + '%';
}

function getDiffClass(value, riskReversed = false) {
    if (riskReversed) {
        return value < 0 ? 'positive' : value > 0 ? 'negative' : '';
    }
    return value < 0 ? 'negative' : value > 0 ? 'positive' : '';
}

function getValidationText(similarity) {
    if (similarity > 0.8) {
        return '高度吻合，古代航海家已掌握最优航线。在当时的技术条件下，航线选择展现了卓越的航海智慧和对海洋环境的深刻理解。';
    } else if (similarity >= 0.6) {
        return '较为合理，存在局部优化空间。古代航线整体方向正确，但在部分航段的选择上可能受限于导航技术或对海洋气象的认知，存在小幅绕行。';
    } else {
        return '偏差较大，可能受限于当时的导航技术。古代航线与理论最优航线存在显著差异，反映了古代航海在定位精度、气象预测和洋流认知等方面的局限。';
    }
}

function renderRoutePlanningRoutes(data) {
    routePlanningLayer.clearLayers();

    const map = window.map;
    if (!map) return;

    const { optimized_route: opt, historical_route: hist } = data;

    if (hist && hist.route_points && hist.route_points.length > 0) {
        const histPoints = hist.route_points.map(p => [p[1], p[0]]);
        const histWeight = calculateRouteWeight(hist.distance_nautical_miles, false);

        historicalRouteLine = L.polyline(histPoints, {
            color: '#ff8844',
            weight: histWeight,
            opacity: 0.9,
            smoothFactor: 1.5,
        });

        historicalRouteLine.bindTooltip(
            `<b>历史航线</b><br>${hist.departure_port_name} → ${hist.arrival_port_name}<br>距离: ${hist.distance_nautical_miles.toFixed(1)} 海里<br>时间: ${hist.estimated_days.toFixed(1)} 天`,
            { direction: 'top', sticky: true }
        );

        historicalRouteLine.on('mouseover', function () {
            this.setStyle({ weight: histWeight + 3, opacity: 1 });
        });
        historicalRouteLine.on('mouseout', function () {
            this.setStyle({ weight: histWeight, opacity: 0.9 });
        });

        routePlanningLayer.addLayer(historicalRouteLine);
    }

    if (opt && opt.route_points && opt.route_points.length > 0) {
        const optPoints = opt.route_points.map(p => [p[1], p[0]]);
        const optWeight = calculateRouteWeight(opt.distance_nautical_miles, true);

        optimizedRouteLine = L.polyline(optPoints, {
            color: '#4a9eff',
            weight: optWeight,
            opacity: 0.9,
            dashArray: '8 6',
            smoothFactor: 1.5,
        });

        optimizedRouteLine.bindTooltip(
            `<b>模拟最优航线</b><br>${opt.departure_port_name} → ${opt.arrival_port_name}<br>距离: ${opt.distance_nautical_miles.toFixed(1)} 海里<br>时间: ${opt.estimated_days.toFixed(1)} 天`,
            { direction: 'top', sticky: true }
        );

        optimizedRouteLine.on('mouseover', function () {
            this.setStyle({ weight: optWeight + 3, opacity: 1 });
        });
        optimizedRouteLine.on('mouseout', function () {
            this.setStyle({ weight: optWeight, opacity: 0.9 });
        });

        routePlanningLayer.addLayer(optimizedRouteLine);
    }

    const showRoutes = document.getElementById('rp-show-routes').checked;
    if (showRoutes) {
        if (!map.hasLayer(routePlanningLayer)) {
            routePlanningLayer.addTo(map);
        }
        fitRouteBounds();
    }
}

function calculateRouteWeight(distance, isOptimized) {
    const baseWeight = isOptimized ? 4 : 3;
    const distanceFactor = Math.max(0, Math.min(2, 2000 / Math.max(distance, 100)));
    return baseWeight + distanceFactor;
}

function fitRouteBounds() {
    const map = window.map;
    if (!map || !routePlanningLayer) return;

    const bounds = routePlanningLayer.getBounds();
    if (bounds.isValid()) {
        map.fitBounds(bounds, { padding: [50, 50] });
    }
}

function toggleRoutePlanningLayer(show) {
    const map = window.map;
    if (!map) return;

    if (show) {
        if (routePlanningData) {
            routePlanningLayer.addTo(map);
            fitRouteBounds();
        }
    } else {
        if (map.hasLayer(routePlanningLayer)) {
            map.removeLayer(routePlanningLayer);
        }
    }

    if (window.AppState && window.AppState.layers) {
        window.AppState.layers.routePlanning = show;
    }
}

function addRoutePlanningLegend() {
    const map = window.map;
    if (!map) return;

    if (routePlanningLegend) {
        map.removeControl(routePlanningLegend);
    }

    routePlanningLegend = L.control({ position: 'bottomright' });
    routePlanningLegend.onAdd = function () {
        const div = L.DomUtil.create('div', 'route-planning-legend');
        div.style.cssText = `
            padding: 8px 12px;
            background: var(--bg-secondary);
            border-radius: 6px;
            border: 1px solid var(--border-color);
            margin-top: 8px;
        `;
        div.innerHTML = `
            <div style="font-size:12px; font-weight:600; color:var(--accent-blue); margin-bottom:6px;">航线规划</div>
            <div style="display:flex; align-items:center; gap:6px; font-size:11px; color:var(--text-secondary); margin:2px 0;">
                <div style="width:20px; height:0; border-top:2px dashed #4a9eff;"></div>
                模拟最优航线
            </div>
            <div style="display:flex; align-items:center; gap:6px; font-size:11px; color:var(--text-secondary); margin:2px 0;">
                <div style="width:20px; height:0; border-top:2px solid #ff8844;"></div>
                历史航线
            </div>
        `;
        return div;
    };
    routePlanningLegend.addTo(map);
}

function clearRoutePlanning() {
    routePlanningData = null;
    if (routePlanningLayer) {
        routePlanningLayer.clearLayers();
    }
    if (window.map && window.map.hasLayer(routePlanningLayer)) {
        window.map.removeLayer(routePlanningLayer);
    }
    if (routePlanningLegend && window.map) {
        window.map.removeControl(routePlanningLegend);
        routePlanningLegend = null;
    }
    const resultsEl = document.getElementById('rp-results');
    if (resultsEl) {
        resultsEl.style.display = 'none';
    }
}

function waitForPortsAndPopulate(retries) {
    if (window.AppState && window.AppState.ports && window.AppState.ports.length > 0) {
        populatePortSelects();
        return;
    }
    if (retries > 0) {
        setTimeout(() => waitForPortsAndPopulate(retries - 1), 200);
    }
}

function routePlanningInit() {
    if (document.getElementById('sidebar')) {
        initRoutePlanning();
        waitForPortsAndPopulate(30);
    } else {
        setTimeout(routePlanningInit, 100);
    }
}

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', routePlanningInit);
} else {
    routePlanningInit();
}

window.RoutePlanningModule = {
    init: initRoutePlanning,
    planRoute: planOptimalRoute,
    clear: clearRoutePlanning,
    toggleLayer: toggleRoutePlanningLayer,
    refreshPorts: populatePortSelects,
    getData: () => routePlanningData,
};
