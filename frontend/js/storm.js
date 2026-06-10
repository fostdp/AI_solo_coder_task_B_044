async function loadStormRisk() {
    showLoading();
    try {
        const modelType = document.getElementById('storm-model').value;
        const resp = await apiFetch('/storm-risk', {
            year_start: AppState.yearStart,
            year_end: AppState.yearEnd,
            model_type: modelType,
        });
        AppState.stormData = resp;

        const highRisks = resp.risks.filter(r => r.risk_score > 0.3)
            .sort((a, b) => b.risk_score - a.risk_score);

        document.getElementById('storm-info').innerHTML = `
            <p>模型: ${modelType === 'logistic_regression' ? '逻辑回归' : '随机森林'}</p>
            <p>航线风险数: ${resp.risks.length}</p>
            <p>高风险航线: ${highRisks.length}</p>
            <div style="margin-top:6px">
                ${highRisks.slice(0, 8).map(r => `
                    <div class="risk-item">
                        ${r.departure_port_name} → ${r.arrival_port_name}
                        <br>季节: ${SEASON_ZH[r.season] || r.season} | 风险: ${(r.risk_score * 100).toFixed(1)}%
                        <br>样本: ${r.sample_size} | 置信度: ${(r.confidence * 100).toFixed(0)}%
                    </div>
                `).join('')}
            </div>
        `;

        if (resp.heatmap && resp.heatmap.length > 0) {
            renderStormHeatmap(resp.heatmap);
        }
    } catch (e) {
        console.error('Storm risk analysis failed:', e);
        document.getElementById('storm-info').innerHTML = '<p style="color:var(--accent-red)">分析失败</p>';
    } finally {
        hideLoading();
    }
}

function renderStormHeatmap(points) {
    if (heatmapLayer) {
        map.removeLayer(heatmapLayer);
        heatmapLayer = null;
    }

    if (!AppState.layers.heatmap) return;

    const heatData = points.map(p => [p.lat, p.lon, p.intensity]);

    heatmapLayer = L.heatLayer(heatData, {
        radius: 25,
        blur: 15,
        maxZoom: 8,
        max: 1.0,
        gradient: {
            0.0: '#0000ff',
            0.25: '#00ffff',
            0.5: '#ffff00',
            0.75: '#ff8800',
            1.0: '#ff0000',
        },
    });

    heatmapLayer.addTo(map);
}

function toggleHeatmap() {
    if (AppState.layers.heatmap) {
        if (AppState.stormData && AppState.stormData.heatmap) {
            renderStormHeatmap(AppState.stormData.heatmap);
        } else {
            loadStormRisk();
        }
    } else {
        if (heatmapLayer) {
            map.removeLayer(heatmapLayer);
            heatmapLayer = null;
        }
    }
}
