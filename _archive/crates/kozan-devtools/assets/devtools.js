/**
 * kozan-devtools — Browser mirror of the Rust kozan-devtools overlay.
 *
 * Uses the SAME devtools.css as the Rust version — no inline CSS.
 * Same DOM structure, same class names, pixel-perfect comparison.
 *
 * Usage:  <script src="devtools.js"></script>
 * Auto-attaches to document.body on load.
 */
(function () {
  'use strict';

  // Capture script reference immediately (unavailable in callbacks).
  const SELF_SCRIPT = document.currentScript;

  const HISTORY = 120;
  const CHART_H = 32;
  const BUDGET_HZ = 144;
  const BUDGET_MS = 1000 / BUDGET_HZ;

  const METRICS = [
    { label: 'Budget',  rgb: [74,222,128],  unit: '%',  cls: 'kdt-color-budget' },
    { label: 'FPS',     rgb: [107,138,253], unit: '',   cls: 'kdt-color-fps' },
    { label: 'Nodes',   rgb: [45,212,191],  unit: '',   cls: 'kdt-color-nodes' },
    { label: 'Style',   rgb: [167,139,250], unit: 'ms', cls: 'kdt-color-style-time' },
    { label: 'Layout',  rgb: [251,191,36],  unit: 'ms', cls: 'kdt-color-layout-time' },
    { label: 'Paint',   rgb: [251,146,60],  unit: 'ms', cls: 'kdt-color-paint-time' },
  ];

  // ── Load devtools.css from same directory as this script ───
  function injectCSS() {
    if (!SELF_SCRIPT) return;
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = SELF_SCRIPT.src.replace(/devtools\.js$/, 'devtools.css');
    document.head.appendChild(link);
  }

  // ── Helpers ──────────────────────────────────────────────────
  function $(tag, cls, parent) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (parent) parent.appendChild(e);
    return e;
  }

  // ── Area chart — matches chart.rs ───────────────────────────
  function createChart(metric, parent) {
    const lane = $('div', 'kdt-chart-lane', parent);

    const toggle = $('div', 'kdt-chart-toggle ' + metric.cls, lane);
    $('div', 'kdt-chart-label', lane).textContent = metric.label;

    const valueBlock = $('div', 'kdt-chart-value-block', lane);
    const curEl = $('div', 'kdt-chart-value', valueBlock);
    curEl.textContent = '--';
    const statEl = $('div', 'kdt-chart-stats', valueBlock);

    const canvas = document.createElement('canvas');
    canvas.width = HISTORY;
    canvas.height = CHART_H;
    canvas.className = 'kdt-chart-canvas';
    lane.appendChild(canvas);
    const ctx = canvas.getContext('2d');

    const [r, g, b] = metric.rgb;
    const buf = new Float32Array(HISTORY);
    let wi = 0, avg = 0, peak = 0, n = 0, on = true;

    toggle.addEventListener('click', () => {
      on = !on;
      toggle.classList.toggle('kdt-chart-toggle-off', !on);
      canvas.classList.toggle('kdt-chart-hidden', !on);
    });

    function syncSize() {
      const rect = canvas.getBoundingClientRect();
      if (rect.width < 1 || rect.height < 1) return;
      if (Math.abs(canvas.width - rect.width) > 0.5 || Math.abs(canvas.height - rect.height) > 0.5) {
        canvas.width = rect.width;
        canvas.height = rect.height;
      }
    }

    function draw() {
      syncSize();
      const w = canvas.width, h = canvas.height;
      if (w < 1 || h < 1) return;
      const xStep = HISTORY > 1 ? w / (HISTORY - 1) : w;

      ctx.clearRect(0, 0, w, h);
      ctx.fillStyle = 'rgba(255,255,255,0.015)';
      ctx.fillRect(0, 0, w, h);

      ctx.strokeStyle = 'rgba(255,255,255,0.04)';
      ctx.lineWidth = 0.5;
      ctx.beginPath(); ctx.moveTo(0, h * 0.5); ctx.lineTo(w, h * 0.5); ctx.stroke();

      const grad = ctx.createLinearGradient(0, 0, 0, h);
      grad.addColorStop(0, `rgba(${r},${g},${b},0.25)`);
      grad.addColorStop(1, `rgba(${r},${g},${b},0.02)`);

      ctx.beginPath();
      ctx.moveTo(0, h);
      for (let i = 0; i < HISTORY; i++) {
        ctx.lineTo(i * xStep, h * (1 - buf[(wi + 1 + i) % HISTORY]));
      }
      ctx.lineTo(w, h);
      ctx.closePath();
      ctx.fillStyle = grad;
      ctx.fill();

      ctx.beginPath();
      for (let i = 0; i < HISTORY; i++) {
        const v = buf[(wi + 1 + i) % HISTORY];
        const x = i * xStep;
        i === 0 ? ctx.moveTo(x, h * (1 - v)) : ctx.lineTo(x, h * (1 - v));
      }
      ctx.strokeStyle = `rgb(${r},${g},${b})`;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    return {
      update(norm, raw, display) {
        n++;
        if (raw > peak) peak = raw;
        avg += (raw - avg) / n;
        curEl.textContent = display;
        statEl.textContent = `${avg.toFixed(1)} / ${peak.toFixed(1)}${metric.unit}`;
        if (!on) return;
        buf[wi] = Math.max(0, Math.min(1, norm));
        wi = (wi + 1) % HISTORY;
        draw();
      },
      reset() {
        avg = 0; peak = 0; n = 0; buf.fill(0); wi = 0;
        statEl.textContent = '';
        ctx.clearRect(0, 0, canvas.width, canvas.height);
      }
    };
  }

  // ── Build DOM — matches shell.rs + performance.rs ──────────
  function build() {
    injectCSS();

    // ── Root (shell.rs:64) ──
    const root = $('div', 'kdt-root');
    root.style.cssText = 'top:8px;right:8px;';

    // ── Badge (shell.rs:258) ──
    const badge = $('div', 'kdt-badge', root);
    badge.style.display = 'none';
    const badgeFpsEl = $('div', 'kdt-fps-number kdt-fps-green', badge);
    badgeFpsEl.textContent = '--';
    const badgeDetail = $('div', 'kdt-fps-label', badge);
    badgeDetail.textContent = 'FPS';

    // ── Panel (shell.rs:291) ──
    const panel = $('div', 'kdt-panel kdt-panel-open', root);

    // Tab bar (shell.rs:296)
    const tabBar = $('div', 'kdt-tab-bar', panel);
    $('div', 'kdt-tab', tabBar).textContent = 'Performance';
    $('div', 'kdt-titlebar-spacer', tabBar);
    const recBtn = $('div', 'kdt-record-btn', tabBar);
    recBtn.textContent = 'Rec';
    const resetBtn = $('div', 'kdt-action-btn', tabBar);
    resetBtn.textContent = 'Reset';
    const fsBtn = $('div', 'kdt-action-btn', tabBar);
    fsBtn.textContent = '[ ]';
    const closeBtn = $('div', 'kdt-close-btn', tabBar);
    closeBtn.textContent = '\u00d7';

    // Tab content — scrollable body (shell.rs:332)
    const content = $('div', 'kdt-tab-content kdt-tab-content-active', panel);

    // ── Performance tab (performance.rs:53) ──
    const perfTab = $('div', 'kdt-perf-tab', content);

    // Timing grid — 5 cells (performance.rs:295)
    const timingGrid = $('div', 'kdt-timing-grid', perfTab);
    function timingCell(grid, initial, label, colorCls) {
      const cell = $('div', 'kdt-timing-cell', grid);
      const v = $('div', 'kdt-timing-value ' + colorCls, cell);
      v.textContent = initial;
      $('div', 'kdt-timing-label', cell).textContent = label;
      return v;
    }
    const tv = {
      style:  timingCell(timingGrid, '--', 'Style',  'kdt-color-style'),
      layout: timingCell(timingGrid, '--', 'Layout', 'kdt-color-layout'),
      paint:  timingCell(timingGrid, '--', 'Paint',  'kdt-color-paint'),
      total:  timingCell(timingGrid, '--', 'Total',  'kdt-color-total'),
      fps:    timingCell(timingGrid, '--', 'FPS',    'kdt-color-fps'),
    };

    // Bottleneck (performance.rs:63)
    const bneckEl = $('div', 'kdt-bottleneck', perfTab);

    // Budget bar (performance.rs:321)
    const budgetRow = $('div', 'kdt-budget-row', perfTab);
    const budgetHdr = $('div', 'kdt-budget-header', budgetRow);
    $('div', 'kdt-budget-label', budgetHdr).textContent = 'Frame Budget';
    $('div', 'kdt-budget-spacer', budgetHdr);
    const budgetPct = $('div', 'kdt-budget-pct', budgetHdr);
    budgetPct.textContent = '0%';
    const budgetTrack = $('div', 'kdt-budget-track', budgetRow);
    const budgetFill = $('div', 'kdt-budget-fill', budgetTrack);

    // Separator
    $('div', 'kdt-sep', perfTab);

    // Charts (performance.rs:77)
    const chartsBox = $('div', 'kdt-charts-section', perfTab);
    const charts = METRICS.map(m => createChart(m, chartsBox));

    // Separator
    $('div', 'kdt-sep', perfTab);

    // Stats row — 3 cells (performance.rs:309)
    const statsRow = $('div', 'kdt-stats-row', perfTab);
    const sv = {
      avg:  timingCell(statsRow, '--', 'Avg',  'kdt-color-total'),
      peak: timingCell(statsRow, '--', 'Peak', 'kdt-color-paint'),
      jank: timingCell(statsRow, '--', 'Jank', 'kdt-fps-red'),
    };

    // Info grid (performance.rs:371)
    const infoGrid = $('div', 'kdt-info-grid', perfTab);
    const iv = {};
    for (const lbl of ['Nodes', 'Viewport', 'Zoom', 'Frame', 'Windows', 'Renderer']) {
      const item = $('div', 'kdt-info-item', infoGrid);
      $('div', 'kdt-info-label', item).textContent = lbl;
      iv[lbl] = $('div', 'kdt-info-value', item);
      iv[lbl].textContent = '--';
    }
    iv.Zoom.textContent = '100%';
    iv.Windows.textContent = '1';
    iv.Renderer.textContent = 'Chrome';

    // Separator before event log (shell.rs:82)
    $('div', 'kdt-sep', content);

    // Event log (shell.rs:338)
    const logSection = $('div', 'kdt-log-section', content);
    const logToggle = $('div', 'kdt-log-toggle', logSection);
    const logArrowEl = $('div', 'kdt-log-arrow', logToggle);
    logArrowEl.textContent = '>';
    logToggle.appendChild(document.createTextNode(' Event Log'));
    const logBody = $('div', 'kdt-log-body', logSection);
    $('div', 'kdt-log-list', logBody);

    // ── Interactions ───────────────────────────────────────────
    let expanded = true;

    badge.addEventListener('click', () => {
      expanded = true;
      badge.style.display = 'none';
      panel.classList.add('kdt-panel-open');
    });
    closeBtn.addEventListener('click', () => {
      expanded = false;
      panel.classList.remove('kdt-panel-open');
      badge.style.display = 'flex';
    });
    fsBtn.addEventListener('click', () => {
      root.classList.toggle('kdt-fullscreen');
      if (root.classList.contains('kdt-fullscreen')) {
        root.style.cssText = '';
      } else {
        root.style.cssText = 'top:8px;right:8px;';
      }
    });

    let logOpen = false;
    logToggle.addEventListener('click', () => {
      logOpen = !logOpen;
      logArrowEl.textContent = logOpen ? 'v' : '>';
      logBody.classList.toggle('kdt-log-open', logOpen);
    });

    // Drag
    let dragging = false, dx = 0, dy = 0;
    tabBar.addEventListener('mousedown', e => {
      if (root.classList.contains('kdt-fullscreen')) return;
      dragging = true;
      const r = root.getBoundingClientRect();
      dx = e.clientX - r.left; dy = e.clientY - r.top;
    });
    window.addEventListener('mousemove', e => {
      if (!dragging) return;
      root.style.left = Math.max(0, e.clientX - dx) + 'px';
      root.style.top = Math.max(0, e.clientY - dy) + 'px';
      root.style.right = 'auto';
    });
    window.addEventListener('mouseup', () => { dragging = false; });

    document.body.appendChild(root);

    // ── Frame loop ─────────────────────────────────────────────
    const PANEL_INTERVAL = 60;
    let frameNum = 0, prevTime = performance.now();
    let avgMs = 0, peakMs = 0, jankCount = 0, nodesMax = 100;
    let fpsFrames = 0, fpsLast = performance.now(), smoothFps = 0;
    let lastPanelUpdate = 0;

    resetBtn.addEventListener('click', () => {
      charts.forEach(c => c.reset());
      frameNum = 0; avgMs = 0; peakMs = 0; jankCount = 0;
      fpsFrames = 0; fpsLast = performance.now(); smoothFps = 0;
    });

    function tick(now) {
      const dt = now - prevTime;
      prevTime = now;
      frameNum++;

      fpsFrames++;
      if (now - fpsLast >= 500) {
        smoothFps = Math.round(fpsFrames / ((now - fpsLast) / 1000));
        fpsFrames = 0; fpsLast = now;
      }
      const fps = smoothFps || Math.round(1000 / Math.max(dt, 0.1));
      const totalMs = dt;
      const styleMs = totalMs * 0.15;
      const layoutMs = totalMs * 0.55;
      const paintMs = totalMs * 0.20;
      const usage = totalMs / BUDGET_MS;

      avgMs += (totalMs - avgMs) / frameNum;
      if (totalMs > peakMs) peakMs = totalMs;
      if (totalMs > BUDGET_MS) jankCount++;

      // Badge — always update
      badgeFpsEl.textContent = fps;
      badgeFpsEl.className = 'kdt-fps-number ' +
        (fps >= 55 ? 'kdt-fps-green' : fps >= 30 ? 'kdt-fps-yellow' : 'kdt-fps-red');
      badgeDetail.textContent = totalMs.toFixed(1) + 'ms';

      // Panel — throttled
      if (!expanded || now - lastPanelUpdate < PANEL_INTERVAL) {
        requestAnimationFrame(tick);
        return;
      }
      lastPanelUpdate = now;
      const jsStart = performance.now();

      tv.style.textContent = styleMs.toFixed(1);
      tv.layout.textContent = layoutMs.toFixed(1);
      tv.paint.textContent = paintMs.toFixed(1);
      tv.total.textContent = totalMs.toFixed(1);
      tv.fps.textContent = fps;

      if (totalMs >= 1.0) {
        const max = Math.max(styleMs, layoutMs, paintMs);
        const phase = max === layoutMs ? 'Layout' : max === styleMs ? 'Style' : 'Paint';
        bneckEl.textContent = `Bottleneck: ${phase} (${Math.round(max / totalMs * 100)}% of frame)`;
      } else { bneckEl.textContent = ''; }

      budgetFill.style.width = Math.min(usage * 100, 100) + '%';
      budgetPct.textContent = Math.round(usage * 100) + '%';
      budgetFill.classList.toggle('kdt-budget-warn', usage > 0.75 && usage <= 1.0);
      budgetFill.classList.toggle('kdt-budget-over', usage > 1.0);

      const nodeCount = document.querySelectorAll('*').length;
      charts[0].update(Math.min(usage / 2, 1), usage * 100, Math.round(usage * 100) + '%');
      charts[1].update(Math.min(fps / BUDGET_HZ, 1), fps, String(fps));
      if (nodeCount > nodesMax) nodesMax = nodeCount * 1.2;
      charts[2].update(nodeCount / nodesMax, nodeCount, String(nodeCount));
      charts[3].update(Math.min(styleMs / BUDGET_MS, 1), styleMs, styleMs.toFixed(1));
      charts[4].update(Math.min(layoutMs / BUDGET_MS, 1), layoutMs, layoutMs.toFixed(1));
      charts[5].update(Math.min(paintMs / BUDGET_MS, 1), paintMs, paintMs.toFixed(1));

      sv.avg.textContent = avgMs.toFixed(1) + 'ms';
      sv.peak.textContent = peakMs.toFixed(1) + 'ms';
      sv.jank.textContent = jankCount;

      const jsMs = performance.now() - jsStart;
      const elCount = document.querySelectorAll('*:not(script):not(style):not(link)').length;
      iv.Nodes.textContent = `${nodeCount} (${elCount} el)`;
      iv.Viewport.textContent = innerWidth + '\u00d7' + innerHeight;
      iv.Frame.textContent = '#' + frameNum;
      iv.Renderer.textContent = 'Chrome | JS ' + jsMs.toFixed(2) + 'ms';

      requestAnimationFrame(tick);
    }
    requestAnimationFrame(tick);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', build);
  } else { build(); }
})();
