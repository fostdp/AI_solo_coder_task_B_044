const MIN_YEAR = -1000;
const MAX_YEAR = 1800;
const TRACK_WIDTH = 280;

let isDragging = null;

function initTimeline() {
    const slider = document.getElementById('timeline-slider');
    updateTimelineUI();

    const handleStart = document.getElementById('handle-start');
    const handleEnd = document.getElementById('handle-end');

    handleStart.addEventListener('mousedown', (e) => {
        e.preventDefault();
        isDragging = 'start';
    });

    handleEnd.addEventListener('mousedown', (e) => {
        e.preventDefault();
        isDragging = 'end';
    });

    document.addEventListener('mousemove', (e) => {
        if (!isDragging) return;
        const sliderRect = slider.getBoundingClientRect();
        let pct = (e.clientX - sliderRect.left) / sliderRect.width;
        pct = Math.max(0, Math.min(1, pct));
        const year = Math.round(MIN_YEAR + pct * (MAX_YEAR - MIN_YEAR));

        if (isDragging === 'start') {
            AppState.yearStart = Math.min(year, AppState.yearEnd - 50);
        } else {
            AppState.yearEnd = Math.max(year, AppState.yearStart + 50);
        }
        updateTimelineUI();
    });

    document.addEventListener('mouseup', () => {
        if (isDragging) {
            isDragging = null;
            onTimelineChange();
        }
    });

    slider.addEventListener('click', (e) => {
        if (e.target.classList.contains('timeline-handle')) return;
        const sliderRect = slider.getBoundingClientRect();
        let pct = (e.clientX - sliderRect.left) / sliderRect.width;
        pct = Math.max(0, Math.min(1, pct));
        const year = Math.round(MIN_YEAR + pct * (MAX_YEAR - MIN_YEAR));
        const mid = (AppState.yearStart + AppState.yearEnd) / 2;
        if (year < mid) {
            AppState.yearStart = Math.min(year, AppState.yearEnd - 50);
        } else {
            AppState.yearEnd = Math.max(year, AppState.yearStart + 50);
        }
        updateTimelineUI();
        onTimelineChange();
    });
}

function updateTimelineUI() {
    const startPct = (AppState.yearStart - MIN_YEAR) / (MAX_YEAR - MIN_YEAR) * 100;
    const endPct = (AppState.yearEnd - MIN_YEAR) / (MAX_YEAR - MIN_YEAR) * 100;

    document.getElementById('handle-start').style.left = `${startPct}%`;
    document.getElementById('handle-end').style.left = `${endPct}%`;
    document.getElementById('timeline-range').style.left = `${startPct}%`;
    document.getElementById('timeline-range').style.width = `${endPct - startPct}%`;

    const formatYear = (y) => y < 0 ? `公元前${Math.abs(y)}年` : `公元${y}年`;
    document.getElementById('year-start-label').textContent = formatYear(AppState.yearStart);
    document.getElementById('year-end-label').textContent = formatYear(AppState.yearEnd);
}

let timelineDebounce = null;
function onTimelineChange() {
    clearTimeout(timelineDebounce);
    timelineDebounce = setTimeout(async () => {
        await loadVoyages();
        renderMapLayers();
    }, 300);
}
