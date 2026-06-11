const ModernComparison = (function () {
    const VIEW_MODES = {
        ANCIENT: 'ancient',
        MODERN: 'modern',
        BOTH: 'both',
    };

    const ANCIENT_RISK_COLORS = {
        very_low: '#90EE90',
        low: '#228B22',
        medium: '#FFD700',
        high: '#FF8C00',
        very_high: '#FF0000',
    };

    const MODERN_RISK_COLORS = {
        very_low: '#ADD8E6',
        low: '#4169E1',
        medium: '#00CED1',
        high: '#9932CC',
        very_high: '#4B0082',
    };

    const RISK_LEVEL_ZH = {
        very_low: '极低',
        low: '低',
        medium: '中',
        high: '高',
        very_high: '极高',
    };

    let ancientHeatmapLayer = null;
    let modernHeatmapLayer = null;
    let ancientHighRiskLayer = null;
    let modernHighRiskLayer = null;
    let comparisonLegend = null;

    function initModernComparison() {
        ancientHighRiskLayer = L.layerGroup();
        modernHighRiskLayer = L.layerGroup();

        document.getElementById('btn-modern-compare').addEventListener('click', async () => {
            await loadModernComparison();
        });

        document.querySelectorAll('input[name="compare-mode"]').forEach(radio => {
            radio.addEventListener('change', (e) => {
                AppState.modernCompare.viewMode = e.target.value;
                updateViewMode();
            });
        });

        document.getElementById('modern-opacity').addEventListener('input', (e) => {
            AppState.modernCompare.modernOpacity = parseFloat(e.target.value);
            document.getElementById('modern-opacity-value').textContent =
                Math.round(AppState.modernCompare.modernOpacity * 100) + '%';
            updateModernHeatmapOpacity();
        });
    }

    async function loadModernComparison() {
        showLoading();
        try {
            const modelType = document.getElementById('compare-model').value;
            const resp = await apiFetch('/insights/modern-comparison', {
                year_start: AppState.yearStart,
                year_end: AppState.yearEnd,
                model_type: modelType,
            });

            AppState.modernCompare = {
                ...AppState.modernCompare,
                data: resp,
                loaded: true,
                viewMode: AppState.modernCompare.viewMode || VIEW_MODES.BOTH,
                modernOpacity: AppState.modernCompare.modernOpacity || 0.6,
            };

            renderComparisonSummary(resp.comparison_summary);
            renderHeatmaps(resp.heatmap_ancient, resp.heatmap_modern);
            renderHighRiskRoutes(resp.ancient_risks, resp.modern_risks);
            updateViewMode();
            updateModernOpacitySlider();

            if (!comparisonLegend) {
                addComparisonLegend();
            }
        } catch (e) {
            console.error('Modern comparison analysis failed:', e);
            document.getElementById('comparison-info').innerHTML =
                '<p style="color:var(--accent-red)">分析失败</p>';
        } finally {
            hideLoading();
        }
    }

    function renderComparisonSummary(summary) {
        const infoEl = document.getElementById('comparison-info');
        const riskReductionClass = summary.risk_reduction_pct >= 0
            ? 'reduction-positive'
            : 'reduction-negative';
        const riskReductionSign = summary.risk_reduction_pct >= 0 ? '↓' : '↑';

        infoEl.innerHTML = `
            <div class="comparison-summary">
                <div class="summary-card">
                    <div class="summary-label">古代平均风险</div>
                    <div class="summary-value ancient-value">
                        ${(summary.avg_ancient_risk * 100).toFixed(1)}%
                    </div>
                </div>
                <div class="summary-card">
                    <div class="summary-label">现代平均风险</div>
                    <div class="summary-value modern-value">
                        ${(summary.avg_modern_risk * 100).toFixed(1)}%
                    </div>
                </div>
                <div class="summary-card highlight">
                    <div class="summary-label">风险降低</div>
                    <div class="summary-value ${riskReductionClass}">
                        ${riskReductionSign} ${Math.abs(summary.risk_reduction_pct).toFixed(1)}%
                    </div>
                </div>
            </div>
            <div class="comparison-stats">
                <div class="stat-row">
                    <span class="stat-label">高风险航线(古代)</span>
                    <span class="stat-value ancient-value">${summary.high_risk_routes_ancient}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">高风险航线(现代)</span>
                    <span class="stat-value modern-value">${summary.high_risk_routes_modern}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">最危险区域(古代)</span>
                    <span class="stat-value">${summary.most_dangerous_region_ancient}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">最危险区域(现代)</span>
                    <span class="stat-value">${summary.most_dangerous_region_modern}</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">古今相关系数</span>
                    <span class="stat-value">${summary.correlation_coefficient.toFixed(3)}</span>
                </div>
            </div>
        `;
    }

    function renderHeatmaps(ancientPoints, modernPoints) {
        if (ancientHeatmapLayer) {
            map.removeLayer(ancientHeatmapLayer);
            ancientHeatmapLayer = null;
        }
        if (modernHeatmapLayer) {
            map.removeLayer(modernHeatmapLayer);
            modernHeatmapLayer = null;
        }

        const ancientData = ancientPoints.map(p => [p.lat, p.lon, p.intensity]);
        ancientHeatmapLayer = L.heatLayer(ancientData, {
            radius: 25,
            blur: 15,
            maxZoom: 8,
            max: 1.0,
            gradient: {
                0.0: '#FFFFE0',
                0.2: '#FFE4B5',
                0.4: '#FFA500',
                0.6: '#FF6347',
                0.8: '#FF0000',
                1.0: '#8B0000',
            },
        });

        const modernData = modernPoints.map(p => [p.lat, p.lon, p.intensity]);
        modernHeatmapLayer = L.heatLayer(modernData, {
            radius: 25,
            blur: 15,
            maxZoom: 8,
            max: 1.0,
            gradient: {
                0.0: '#E0FFFF',
                0.2: '#ADD8E6',
                0.4: '#4169E1',
                0.6: '#9932CC',
                0.8: '#4B0082',
                1.0: '#191970',
            },
        });

        updateModernHeatmapOpacity();
    }

    function renderHighRiskRoutes(ancientRisks, modernRisks) {
        ancientHighRiskLayer.clearLayers();
        modernHighRiskLayer.clearLayers();

        const highAncient = ancientRisks.filter(r => r.risk_score > 0.3);
        highAncient.forEach(risk => {
            const routePoints = getRoutePointsFromRisk(risk);
            if (routePoints.length < 2) return;

            const polyline = L.polyline(routePoints, {
                color: ANCIENT_RISK_COLORS.very_high,
                weight: 3,
                opacity: 0.8,
                dashArray: '8, 4',
                smoothFactor: 1.5,
            });

            polyline.bindTooltip(
                `<b>古代高风险航线</b><br>
                 ${risk.departure_port_name} → ${risk.arrival_port_name}<br>
                 风险: ${(risk.risk_score * 100).toFixed(1)}%<br>
                 季节: ${SEASON_ZH[risk.season] || risk.season}`,
                { direction: 'top', sticky: true }
            );

            polyline.on('click', () => {
                showRouteComparisonDetail(risk, null);
            });

            ancientHighRiskLayer.addLayer(polyline);
        });

        const highModern = modernRisks.filter(r => r.risk_score > 0.3);
        highModern.forEach(risk => {
            const routePoints = risk.route_points
                ? risk.route_points.map(p => [p[1], p[0]])
                : [];
            if (routePoints.length < 2) return;

            const polyline = L.polyline(routePoints, {
                color: MODERN_RISK_COLORS.very_high,
                weight: 3,
                opacity: 0.8,
                smoothFactor: 1.5,
            });

            const ancientMatch = ancientRisks.find(a =>
                a.departure_port_id === risk.departure_port_id &&
                a.arrival_port_id === risk.arrival_port_id
            );

            const changePct = ancientMatch
                ? ((risk.risk_score - ancientMatch.risk_score) / ancientMatch.risk_score * 100)
                : null;
            const changeText = changePct !== null
                ? `<br>变化: ${changePct >= 0 ? '+' : ''}${changePct.toFixed(1)}%`
                : '';

            polyline.bindTooltip(
                `<b>现代高风险航线</b><br>
                 风险等级: ${RISK_LEVEL_ZH[risk.risk_level] || risk.risk_level}<br>
                 风险: ${(risk.risk_score * 100).toFixed(1)}%
                 ${changeText}`,
                { direction: 'top', sticky: true }
            );

            polyline.on('click', () => {
                showRouteComparisonDetail(ancientMatch, risk);
            });

            modernHighRiskLayer.addLayer(polyline);
        });
    }

    function getRoutePointsFromRisk(risk) {
        if (risk.route_points && Array.isArray(risk.route_points)) {
            return risk.route_points.map(p => [p[1], p[0]]);
        }
        const dep = AppState.ports.find(p => p.id === risk.departure_port_id);
        const arr = AppState.ports.find(p => p.id === risk.arrival_port_id);
        if (dep && arr && dep.lat && dep.lon && arr.lat && arr.lon) {
            return [[dep.lat, dep.lon], [arr.lat, arr.lon]];
        }
        return [];
    }

    function showRouteComparisonDetail(ancientRisk, modernRisk) {
        const panel = document.getElementById('voyage-panel');
        const content = document.getElementById('panel-content');

        const depName = ancientRisk?.departure_port_name || modernRisk?.departure_port_name || '未知';
        const arrName = ancientRisk?.arrival_port_name || modernRisk?.arrival_port_name || '未知';

        const ancientRiskPct = ancientRisk ? (ancientRisk.risk_score * 100).toFixed(1) : '-';
        const modernRiskPct = modernRisk ? (modernRisk.risk_score * 100).toFixed(1) : '-';
        const ancientLevel = ancientRisk ? getRiskLevel(ancientRisk.risk_score) : '-';
        const modernLevel = modernRisk ? (modernRisk.risk_level || '-') : '-';

        let changePct = '-';
        let changeClass = '';
        if (ancientRisk && modernRisk && ancientRisk.risk_score > 0) {
            const pct = ((modernRisk.risk_score - ancientRisk.risk_score) / ancientRisk.risk_score * 100);
            changePct = (pct >= 0 ? '+' : '') + pct.toFixed(1) + '%';
            changeClass = pct < 0 ? 'reduction-positive' : 'reduction-negative';
        }

        content.innerHTML = `
            <h3 style="color:var(--accent-gold);margin-bottom:12px">
                航线对比：${depName} → ${arrName}
            </h3>

            <div class="comparison-detail-grid">
                <div class="detail-block ancient-block">
                    <h4 style="color:#FF6347;margin-bottom:8px">🏛️ 古代</h4>
                    <div class="detail-row">
                        <span class="detail-label">风险评分</span>
                        <span class="detail-value">${ancientRiskPct}%</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">风险等级</span>
                        <span class="detail-value">${RISK_LEVEL_ZH[ancientLevel] || ancientLevel}</span>
                    </div>
                    ${ancientRisk ? `
                        <div class="detail-row">
                            <span class="detail-label">季节</span>
                            <span class="detail-value">${SEASON_ZH[ancientRisk.season] || ancientRisk.season}</span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">样本量</span>
                            <span class="detail-value">${ancientRisk.sample_size || '-'}</span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">置信度</span>
                            <span class="detail-value">${ancientRisk.confidence ? (ancientRisk.confidence * 100).toFixed(0) + '%' : '-'}</span>
                        </div>
                    ` : ''}
                </div>

                <div class="detail-block modern-block">
                    <h4 style="color:#4169E1;margin-bottom:8px">🚢 现代</h4>
                    <div class="detail-row">
                        <span class="detail-label">风险评分</span>
                        <span class="detail-value">${modernRiskPct}%</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">风险等级</span>
                        <span class="detail-value">${RISK_LEVEL_ZH[modernLevel] || modernLevel}</span>
                    </div>
                    ${modernRisk ? `
                        <div class="detail-row">
                            <span class="detail-label">模型</span>
                            <span class="detail-value">${modernRisk.model_type || '-'}</span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">预计延误</span>
                            <span class="detail-value">${modernRisk.estimated_delay_hours ? modernRisk.estimated_delay_hours.toFixed(1) + '小时' : '-'}</span>
                        </div>
                        ${modernRisk.alternative_route_suggestion ? `
                            <div class="detail-row">
                                <span class="detail-label">绕行建议</span>
                                <span class="detail-value" style="font-size:11px">${modernRisk.alternative_route_suggestion}</span>
                            </div>
                        ` : ''}
                    ` : ''}
                </div>
            </div>

            <div class="comparison-change-block">
                <div class="detail-row">
                    <span class="detail-label">风险变化</span>
                    <span class="detail-value ${changeClass}">${changePct}</span>
                </div>
                ${ancientRisk && modernRisk ? `
                    <div class="detail-row">
                        <span class="detail-label">绝对差值</span>
                        <span class="detail-value">
                            ${((modernRisk.risk_score - ancientRisk.risk_score) * 100).toFixed(2)} 百分点
                        </span>
                    </div>
                ` : ''}
            </div>
        `;

        panel.classList.remove('hidden');
    }

    function getRiskLevel(score) {
        if (score >= 0.7) return 'very_high';
        if (score >= 0.5) return 'high';
        if (score >= 0.3) return 'medium';
        if (score >= 0.1) return 'low';
        return 'very_low';
    }

    function updateViewMode() {
        const mode = AppState.modernCompare?.viewMode || VIEW_MODES.BOTH;

        if (mode === VIEW_MODES.ANCIENT || mode === VIEW_MODES.BOTH) {
            if (ancientHeatmapLayer && !map.hasLayer(ancientHeatmapLayer)) {
                ancientHeatmapLayer.addTo(map);
            }
            if (ancientHighRiskLayer && !map.hasLayer(ancientHighRiskLayer)) {
                ancientHighRiskLayer.addTo(map);
            }
        } else {
            if (ancientHeatmapLayer && map.hasLayer(ancientHeatmapLayer)) {
                map.removeLayer(ancientHeatmapLayer);
            }
            if (ancientHighRiskLayer && map.hasLayer(ancientHighRiskLayer)) {
                map.removeLayer(ancientHighRiskLayer);
            }
        }

        if (mode === VIEW_MODES.MODERN || mode === VIEW_MODES.BOTH) {
            if (modernHeatmapLayer && !map.hasLayer(modernHeatmapLayer)) {
                modernHeatmapLayer.addTo(map);
            }
            if (modernHighRiskLayer && !map.hasLayer(modernHighRiskLayer)) {
                modernHighRiskLayer.addTo(map);
            }
        } else {
            if (modernHeatmapLayer && map.hasLayer(modernHeatmapLayer)) {
                map.removeLayer(modernHeatmapLayer);
            }
            if (modernHighRiskLayer && map.hasLayer(modernHighRiskLayer)) {
                map.removeLayer(modernHighRiskLayer);
            }
        }
    }

    function updateModernHeatmapOpacity() {
        if (!modernHeatmapLayer) return;
        const opacity = AppState.modernCompare?.modernOpacity ?? 0.6;
        const canvas = modernHeatmapLayer._canvas;
        if (canvas) {
            canvas.style.opacity = opacity;
        }
    }

    function updateModernOpacitySlider() {
        const slider = document.getElementById('modern-opacity');
        const valueEl = document.getElementById('modern-opacity-value');
        if (slider && valueEl && AppState.modernCompare?.modernOpacity !== undefined) {
            slider.value = AppState.modernCompare.modernOpacity;
            valueEl.textContent = Math.round(AppState.modernCompare.modernOpacity * 100) + '%';
        }
    }

    function addComparisonLegend() {
        comparisonLegend = L.control({ position: 'bottomright' });
        comparisonLegend.onAdd = function () {
            const div = L.DomUtil.create('div', 'comparison-legend');
            div.innerHTML = `
                <div class="legend-title">古代风险热力图</div>
                <div class="legend-gradient ancient-gradient"></div>
                <div class="legend-labels">
                    <span>低</span>
                    <span>高</span>
                </div>
                <div class="legend-title" style="margin-top:8px">现代风险热力图</div>
                <div class="legend-gradient modern-gradient"></div>
                <div class="legend-labels">
                    <span>低</span>
                    <span>高</span>
                </div>
                <div class="legend-title" style="margin-top:8px">高风险航线</div>
                <div class="legend-item">
                    <div class="legend-line ancient-line"></div>
                    <span>古代（虚线）</span>
                </div>
                <div class="legend-item">
                    <div class="legend-line modern-line"></div>
                    <span>现代（实线）</span>
                </div>
            `;
            return div;
        };
        comparisonLegend.addTo(map);
    }

    function clearModernComparison() {
        if (ancientHeatmapLayer) {
            map.removeLayer(ancientHeatmapLayer);
            ancientHeatmapLayer = null;
        }
        if (modernHeatmapLayer) {
            map.removeLayer(modernHeatmapLayer);
            modernHeatmapLayer = null;
        }
        if (ancientHighRiskLayer) {
            map.removeLayer(ancientHighRiskLayer);
            ancientHighRiskLayer.clearLayers();
        }
        if (modernHighRiskLayer) {
            map.removeLayer(modernHighRiskLayer);
            modernHighRiskLayer.clearLayers();
        }
        if (comparisonLegend) {
            map.removeControl(comparisonLegend);
            comparisonLegend = null;
        }
        if (AppState.modernCompare) {
            AppState.modernCompare.loaded = false;
        }
    }

    return {
        init: initModernComparison,
        load: loadModernComparison,
        clear: clearModernComparison,
        updateViewMode: updateViewMode,
        VIEW_MODES: VIEW_MODES,
    };
})();
