const API_BASE = '/api';

const AppState = {
    yearStart: -1000,
    yearEnd: 1800,
    season: '',
    cargoType: '',
    shipType: '',
    region: '',
    stormOnly: false,
    voyages: [],
    ports: [],
    networkData: null,
    stormData: null,
    heatmapLayer: null,
    layers: {
        routes: true,
        ports: true,
        storms: true,
        network: false,
        heatmap: false,
        currents: false,
    },
};

const SEASON_COLORS = {
    spring: '#44ff88',
    summer: '#ffd700',
    autumn: '#ff8844',
    winter: '#4a9eff',
};

const CARGO_COLORS = {
    grain: '#c8a96e',
    olive_oil: '#a4c639',
    wine: '#8b2252',
    spices: '#ff6347',
    silk: '#da70d6',
    ceramics: '#87ceeb',
    ivory: '#fffff0',
    gold: '#ffd700',
    timber: '#8b4513',
    salt: '#f0f0f0',
    textiles: '#dda0dd',
    glass: '#00ced1',
    incense: '#daa520',
    precious_stones: '#ee82ee',
    copper: '#b87333',
};

const SEASON_ZH = {
    spring: '春季',
    summer: '夏季',
    autumn: '秋季',
    winter: '冬季',
};

const SHIP_ZH = {
    trireme: '三列桨座战船',
    merchant_round_ship: '商船',
    dhow: '单桅三角帆船',
    junk: '中国帆船',
    carrack: '卡拉维尔帆船',
    longship: '长船',
    galley: '桨帆船',
    treasure_ship: '宝船',
};

const CARGO_ZH = {
    grain: '粮食',
    olive_oil: '橄榄油',
    wine: '葡萄酒',
    spices: '香料',
    silk: '丝绸',
    ceramics: '陶瓷',
    ivory: '象牙',
    gold: '黄金',
    timber: '木材',
    salt: '盐',
    textiles: '纺织品',
    glass: '玻璃',
    incense: '乳香',
    precious_stones: '宝石',
    copper: '铜',
};

async function apiFetch(endpoint, params = {}) {
    const url = new URL(API_BASE + endpoint, window.location.origin);
    Object.entries(params).forEach(([k, v]) => {
        if (v !== '' && v !== null && v !== undefined) {
            url.searchParams.set(k, v);
        }
    });
    const resp = await fetch(url.toString());
    if (!resp.ok) throw new Error(`API error: ${resp.status}`);
    return resp.json();
}

function showLoading() {
    document.getElementById('loading').classList.remove('hidden');
}

function hideLoading() {
    document.getElementById('loading').classList.add('hidden');
}

function updateStats(stats) {
    document.getElementById('stat-voyages').textContent = `航线: ${stats.total_voyages || 0}`;
    document.getElementById('stat-ports').textContent = `港口: ${stats.total_ports || 0}`;
    document.getElementById('stat-storms').textContent = `风暴: ${stats.storm_encounters || 0}`;
}

async function loadInitialData() {
    try {
        const [portsResp, statsResp] = await Promise.all([
            apiFetch('/ports'),
            apiFetch('/stats'),
        ]);
        AppState.ports = portsResp.ports || [];
        updateStats(statsResp);
        return true;
    } catch (e) {
        console.error('Failed to load initial data:', e);
        return false;
    }
}

async function loadVoyages() {
    showLoading();
    try {
        const params = {
            year_start: AppState.yearStart,
            year_end: AppState.yearEnd,
        };
        if (AppState.season) params.season = AppState.season;
        if (AppState.cargoType) params.cargo_type = AppState.cargoType;
        if (AppState.shipType) params.ship_type = AppState.shipType;
        if (AppState.region) params.region = AppState.region;
        if (AppState.stormOnly) params.encountered_storm = true;

        const resp = await apiFetch('/voyages', params);
        AppState.voyages = resp.voyages || [];
        return AppState.voyages;
    } catch (e) {
        console.error('Failed to load voyages:', e);
        return [];
    } finally {
        hideLoading();
    }
}

function initEventListeners() {
    document.getElementById('btn-apply-filters').addEventListener('click', async () => {
        AppState.season = document.getElementById('filter-season').value;
        AppState.cargoType = document.getElementById('filter-cargo').value;
        AppState.shipType = document.getElementById('filter-ship').value;
        AppState.region = document.getElementById('filter-region').value;
        AppState.stormOnly = document.getElementById('filter-storm').checked;
        await loadVoyages();
        renderMapLayers();
    });

    document.getElementById('btn-network').addEventListener('click', async () => {
        await loadNetworkAnalysis();
    });

    document.getElementById('btn-community').addEventListener('click', () => {
        if (AppState.networkData) {
            renderCommunityColors();
        } else {
            document.getElementById('network-info').innerHTML = '<p style="color:var(--accent-red)">请先计算贸易网络</p>';
        }
    });

    document.getElementById('btn-hubs').addEventListener('click', () => {
        if (AppState.networkData) {
            renderHubPorts();
        } else {
            document.getElementById('network-info').innerHTML = '<p style="color:var(--accent-red)">请先计算贸易网络</p>';
        }
    });

    document.getElementById('btn-storm').addEventListener('click', async () => {
        await loadStormRisk();
    });

    document.getElementById('btn-heatmap').addEventListener('click', () => {
        toggleHeatmap();
    });

    document.getElementById('panel-close').addEventListener('click', () => {
        closeVoyageDetail();
    });

    document.getElementById('layer-routes').addEventListener('change', (e) => {
        AppState.layers.routes = e.target.checked;
        renderMapLayers();
    });
    document.getElementById('layer-ports').addEventListener('change', (e) => {
        AppState.layers.ports = e.target.checked;
        renderMapLayers();
    });
    document.getElementById('layer-storms').addEventListener('change', (e) => {
        AppState.layers.storms = e.target.checked;
        renderMapLayers();
    });
    document.getElementById('layer-network').addEventListener('change', (e) => {
        AppState.layers.network = e.target.checked;
        renderMapLayers();
    });
    document.getElementById('layer-heatmap').addEventListener('change', (e) => {
        AppState.layers.heatmap = e.target.checked;
        toggleHeatmap();
    });
    document.getElementById('layer-currents').addEventListener('change', (e) => {
        AppState.layers.currents = e.target.checked;
        renderMapLayers();
    });
}

document.addEventListener('DOMContentLoaded', async () => {
    initMap();
    initTimeline();
    initEventListeners();

    const ok = await loadInitialData();
    if (ok) {
        renderPorts();
        await loadVoyages();
        renderMapLayers();
    }
});
