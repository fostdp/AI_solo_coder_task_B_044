/**
 * 港口兴衰影响因素分析模块
 * 提供面板回归、Granger因果检验、因子权重排名、历史事件时间线
 */

const PortRiseFallComponent = (function () {
    let eventMarkers = null;
    let currentData = null;

    const EVENT_ICONS = {
        war: '⚔️',
        regime_change: '👑',
        plague: '☠️',
        technology: '⚙️',
        expedition: '🧭',
    };

    const EVENT_COLORS = {
        war: '#ff4444',
        regime_change: '#ffd700',
        plague: '#aa66ff',
        technology: '#44ff88',
        expedition: '#4a9eff',
    };

    function init() {
        eventMarkers = L.layerGroup();
        populatePortSelect();
        initEventListeners();
    }

    function populatePortSelect() {
        const select = document.getElementById('port-select');
        if (!select || !AppState.ports) return;

        select.innerHTML = '<option value="">请选择港口</option>';

        AppState.ports
            .sort((a, b) => {
                const nameA = a.name_zh || a.name;
                const nameB = b.name_zh || b.name;
                return nameA.localeCompare(nameB, 'zh-CN');
            })
            .forEach(port => {
                const option = document.createElement('option');
                option.value = port.id;
                option.textContent = port.name_zh || port.name;
                select.appendChild(option);
            });
    }

    function initEventListeners() {
        const btnAnalyze = document.getElementById('btn-port-rise-fall');
        if (btnAnalyze) {
            btnAnalyze.addEventListener('click', handleAnalyze);
        }

        const portSelect = document.getElementById('port-select');
        if (portSelect) {
            portSelect.addEventListener('change', handlePortChange);
        }
    }

    function handlePortChange() {
        const portId = document.getElementById('port-select').value;
        if (portId) {
            loadPortRiseFallAnalysis();
        }
    }

    function handleAnalyze() {
        loadPortRiseFallAnalysis();
    }

    async function loadPortRiseFallAnalysis() {
        const portId = document.getElementById('port-select').value;
        if (!portId) {
            document.getElementById('rise-fall-result').innerHTML =
                '<p style="color:var(--accent-red)">请先选择港口</p>';
            return;
        }

        showLoading();
        try {
            const params = {
                port_id: parseInt(portId),
                year_start: AppState.yearStart,
                year_end: AppState.yearEnd,
            };

            const resp = await apiFetch('/insights/port-rise-fall', params);
            currentData = resp;

            renderRegressionResults(resp);
            renderGrangerResults(resp);
            renderFactorWeights(resp);
            renderEventTimeline(resp);
            renderEventMarkers(resp);

            if (resp.regression_results && resp.regression_results.length > 0) {
                const first = resp.regression_results[0];
                document.getElementById('rise-fall-summary').innerHTML = `
                    <p>港口: ${first.port_name}</p>
                    <p>观测期: ${first.period_start} — ${first.period_end}年</p>
                    <p>样本数: ${first.n_observations}</p>
                    <p>R²: ${(first.r_squared * 100).toFixed(2)}%</p>
                `;
            }
        } catch (e) {
            console.error('Port rise-fall analysis failed:', e);
            document.getElementById('rise-fall-result').innerHTML =
                '<p style="color:var(--accent-red)">分析失败，请检查数据</p>';
        } finally {
            hideLoading();
        }
    }

    function renderRegressionResults(data) {
        const container = document.getElementById('regression-results');
        if (!container) return;

        if (!data.regression_results || data.regression_results.length === 0) {
            container.innerHTML = '<p class="no-data">暂无回归结果</p>';
            return;
        }

        const result = data.regression_results[0];

        let html = `
            <div class="regression-header">
                <span class="regression-title">面板回归分析</span>
                <span class="regression-r2">R² = ${(result.r_squared * 100).toFixed(2)}%</span>
            </div>
            <table class="info-table coefficient-table">
                <thead>
                    <tr>
                        <th>变量</th>
                        <th>系数</th>
                        <th>标准误</th>
                        <th>t值</th>
                        <th>p值</th>
                    </tr>
                </thead>
                <tbody>
        `;

        result.coefficients.forEach(coef => {
            const sigClass = coef.is_significant ? 'significant' : '';
            html += `
                <tr class="${sigClass}">
                    <td>${coef.variable_zh || coef.variable}</td>
                    <td>${coef.coefficient.toFixed(4)}</td>
                    <td>${coef.standard_error.toFixed(4)}</td>
                    <td>${coef.t_statistic.toFixed(3)}</td>
                    <td>${coef.p_value.toFixed(4)}</td>
                </tr>
            `;
        });

        html += `
                </tbody>
            </table>
            <div class="regression-footer">
                <span>调整R²: ${(result.adj_r_squared * 100).toFixed(2)}%</span>
                <span>F统计量: ${result.f_statistic.toFixed(3)}</span>
                <span>F-p值: ${result.p_value.toFixed(4)}</span>
            </div>
        `;

        container.innerHTML = html;
    }

    function renderGrangerResults(data) {
        const container = document.getElementById('granger-results');
        if (!container) return;

        if (!data.granger_results || data.granger_results.length === 0) {
            container.innerHTML = '<p class="no-data">暂无Granger因果检验结果</p>';
            return;
        }

        let html = `
            <div class="section-title">Granger因果检验</div>
            <table class="info-table granger-table">
                <thead>
                    <tr>
                        <th>原因变量</th>
                        <th>结果变量</th>
                        <th>F值</th>
                        <th>p值</th>
                        <th>显著</th>
                    </tr>
                </thead>
                <tbody>
        `;

        data.granger_results.forEach(item => {
            const sigClass = item.is_significant ? 'significant' : '';
            const sigIcon = item.is_significant ? '✓' : '✗';
            const directionIcon = item.direction === 'positive' ? '↑' : '↓';
            html += `
                <tr class="${sigClass}">
                    <td>${item.cause_variable_zh || item.cause_variable}</td>
                    <td>${item.effect_variable_zh || item.effect_variable} ${directionIcon}</td>
                    <td>${item.f_statistic.toFixed(3)}</td>
                    <td>${item.p_value.toFixed(4)}</td>
                    <td>${sigIcon}</td>
                </tr>
            `;
        });

        html += `
                </tbody>
            </table>
        `;

        container.innerHTML = html;
    }

    function renderFactorWeights(data) {
        const container = document.getElementById('factor-weights');
        if (!container) return;

        if (!data.factor_weights || data.factor_weights.length === 0) {
            container.innerHTML = '<p class="no-data">暂无因子权重数据</p>';
            return;
        }

        const maxCoef = Math.max(...data.factor_weights.map(w => w.avg_coefficient), 0.001);

        let html = '<div class="section-title">因子权重排名</div>';
        html += '<div class="factor-bar-chart">';

        data.factor_weights.forEach((factor, index) => {
            const widthPct = (factor.avg_coefficient / maxCoef) * 100;
            const sigPct = (factor.significance_rate * 100).toFixed(0);
            const barColor = factor.significance_rate >= 0.5
                ? 'var(--accent-gold)'
                : 'var(--accent-blue)';

            html += `
                <div class="factor-bar-item">
                    <div class="factor-bar-label">
                    <span class="factor-rank">${factor.importance_rank}</span>
                    <span class="factor-name">${factor.factor_zh || factor.factor}</span>
                </div>
                <div class="factor-bar-track">
                    <div class="factor-bar-fill" style="width: ${widthPct}%; background: ${barColor}"></div>
                </div>
                <div class="factor-bar-value">
                    <span>${factor.avg_coefficient.toFixed(4)}</span>
                    <span class="factor-sig-rate" title="显著性比例">${sigPct}%</span>
                </div>
            </div>
            `;
        });

        html += '</div>';
        container.innerHTML = html;
    }

    function renderEventTimeline(data) {
        const container = document.getElementById('event-timeline');
        if (!container) return;

        if (!data.historical_events || data.historical_events.length === 0) {
            container.innerHTML = '<p class="no-data">暂无历史事件</p>';
            return;
        }

        const sortedEvents = [...data.historical_events].sort((a, b) => a.start_year - b.start_year);

        let html = '<div class="section-title">历史事件时间线</div>';
        html += '<div class="event-timeline">';

        sortedEvents.forEach((event, index) => {
            const icon = EVENT_ICONS[event.event_type] || '📌';
            const color = EVENT_COLORS[event.event_type] || '#888';
            const endYear = event.end_year ? ` — ${event.end_year}` : '';
            const severity = event.severity ? `（严重度: ${event.severity}` : '';

            html += `
                <div class="timeline-item" data-event-id="${event.id}" data-lat="${event.lat || ''}" data-lon="${event.lon || ''}">
                    <div class="timeline-marker" style="border-color: ${color}">
                        <span class="timeline-icon">${icon}</span>
                    </div>
                    <div class="timeline-content">
                        <div class="timeline-year">${event.start_year}年${endYear}</div>
                        <div class="timeline-title">${event.event_name_zh || event.event_name}</div>
                        <div class="timeline-desc">${event.description || ''}${severity}</div>
                    </div>
                </div>
            `;
        });

        html += '</div>';
        container.innerHTML = html;

        container.querySelectorAll('.timeline-item').forEach(item => {
            item.addEventListener('click', () => {
                const eventId = item.dataset.eventId;
                const lat = parseFloat(item.dataset.lat);
                const lon = parseFloat(item.dataset.lon);
                highlightEvent(eventId, lat, lon);
            });
        });
    }

    function renderEventMarkers(data) {
        if (!eventMarkers) return;
        eventMarkers.clearLayers();

        if (!data.historical_events || data.historical_events.length === 0) return;

        data.historical_events.forEach(event => {
            if (!event.lat || !event.lon) return;

            const icon = EVENT_ICONS[event.event_type] || '📌';
            const color = EVENT_COLORS[event.event_type] || '#ff4444';

            const eventIcon = L.divIcon({
                html: `<div class="event-marker-icon" style="color:${color}">${icon}</div>`,
                className: 'event-marker',
                iconSize: [24, 24],
                iconAnchor: [12, 12],
            });

            const marker = L.marker([event.lat, event.lon], { icon: eventIcon });

            const endYear = event.end_year ? ` — ${event.end_year}` : '';
            marker.bindTooltip(
                `${event.event_name_zh || event.event_name}<br>${event.start_year}年${endYear}`,
                { direction: 'top' }
            );

            marker.on('click', () => {
                highlightEvent(event.id, event.lat, event.lon);
            });

            eventMarkers.addLayer(marker);
        });

        if (AppState.layers.events !== false) {
            eventMarkers.addTo(map);
        }
    }

    function highlightEvent(eventId, lat, lon) {
        if (lat && lon) {
            map.flyTo([lat, lon], 6, { duration: 1 });
        }

        document.querySelectorAll('.timeline-item').forEach(item => {
            item.classList.remove('active');
            if (item.dataset.eventId == eventId) {
                item.classList.add('active');
                item.scrollIntoView({ behavior: 'smooth', block: 'center' });
            }
        });

        if (eventMarkers) {
            eventMarkers.eachLayer(marker => {
                const icon = marker.getIcon();
                if (icon && icon.options && icon.options.html) {
                }
            });
        }
    }

    function onTimeRangeChange() {
        const portSelect = document.getElementById('port-select');
        if (portSelect && portSelect.value && currentData) {
            loadPortRiseFallAnalysis();
        }
    }

    function toggleEventLayer(show) {
        if (!eventMarkers) return;
        if (show) {
            eventMarkers.addTo(map);
        } else {
            map.removeLayer(eventMarkers);
        }
    }

    function refreshPortList() {
        populatePortSelect();
    }

    return {
        init: init,
        loadPortRiseFallAnalysis: loadPortRiseFallAnalysis,
        onTimeRangeChange: onTimeRangeChange,
        toggleEventLayer: toggleEventLayer,
        refreshPortList: refreshPortList,
        getCurrentData: () => currentData,
    };
})();

window.PortRiseFallComponent = PortRiseFallComponent;
