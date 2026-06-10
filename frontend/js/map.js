let map = null;
let routeLayer = null;
let portMarkers = null;
let stormMarkers = null;
let networkLayer = null;
let heatmapLayer = null;
let currentArrows = null;

function initMap() {
    map = L.map('map', {
        center: [25, 40],
        zoom: 3,
        minZoom: 2,
        maxZoom: 10,
        zoomControl: true,
        attributionControl: true,
    });

    L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png', {
        attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OSM</a> &copy; <a href="https://carto.com/">CARTO</a>',
        subdomains: 'abcd',
        maxZoom: 19,
    }).addTo(map);

    routeLayer = L.layerGroup().addTo(map);
    portMarkers = L.layerGroup().addTo(map);
    stormMarkers = L.layerGroup().addTo(map);
    networkLayer = L.layerGroup();
    heatmapLayer = null;
    currentArrows = L.layerGroup();

    addLegend();
}

function addLegend() {
    const legend = L.control({ position: 'bottomright' });
    legend.onAdd = function () {
        const div = L.DomUtil.create('div', 'season-legend');
        div.innerHTML = `
            <div class="legend-title">季节颜色</div>
            <div class="legend-item"><div class="legend-color" style="background:#44ff88"></div>春季</div>
            <div class="legend-item"><div class="legend-color" style="background:#ffd700"></div>夏季</div>
            <div class="legend-item"><div class="legend-color" style="background:#ff8844"></div>秋季</div>
            <div class="legend-item"><div class="legend-color" style="background:#4a9eff"></div>冬季</div>
            <div class="legend-title" style="margin-top:6px">风暴标记</div>
            <div class="legend-item" style="color:#ff4444">✕ 遇难点</div>
        `;
        return div;
    };
    legend.addTo(map);
}

function getRouteColor(voyage) {
    if (AppState.layers.network && AppState.networkData) {
        return getNetworkColor(voyage);
    }
    return SEASON_COLORS[voyage.season] || '#ffffff';
}

function getNetworkColor(voyage) {
    const comm = getCommunityForRoute(voyage);
    const communityColors = [
        '#ff6b6b', '#4ecdc4', '#45b7d1', '#f9ca24',
        '#6c5ce7', '#a8e6cf', '#fd79a8', '#fdcb6e',
    ];
    return communityColors[comm % communityColors.length];
}

function getCommunityForRoute(voyage) {
    if (!AppState.networkData) return 0;
    const node = AppState.networkData.nodes.find(
        n => n.port_id === voyage.departure_port_id
    );
    return node ? node.community_id : 0;
}

function renderPorts() {
    portMarkers.clearLayers();
    if (!AppState.layers.ports) return;

    AppState.ports.forEach(port => {
        const marker = L.circleMarker([port.lat, port.lon], {
            radius: 5,
            fillColor: '#ffd700',
            color: '#b8960a',
            weight: 1,
            opacity: 0.9,
            fillOpacity: 0.7,
        });
        marker.bindTooltip(`${port.name_zh || port.name}<br>${port.region || ''}`, {
            className: 'port-tooltip',
            direction: 'top',
        });
        portMarkers.addLayer(marker);
    });
}

function renderVoyages() {
    routeLayer.clearLayers();
    stormMarkers.clearLayers();

    if (!AppState.layers.routes && !AppState.layers.storms) return;

    AppState.voyages.forEach(voyage => {
        if (AppState.layers.routes) {
            const color = getRouteColor(voyage);
            let routePoints;

            if (voyage.route_points && Array.isArray(voyage.route_points)) {
                routePoints = voyage.route_points.map(p => [p[1], p[0]]);
            } else {
                routePoints = [
                    [voyage.departure_lat, voyage.departure_lon],
                    [voyage.arrival_lat, voyage.arrival_lon],
                ];
            }

            const polyline = L.polyline(routePoints, {
                color: color,
                weight: 1.5,
                opacity: 0.6,
                smoothFactor: 1,
            });

            polyline.on('click', () => {
                showVoyageDetail(voyage);
            });

            polyline.on('mouseover', function () {
                this.setStyle({ weight: 3, opacity: 1 });
            });
            polyline.on('mouseout', function () {
                this.setStyle({ weight: 1.5, opacity: 0.6 });
            });

            routeLayer.addLayer(polyline);
        }

        if (AppState.layers.storms && voyage.encountered_storm) {
            let stormLat, stormLon;
            if (voyage.route_points && Array.isArray(voyage.route_points)) {
                const mid = Math.floor(voyage.route_points.length / 2);
                stormLon = voyage.route_points[mid][0];
                stormLat = voyage.route_points[mid][1];
            } else {
                stormLat = (voyage.departure_lat + voyage.arrival_lat) / 2;
                stormLon = (voyage.departure_lon + voyage.arrival_lon) / 2;
            }

            const stormIcon = L.divIcon({
                html: '<div style="color:#ff4444;font-size:16px;font-weight:bold;text-shadow:0 0 4px #ff0000">✕</div>',
                className: 'storm-marker',
                iconSize: [16, 16],
                iconAnchor: [8, 8],
            });

            const marker = L.marker([stormLat, stormLon], { icon: stormIcon });
            marker.bindTooltip(
                `⚡ 风暴遇难<br>${voyage.departure_port} → ${voyage.arrival_port}<br>${SEASON_ZH[voyage.season] || voyage.season}`,
                { direction: 'top' }
            );
            marker.on('click', () => showVoyageDetail(voyage));
            stormMarkers.addLayer(marker);
        }
    });
}

function renderNetworkEdges() {
    networkLayer.clearLayers();
    if (!AppState.layers.network || !AppState.networkData) return;

    const edges = AppState.networkData.edges;
    const nodes = AppState.networkData.nodes;
    const nodeMap = {};
    nodes.forEach(n => { nodeMap[n.port_id] = n; });

    const maxWeight = Math.max(...edges.map(e => e.weight), 1);

    edges.forEach(edge => {
        const src = nodeMap[edge.source];
        const tgt = nodeMap[edge.target];
        if (!src || !tgt) return;

        const comm = src.community_id;
        const communityColors = [
            '#ff6b6b', '#4ecdc4', '#45b7d1', '#f9ca24',
            '#6c5ce7', '#a8e6cf', '#fd79a8', '#fdcb6e',
        ];
        const color = communityColors[comm % communityColors.length];
        const weight = 1 + (edge.weight / maxWeight) * 5;

        const line = L.polyline([[src.lat, src.lon], [tgt.lat, tgt.lon]], {
            color: color,
            weight: weight,
            opacity: 0.5,
            dashArray: '4 4',
        });
        networkLayer.addLayer(line);
    });

    nodes.forEach(node => {
        const radius = 3 + node.betweenness_centrality * 50;
        const marker = L.circleMarker([node.lat, node.lon], {
            radius: Math.min(radius, 15),
            fillColor: node.is_hub ? '#ffd700' : '#4a9eff',
            color: node.is_hub ? '#b8960a' : '#2a6ecf',
            weight: node.is_hub ? 2 : 1,
            opacity: 0.9,
            fillOpacity: 0.7,
        });
        marker.bindTooltip(
            `${node.port_name_zh || node.port_name}<br>中介中心性: ${node.betweenness_centrality.toFixed(4)}<br>贸易流量: ${node.trade_flow.toFixed(1)}`,
            { direction: 'top' }
        );
        networkLayer.addLayer(marker);
    });

    networkLayer.addTo(map);
}

function renderMapLayers() {
    renderPorts();
    renderVoyages();
    renderNetworkEdges();

    if (AppState.layers.network) {
        networkLayer.addTo(map);
    } else {
        map.removeLayer(networkLayer);
    }

    if (AppState.layers.currents) {
        currentArrows.addTo(map);
    } else {
        map.removeLayer(currentArrows);
    }
}

function renderCommunityColors() {
    if (!AppState.networkData) return;
    renderVoyages();
    document.getElementById('network-info').innerHTML =
        AppState.networkData.nodes
            ? `<p>识别出 ${new Set(AppState.networkData.nodes.map(n => n.community_id)).size} 个贸易社区</p>` +
              AppState.networkData.nodes
                  .sort((a, b) => a.community_id - b.community_id)
                  .map(n => `<div class="hub-item" style="border-left-color:${
                      ['#ff6b6b','#4ecdc4','#45b7d1','#f9ca24','#6c5ce7','#a8e6cf','#fd79a8','#fdcb6e'][n.community_id % 8]
                  }">社区${n.community_id}: ${n.port_name_zh || n.port_name}</div>`)
                  .join('')
            : '';
}

function renderHubPorts() {
    if (!AppState.networkData) return;
    const hubs = AppState.networkData.nodes.filter(n => n.is_hub)
        .sort((a, b) => b.betweenness_centrality - a.betweenness_centrality);
    document.getElementById('network-info').innerHTML =
        `<p>核心枢纽港: ${hubs.length} 个</p>` +
        hubs.map(n => `<div class="hub-item">⭐ ${n.port_name_zh || n.port_name} (BC: ${n.betweenness_centrality.toFixed(4)})</div>`).join('');
}
