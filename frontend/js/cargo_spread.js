const CARGO_SPREAD_CARGOS = [
    { value: 'spices', label: '香料' },
    { value: 'silk', label: '丝绸' },
    { value: 'porcelain', label: '瓷器' },
    { value: 'gemstones', label: '宝石' },
    { value: 'wine', label: '葡萄酒' },
    { value: 'olive_oil', label: '橄榄油' },
    { value: 'grain', label: '粮食' },
    { value: 'timber', label: '木材' },
    { value: 'incense', label: '乳香' },
    { value: 'ivory', label: '象牙' },
];

const TECH_PRESETS = [
    { value: 'iron_smelting', label: '冶铁技术', category: '冶金' },
    { value: 'porcelain', label: '瓷器制造', category: '陶瓷' },
    { value: 'shipbuilding', label: '造船技术', category: '航海' },
    { value: 'navigation', label: '航海术', category: '航海' },
    { value: 'papermaking', label: '造纸术', category: '技术' },
    { value: 'coinage', label: '铸币技术', category: '经济' },
];

let cargoSpreadLayer = null;
let cargoSpreadEdges = [];
let cargoSpreadNodes = [];
let originMarkers = [];
let hubMarkers = [];
let techDiffusionLayer = null;
let techAnimationId = null;
let techAnimationPlaying = false;
let techAnimationProgress = 0;
let currentTechDiffusion = null;

function initCargoSpread() {
    cargoSpreadLayer = L.layerGroup();
    techDiffusionLayer = L.layerGroup();
    initCargoSpreadUI();
}

function initCargoSpreadUI() {
    const sidebar = document.getElementById('sidebar');

    const section = document.createElement('div');
    section.className = 'sidebar-section';
    section.innerHTML = `
        <h3>📦 货物传播与文化交流</h3>
        <div class="filter-group">
            <label>货物类型</label>
            <select id="cargo-spread-type">
                ${CARGO_SPREAD_CARGOS.map(c => `<option value="${c.value}">${c.label}</option>`).join('')}
            </select>
        </div>
        <button id="btn-cargo-spread" class="btn-primary">分析传播网络</button>
        <div id="cargo-spread-info" class="analysis-result"></div>
        
        <div style="margin-top:12px">
            <h4 style="margin:8px 0;color:var(--accent-gold)">🏺 技术扩散</h4>
            <div class="filter-group">
                <label>选择技术</label>
                <select id="tech-diffusion-select">
                    ${TECH_PRESETS.map(t => `<option value="${t.value}">${t.label} (${t.category})</option>`).join('')}
                </select>
            </div>
            <div class="analysis-controls">
                <button id="btn-tech-play" class="btn-analysis">▶ 播放扩散</button>
                <button id="btn-tech-pause" class="btn-analysis" disabled>⏸ 暂停</button>
            </div>
            <div id="tech-diffusion-info" class="analysis-result"></div>
        </div>
    `;

    const stormSection = sidebar.querySelector('.sidebar-section:nth-child(4)');
    if (stormSection) {
        sidebar.insertBefore(section, stormSection);
    } else {
        sidebar.appendChild(section);
    }

    document.getElementById('btn-cargo-spread').addEventListener('click', async () => {
        const cargoType = document.getElementById('cargo-spread-type').value;
        await loadCargoSpread(cargoType);
    });

    document.getElementById('btn-tech-play').addEventListener('click', () => {
        const techName = document.getElementById('tech-diffusion-select').value;
        startTechDiffusionAnimation(techName);
    });

    document.getElementById('btn-tech-pause').addEventListener('click', () => {
        pauseTechDiffusionAnimation();
    });

    document.getElementById('tech-diffusion-select').addEventListener('change', () => {
        const techName = document.getElementById('tech-diffusion-select').value;
        showTechDiffusionPath(techName);
    });
}

async function loadCargoSpread(cargoType) {
    showLoading();
    try {
        const resp = await apiFetch('/insights/cargo-spread', {
            cargo_type: cargoType,
            year_start: AppState.yearStart,
            year_end: AppState.yearEnd,
        });

        AppState.cargoSpreadData = resp;
        renderCargoSpreadNetwork();
        updateCargoSpreadInfo();
        updateCulturalExchangeRanking();

        AppState.layers.cargoSpread = true;
        const layerCheckbox = document.getElementById('layer-cargo-spread');
        if (layerCheckbox) layerCheckbox.checked = true;

        const firstTech = resp.tech_diffusions[0];
        if (firstTech) {
            document.getElementById('tech-diffusion-select').value = firstTech.tech_name;
            showTechDiffusionPath(firstTech.tech_name);
        }
    } catch (e) {
        console.error('Cargo spread analysis failed:', e);
        document.getElementById('cargo-spread-info').innerHTML = '<p style="color:var(--accent-red)">分析失败</p>';
    } finally {
        hideLoading();
    }
}

function renderCargoSpreadNetwork() {
    cargoSpreadLayer.clearLayers();
    originMarkers = [];
    hubMarkers = [];
    cargoSpreadEdges = [];
    cargoSpreadNodes = [];

    const data = AppState.cargoSpreadData;
    if (!data || !data.spread_network) return;

    const network = data.spread_network;
    const nodeMap = {};
    network.nodes.forEach(n => {
        nodeMap[n.port_id] = n;
    });

    const portMap = {};
    AppState.ports.forEach(p => {
        portMap[p.id] = p;
    });

    const years = network.edges.map(e => e.first_spread_year).filter(y => y > 0);
    const minYear = years.length > 0 ? Math.min(...years) : AppState.yearStart;
    const maxYear = years.length > 0 ? Math.max(...years) : AppState.yearEnd;

    network.edges.forEach(edge => {
        const fromPort = portMap[edge.from_port_id];
        const toPort = portMap[edge.to_port_id];
        if (!fromPort || !toPort) return;

        const color = getSpreadYearColor(edge.first_spread_year, minYear, maxYear);
        const weight = 1 + Math.log2(edge.flow_volume + 1) * 1.5;

        const line = L.polyline([[fromPort.lat, fromPort.lon], [toPort.lat, toPort.lon]], {
            color: color,
            weight: weight,
            opacity: 0.7,
            smoothFactor: 1.5,
        });

        const arrowIcon = L.divIcon({
            html: `<div style="color:${color};font-size:12px;transform:rotate(${getBearing(fromPort, toPort)}deg)">➤</div>`,
            className: 'spread-arrow',
            iconSize: [12, 12],
            iconAnchor: [6, 6],
        });

        const midLat = (fromPort.lat + toPort.lat) / 2;
        const midLon = (fromPort.lon + toPort.lon) / 2;
        const arrowMarker = L.marker([midLat, midLon], { icon: arrowIcon, interactive: false });

        line.bindTooltip(
            `${fromPort.name_zh || fromPort.name} → ${toPort.name_zh || toPort.name}<br>流量: ${edge.flow_volume.toFixed(1)}<br>首次传播: ${edge.first_spread_year}年`,
            { direction: 'top', sticky: true }
        );

        line.on('click', () => {
            showSpreadEdgeDetail(edge, fromPort, toPort);
        });

        cargoSpreadLayer.addLayer(line);
        cargoSpreadLayer.addLayer(arrowMarker);
        cargoSpreadEdges.push({ line, arrowMarker, edge, fromPort, toPort });
    });

    network.nodes.forEach(node => {
        const port = portMap[node.port_id];
        if (!port) return;

        const radius = 4 + node.adoption_level * 12;
        const isOrigin = network.origin_ports.includes(node.port_id);
        const isHub = network.hub_ports.includes(node.port_id);

        if (isOrigin) {
            const starIcon = L.divIcon({
                html: `<div style="color:#ffd700;font-size:${radius * 2}px;text-shadow:0 0 6px #ffaa00,0 0 12px #ff8800">★</div>`,
                className: 'origin-marker',
                iconSize: [radius * 2, radius * 2],
                iconAnchor: [radius, radius],
            });
            const marker = L.marker([port.lat, port.lon], { icon: starIcon });
            marker.bindTooltip(
                `⭐ 起源港<br>${port.name_zh || port.name}<br>首次接收: ${node.first_received_year}年<br>采纳度: ${(node.adoption_level * 100).toFixed(1)}%<br>介数: ${node.betweenness.toFixed(4)}`,
                { direction: 'top' }
            );
            marker.on('click', () => showSpreadNodeDetail(node, port, 'origin'));
            cargoSpreadLayer.addLayer(marker);
            originMarkers.push(marker);
        }

        if (isHub) {
            const diamondIcon = L.divIcon({
                html: `<div style="color:#9b59b6;font-size:${radius * 1.8}px;text-shadow:0 0 6px #8e44ad">◆</div>`,
                className: 'hub-marker',
                iconSize: [radius * 1.8, radius * 1.8],
                iconAnchor: [radius * 0.9, radius * 0.9],
            });
            const marker = L.marker([port.lat, port.lon], { icon: diamondIcon });
            marker.bindTooltip(
                `💎 枢纽港<br>${port.name_zh || port.name}<br>首次接收: ${node.first_received_year}年<br>采纳度: ${(node.adoption_level * 100).toFixed(1)}%<br>介数: ${node.betweenness.toFixed(4)}`,
                { direction: 'top' }
            );
            marker.on('click', () => showSpreadNodeDetail(node, port, 'hub'));
            cargoSpreadLayer.addLayer(marker);
            hubMarkers.push(marker);
        }

        if (!isOrigin && !isHub) {
            const circleMarker = L.circleMarker([port.lat, port.lon], {
                radius: radius,
                fillColor: '#3498db',
                color: '#2980b9',
                weight: 1,
                opacity: 0.8,
                fillOpacity: 0.6,
            });
            circleMarker.bindTooltip(
                `${port.name_zh || port.name}<br>首次接收: ${node.first_received_year}年<br>采纳度: ${(node.adoption_level * 100).toFixed(1)}%<br>介数: ${node.betweenness.toFixed(4)}`,
                { direction: 'top' }
            );
            circleMarker.on('click', () => showSpreadNodeDetail(node, port, 'normal'));
            cargoSpreadLayer.addLayer(circleMarker);
            cargoSpreadNodes.push({ marker: circleMarker, node, port });
        }
    });

    const layerCheckbox = document.getElementById('layer-cargo-spread');
    if (layerCheckbox && layerCheckbox.checked) {
        cargoSpreadLayer.addTo(map);
    }
}

function getSpreadYearColor(year, minYear, maxYear) {
    if (year <= 0 || minYear === maxYear) return '#87ceeb';

    const t = (year - minYear) / (maxYear - minYear);

    const r = Math.round(135 + t * (106 - 135));
    const g = Math.round(206 - t * 206);
    const b = Math.round(235 + t * (186 - 235));

    return `rgb(${r},${g},${b})`;
}

function getBearing(from, to) {
    const lat1 = from.lat * Math.PI / 180;
    const lat2 = to.lat * Math.PI / 180;
    const dLon = (to.lon - from.lon) * Math.PI / 180;

    const y = Math.sin(dLon) * Math.cos(lat2);
    const x = Math.cos(lat1) * Math.sin(lat2) - Math.sin(lat1) * Math.cos(lat2) * Math.cos(dLon);
    let brng = Math.atan2(y, x) * 180 / Math.PI;

    return brng + 90;
}

function updateCargoSpreadInfo() {
    const data = AppState.cargoSpreadData;
    if (!data || !data.spread_network) return;

    const network = data.spread_network;
    const cargoZh = CARGO_ZH[data.cargo_type] || data.cargo_type;

    document.getElementById('cargo-spread-info').innerHTML = `
        <p>货物类型: <strong>${cargoZh}</strong></p>
        <p>港口节点: ${network.nodes.length}</p>
        <p>传播路径: ${network.edges.length}</p>
        <p>起源港口: ${network.origin_ports.length}</p>
        <p>枢纽港口: ${network.hub_ports.length}</p>
        <div id="cultural-exchange-ranking" style="margin-top:8px"></div>
    `;
}

function updateCulturalExchangeRanking() {
    const data = AppState.cargoSpreadData;
    if (!data || !data.spread_network) return;

    const network = data.spread_network;
    const portMap = {};
    AppState.ports.forEach(p => { portMap[p.id] = p; });

    const ranked = [...network.nodes]
        .sort((a, b) => b.betweenness - a.betweenness)
        .slice(0, 5);

    const rankingHtml = `
        <h4 style="margin:8px 0;color:var(--accent-gold)">🏆 文化交流指数排名</h4>
        ${ranked.map((n, i) => {
            const port = portMap[n.port_id];
            const name = port ? (port.name_zh || port.name) : n.port_name;
            const medal = ['🥇', '🥈', '🥉', '4️⃣', '5️⃣'][i];
            return `<div class="hub-item">${medal} ${name} (${n.betweenness.toFixed(4)})</div>`;
        }).join('')}
    `;

    const rankingEl = document.getElementById('cultural-exchange-ranking');
    if (rankingEl) {
        rankingEl.innerHTML = rankingHtml;
    }
}

function showSpreadNodeDetail(node, port, type) {
    const typeLabel = {
        origin: '⭐ 起源港',
        hub: '💎 枢纽港',
        normal: '📍 普通港',
    }[type] || '港口';

    const panelContent = `
        <h3 style="color:var(--accent-gold)">${typeLabel}</h3>
        <h2>${port.name_zh || port.name}</h2>
        <p>英文名: ${port.name}</p>
        <p>所属地区: ${port.region || '未知'}</p>
        <hr style="margin:12px 0;border-color:#333">
        <h4>📊 传播数据</h4>
        <p>首次接收年份: <strong>${node.first_received_year}年</strong></p>
        <p>采纳度: <strong>${(node.adoption_level * 100).toFixed(1)}%</strong></p>
        <p>介数中心性: <strong>${node.betweenness.toFixed(4)}</strong></p>
    `;

    document.getElementById('panel-content').innerHTML = panelContent;
    document.getElementById('voyage-panel').classList.remove('hidden');
}

function showSpreadEdgeDetail(edge, fromPort, toPort) {
    const panelContent = `
        <h3 style="color:var(--accent-gold)">🔗 传播路径</h3>
        <h2>${fromPort.name_zh || fromPort.name} → ${toPort.name_zh || toPort.name}</h2>
        <hr style="margin:12px 0;border-color:#333">
        <h4>📊 路径数据</h4>
        <p>流量: <strong>${edge.flow_volume.toFixed(1)}</strong></p>
        <p>首次传播年份: <strong>${edge.first_spread_year}年</strong></p>
    `;

    document.getElementById('panel-content').innerHTML = panelContent;
    document.getElementById('voyage-panel').classList.remove('hidden');
}

function showTechDiffusionPath(techName) {
    techDiffusionLayer.clearLayers();
    stopTechDiffusionAnimation();

    const data = AppState.cargoSpreadData;
    if (!data || !data.tech_diffusions) return;

    const tech = data.tech_diffusions.find(t => t.tech_name === techName);
    if (!tech) return;

    currentTechDiffusion = tech;

    const portMap = {};
    AppState.ports.forEach(p => { portMap[p.id] = p; });

    const routePorts = tech.spread_route
        .map(id => portMap[id])
        .filter(p => p);

    if (routePorts.length < 2) return;

    const line = L.polyline(
        routePorts.map(p => [p.lat, p.lon]),
        {
            color: '#9b59b6',
            weight: 3,
            opacity: 0.8,
            dashArray: '8 4',
            smoothFactor: 1.5,
        }
    );
    techDiffusionLayer.addLayer(line);

    routePorts.forEach((port, i) => {
        const isOrigin = i === 0;
        const marker = L.circleMarker([port.lat, port.lon], {
            radius: isOrigin ? 8 : 5,
            fillColor: isOrigin ? '#e74c3c' : '#9b59b6',
            color: isOrigin ? '#c0392b' : '#8e44ad',
            weight: 2,
            opacity: 0.9,
            fillOpacity: 0.8,
        });
        marker.bindTooltip(
            `${isOrigin ? '🏠 起源: ' : ''}${port.name_zh || port.name}`,
            { direction: 'top' }
        );
        techDiffusionLayer.addLayer(marker);
    });

    const yearsSpan = tech.estimated_end_year - tech.estimated_start_year;
    const techZh = TECH_PRESETS.find(t => t.value === techName)?.label || tech.tech_name_zh;

    document.getElementById('tech-diffusion-info').innerHTML = `
        <p>技术: <strong>${techZh}</strong></p>
        <p>起源: ${tech.origin_port_name}</p>
        <p>起始年份: ${tech.estimated_start_year}年</p>
        <p>传播耗时: 约 ${yearsSpan} 年</p>
        <p>扩散速度: ${tech.diffusion_speed_km_yr} km/年</p>
        <p>途经港口: ${routePorts.length} 个</p>
    `;

    if (AppState.layers.cargoSpread) {
        techDiffusionLayer.addTo(map);
    }
}

function startTechDiffusionAnimation(techName) {
    const data = AppState.cargoSpreadData;
    if (!data || !data.tech_diffusions) return;

    const tech = data.tech_diffusions.find(t => t.tech_name === techName);
    if (!tech) return;

    showTechDiffusionPath(techName);

    const portMap = {};
    AppState.ports.forEach(p => { portMap[p.id] = p; });

    const routePorts = tech.spread_route
        .map(id => portMap[id])
        .filter(p => p);

    if (routePorts.length < 2) return;

    techAnimationProgress = 0;
    techAnimationPlaying = true;

    document.getElementById('btn-tech-play').disabled = true;
    document.getElementById('btn-tech-pause').disabled = false;

    const totalSteps = 100;
    let step = 0;

    function animate() {
        if (!techAnimationPlaying) return;

        step++;
        techAnimationProgress = step / totalSteps;

        if (step > totalSteps) {
            stopTechDiffusionAnimation();
            return;
        }

        updateTechAnimationFrame(routePorts, tech, techAnimationProgress);

        techAnimationId = requestAnimationFrame(animate);
    }

    animate();
}

function updateTechAnimationFrame(routePorts, tech, progress) {
    techDiffusionLayer.clearLayers();

    const portMap = {};
    AppState.ports.forEach(p => { portMap[p.id] = p; });

    const totalSegments = routePorts.length - 1;
    const currentSegment = Math.floor(progress * totalSegments);
    const segmentProgress = (progress * totalSegments) - currentSegment;

    const fullPorts = routePorts.slice(0, currentSegment + 1);
    if (fullPorts.length > 1) {
        const completedLine = L.polyline(
            fullPorts.map(p => [p.lat, p.lon]),
            {
                color: '#e74c3c',
                weight: 4,
                opacity: 0.9,
                smoothFactor: 1.5,
            }
        );
        techDiffusionLayer.addLayer(completedLine);
    }

    if (currentSegment < totalSegments) {
        const start = routePorts[currentSegment];
        const end = routePorts[currentSegment + 1];
        const midLat = start.lat + (end.lat - start.lat) * segmentProgress;
        const midLon = start.lon + (end.lon - start.lon) * segmentProgress;

        const partialLine = L.polyline(
            [[start.lat, start.lon], [midLat, midLon]],
            {
                color: '#e74c3c',
                weight: 4,
                opacity: 0.9,
                smoothFactor: 1.5,
            }
        );
        techDiffusionLayer.addLayer(partialLine);

        const movingIcon = L.divIcon({
            html: '<div style="color:#f39c12;font-size:20px;text-shadow:0 0 8px #e67e22">⚡</div>',
            className: 'tech-moving-marker',
            iconSize: [20, 20],
            iconAnchor: [10, 10],
        });
        const movingMarker = L.marker([midLat, midLon], { icon: movingIcon, interactive: false });
        techDiffusionLayer.addLayer(movingMarker);
    }

    routePorts.forEach((port, i) => {
        const isActivated = i <= currentSegment;
        const isOrigin = i === 0;
        const marker = L.circleMarker([port.lat, port.lon], {
            radius: isOrigin ? 8 : 5,
            fillColor: isActivated ? (isOrigin ? '#e74c3c' : '#27ae60') : '#7f8c8d',
            color: isActivated ? (isOrigin ? '#c0392b' : '#1e8449') : '#5d6d7e',
            weight: 2,
            opacity: isActivated ? 1 : 0.5,
            fillOpacity: isActivated ? 0.9 : 0.3,
        });
        marker.bindTooltip(
            `${isOrigin ? '🏠 起源: ' : ''}${port.name_zh || port.name}${isActivated ? ' ✓' : ''}`,
            { direction: 'top' }
        );
        techDiffusionLayer.addLayer(marker);
    });

    const currentYear = Math.round(tech.estimated_start_year +
        (tech.estimated_end_year - tech.estimated_start_year) * progress);

    const techZh = TECH_PRESETS.find(t => t.value === tech.tech_name)?.label || tech.tech_name_zh;

    document.getElementById('tech-diffusion-info').innerHTML = `
        <p>技术: <strong>${techZh}</strong></p>
        <p>当前年份: <strong style="color:var(--accent-gold)">${currentYear}年</strong></p>
        <p>起源: ${tech.origin_port_name}</p>
        <p>扩散速度: ${tech.diffusion_speed_km_yr} km/年</p>
        <p>进度: ${(progress * 100).toFixed(0)}%</p>
    `;

    if (AppState.layers.cargoSpread) {
        techDiffusionLayer.addTo(map);
    }
}

function pauseTechDiffusionAnimation() {
    techAnimationPlaying = false;
    if (techAnimationId) {
        cancelAnimationFrame(techAnimationId);
        techAnimationId = null;
    }
    document.getElementById('btn-tech-play').disabled = false;
    document.getElementById('btn-tech-pause').disabled = true;
}

function stopTechDiffusionAnimation() {
    techAnimationPlaying = false;
    techAnimationProgress = 0;
    if (techAnimationId) {
        cancelAnimationFrame(techAnimationId);
        techAnimationId = null;
    }
    const playBtn = document.getElementById('btn-tech-play');
    const pauseBtn = document.getElementById('btn-tech-pause');
    if (playBtn) playBtn.disabled = false;
    if (pauseBtn) pauseBtn.disabled = true;
}

function toggleCargoSpreadLayer(visible) {
    if (visible) {
        cargoSpreadLayer.addTo(map);
        techDiffusionLayer.addTo(map);
    } else {
        map.removeLayer(cargoSpreadLayer);
        map.removeLayer(techDiffusionLayer);
    }
}

function addCargoSpreadLayerControl() {
    const layerControls = document.querySelector('.layer-controls');
    if (!layerControls) return;

    const label = document.createElement('label');
    label.innerHTML = '<input type="checkbox" id="layer-cargo-spread" /> 货物传播';
    layerControls.appendChild(label);

    document.getElementById('layer-cargo-spread').addEventListener('change', (e) => {
        AppState.layers.cargoSpread = e.target.checked;
        toggleCargoSpreadLayer(e.target.checked);
    });
}

document.addEventListener('DOMContentLoaded', () => {
    initCargoSpread();
    addCargoSpreadLayerControl();
});
