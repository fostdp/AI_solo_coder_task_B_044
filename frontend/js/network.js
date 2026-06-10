async function loadNetworkAnalysis() {
    showLoading();
    try {
        const resp = await apiFetch('/network', {
            year_start: AppState.yearStart,
            year_end: AppState.yearEnd,
        });
        AppState.networkData = resp;

        document.getElementById('layer-network').checked = true;
        AppState.layers.network = true;
        renderNetworkEdges();
        renderMapLayers();

        const hubs = resp.nodes.filter(n => n.is_hub);
        const communities = new Set(resp.nodes.map(n => n.community_id)).size;
        document.getElementById('network-info').innerHTML = `
            <p>港口数: ${resp.nodes.length}</p>
            <p>航线数: ${resp.edges.length}</p>
            <p>贸易社区: ${communities}</p>
            <p>核心枢纽: ${hubs.length}</p>
            <div style="margin-top:6px">
                ${hubs.sort((a,b) => b.betweenness_centrality - a.betweenness_centrality)
                    .slice(0, 5)
                    .map(n => `<div class="hub-item">⭐ ${n.port_name_zh || n.port_name} (BC: ${n.betweenness_centrality.toFixed(4)})</div>`)
                    .join('')}
            </div>
        `;
    } catch (e) {
        console.error('Network analysis failed:', e);
        document.getElementById('network-info').innerHTML = '<p style="color:var(--accent-red)">分析失败</p>';
    } finally {
        hideLoading();
    }
}
