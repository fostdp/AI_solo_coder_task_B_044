function showVoyageDetail(voyage) {
    const panel = document.getElementById('voyage-panel');
    const content = document.getElementById('panel-content');
    panel.classList.remove('hidden');

    content.innerHTML = `
        <div class="detail-row">
            <span class="detail-label">出发港</span>
            <span class="detail-value">${voyage.departure_port_zh || voyage.departure_port}</span>
        </div>
        <div class="detail-row">
            <span class="detail-label">目的港</span>
            <span class="detail-value">${voyage.arrival_port_zh || voyage.arrival_port}</span>
        </div>
        <div class="detail-row">
            <span class="detail-label">年份</span>
            <span class="detail-value">${formatYear(voyage.voyage_year)}</span>
        </div>
        <div class="detail-row">
            <span class="detail-label">季节</span>
            <span class="detail-value">${SEASON_ZH[voyage.season] || voyage.season}</span>
        </div>
        <div class="detail-row">
            <span class="detail-label">船只类型</span>
            <span class="detail-value">${SHIP_ZH[voyage.ship_type] || voyage.ship_type}</span>
        </div>
        <div class="detail-row">
            <span class="detail-label">货物类型</span>
            <span class="detail-value">${CARGO_ZH[voyage.cargo_type] || voyage.cargo_type}</span>
        </div>
        <div class="detail-row">
            <span class="detail-label">遭遇风暴</span>
            <span class="detail-value ${voyage.encountered_storm ? 'storm-yes' : 'storm-no'}">
                ${voyage.encountered_storm ? '⚡ 是' : '✓ 否'}
            </span>
        </div>
    `;

    if (voyage.route_points && Array.isArray(voyage.route_points)) {
        const routeInfo = document.createElement('div');
        routeInfo.className = 'detail-row';
        routeInfo.innerHTML = `
            <span class="detail-label">航线路点数</span>
            <span class="detail-value">${voyage.route_points.length}</span>
        `;
        content.appendChild(routeInfo);
    }
}

function formatYear(year) {
    if (year < 0) {
        return '公元前' + Math.abs(year) + '年';
    }
    return '公元' + year + '年';
}

function closeVoyageDetail() {
    document.getElementById('voyage-panel').classList.add('hidden');
}
