//! Performance Monitor tab — live metrics with smooth area charts.
//!
//! Chrome: Performance Monitor panel. Each metric has a toggleable
//! chart lane showing a filled area chart of the last 120 frames.

use std::cell::Cell;
use std::rc::Rc;

use kozan_core::styling::units::pct;
use kozan_core::{ContainerNode, Document, Element, HtmlDivElement, Node, Text};
use kozan_primitives::color::Color;
use kozan_platform::ViewContext;

use crate::chart::AreaChart;
use crate::metrics::{FrameHistory, FrameSnapshot};

/// Performance tab — owns its DOM subtree and update logic.
pub struct PerformanceTab {
    pub container: HtmlDivElement,

    style_val: Text,
    layout_val: Text,
    paint_val: Text,
    total_val: Text,
    fps_val: Text,
    bottleneck: Text,

    budget_fill: HtmlDivElement,
    budget_pct: Text,

    chart_budget: AreaChart,
    chart_fps: AreaChart,
    chart_nodes: AreaChart,
    chart_style: AreaChart,
    chart_layout: AreaChart,
    chart_paint: AreaChart,

    stats_avg: Text,
    stats_peak: Text,
    stats_jank: Text,

    info_nodes: Text,
    info_viewport: Text,
    info_zoom: Text,
    info_frame: Text,
    info_windows: Text,
    info_renderer: Text,

    nodes_max: Cell<f32>,
}

impl PerformanceTab {
    pub fn build(doc: &Document, ctx: &ViewContext) -> Self {
        let container = doc.div();
        container.class_add("kdt-perf-tab");

        // Timing grid
        let (timing_grid, style_val, layout_val, paint_val, total_val, fps_val) =
            build_timing_grid(doc);
        container.child(timing_grid);

        // Bottleneck
        let bottleneck = doc.create_text("");
        let bottleneck_el = doc.div();
        bottleneck_el.class_add("kdt-bottleneck");
        bottleneck_el.append(bottleneck);
        container.child(bottleneck_el);

        // Budget bar
        let (budget_row, budget_fill, budget_pct) = build_budget_bar(doc);
        container.child(budget_row);

        // Separator
        add_sep(doc, &container);

        // Charts
        let charts = doc.div();
        charts.class_add("kdt-charts-section");

        // New color scheme matching the CSS tokens
        let chart_budget = AreaChart::build(doc, "Budget", "kdt-color-budget",
            Color::from_rgb8(74, 222, 128));
        let chart_fps = AreaChart::build(doc, "FPS", "kdt-color-fps",
            Color::from_rgb8(107, 138, 253));
        let chart_nodes = AreaChart::build(doc, "Nodes", "kdt-color-nodes",
            Color::from_rgb8(45, 212, 191));
        let chart_style = AreaChart::build(doc, "Style", "kdt-color-style-time",
            Color::from_rgb8(167, 139, 250));
        let chart_layout = AreaChart::build(doc, "Layout", "kdt-color-layout-time",
            Color::from_rgb8(251, 191, 36));
        let chart_paint = AreaChart::build(doc, "Paint", "kdt-color-paint-time",
            Color::from_rgb8(251, 146, 60));

        charts
            .child(chart_budget.lane)
            .child(chart_fps.lane)
            .child(chart_nodes.lane)
            .child(chart_style.lane)
            .child(chart_layout.lane)
            .child(chart_paint.lane);
        container.child(charts);

        // Separator
        add_sep(doc, &container);

        // Stats row
        let (stats_row, stats_avg, stats_peak, stats_jank) = build_stats_row(doc);
        container.child(stats_row);

        // Info grid
        let (info_grid, info_nodes, info_viewport, info_zoom, info_frame, info_windows, info_renderer) =
            build_info_row(doc, ctx);
        container.child(info_grid);

        Self {
            container,
            style_val, layout_val, paint_val, total_val, fps_val,
            bottleneck, budget_fill, budget_pct,
            chart_budget, chart_fps, chart_nodes,
            chart_style, chart_layout, chart_paint,
            stats_avg, stats_peak, stats_jank,
            info_nodes, info_viewport, info_zoom, info_frame,
            info_windows, info_renderer,
            nodes_max: Cell::new(100.0),
        }
    }

    pub fn reset_charts(&self) {
        self.chart_budget.reset();
        self.chart_fps.reset();
        self.chart_nodes.reset();
        self.chart_style.reset();
        self.chart_layout.reset();
        self.chart_paint.reset();
        self.nodes_max.set(100.0);
    }

    pub fn update(
        &self,
        snap: &FrameSnapshot,
        history: &FrameHistory,
        ctx: &ViewContext,
        prev_zoom: &Rc<Cell<f64>>,
        prev_vp_w: &Rc<Cell<u32>>,
        prev_vp_h: &Rc<Cell<u32>>,
        frame_num: u64,
    ) {
        let t = &snap.timing;

        self.style_val.set_content(format!("{:.1}", t.style_ms));
        self.layout_val.set_content(format!("{:.1}", t.layout_ms));
        self.paint_val.set_content(format!("{:.1}", t.paint_ms));
        self.total_val.set_content(format!("{:.1}", t.total_ms));
        self.fps_val.set_content(format!("{:.0}", snap.fps));
        self.bottleneck.set_content(bottleneck_text(t));

        self.update_budget_bar(snap);
        self.update_charts(snap);
        self.update_stats(history);
        self.update_info(ctx, prev_zoom, prev_vp_w, prev_vp_h, frame_num);
    }

    fn update_budget_bar(&self, snap: &FrameSnapshot) {
        let usage = snap.budget_usage();
        let clamped = (usage * 100.0).min(100.0);
        self.budget_fill.style().w(pct(clamped as f32));
        self.budget_pct.set_content(format!("{:.0}%", usage * 100.0));

        self.budget_fill.class_remove("kdt-budget-warn");
        self.budget_fill.class_remove("kdt-budget-over");
        if usage > 1.0 {
            self.budget_fill.class_add("kdt-budget-over");
        } else if usage > 0.75 {
            self.budget_fill.class_add("kdt-budget-warn");
        }
    }

    fn update_charts(&self, snap: &FrameSnapshot) {
        let t = &snap.timing;
        let usage = snap.budget_usage();

        self.chart_budget.update(
            (usage as f32 / 2.0).clamp(0.0, 1.0),
            usage * 100.0,
            &format!("{:.0}%", usage * 100.0), "%",
        );
        self.chart_fps.update(
            (snap.fps as f32 / 120.0).clamp(0.0, 1.0),
            snap.fps, &format!("{:.0}", snap.fps), "",
        );

        let nodes = snap.dom_node_count as f32;
        if nodes > self.nodes_max.get() {
            self.nodes_max.set(nodes * 1.2);
        }
        self.chart_nodes.update(
            nodes / self.nodes_max.get(),
            snap.dom_node_count as f64, &format!("{}", snap.dom_node_count), "",
        );

        let budget_ms = snap.budget_ms as f32;
        if budget_ms > 0.0 {
            self.chart_style.update(
                (t.style_ms as f32 / budget_ms).clamp(0.0, 1.0),
                t.style_ms, &format!("{:.1}", t.style_ms), "ms",
            );
            self.chart_layout.update(
                (t.layout_ms as f32 / budget_ms).clamp(0.0, 1.0),
                t.layout_ms, &format!("{:.1}", t.layout_ms), "ms",
            );
            self.chart_paint.update(
                (t.paint_ms as f32 / budget_ms).clamp(0.0, 1.0),
                t.paint_ms, &format!("{:.1}", t.paint_ms), "ms",
            );
        }
    }

    fn update_stats(&self, history: &FrameHistory) {
        self.stats_avg.set_content(format!("{:.1}ms", history.avg_ms()));
        self.stats_peak.set_content(format!("{:.1}ms", history.peak_ms()));
        self.stats_jank.set_content(format!("{}", history.jank_count()));
    }

    fn update_info(
        &self,
        ctx: &ViewContext,
        prev_zoom: &Rc<Cell<f64>>,
        prev_vp_w: &Rc<Cell<u32>>,
        prev_vp_h: &Rc<Cell<u32>>,
        frame_num: u64,
    ) {
        let doc = ctx.document();
        let vp = ctx.viewport();

        self.info_nodes.set_content(format!("{} ({} el)", doc.node_count(), doc.element_count()));
        self.info_viewport.set_content(format!("{:.0}\u{00d7}{:.0}", vp.logical_width(), vp.logical_height()));

        let zoom = vp.page_zoom_factor();
        self.info_zoom.set_content(format!("{:.0}%", zoom * 100.0));
        self.info_frame.set_content(format!("#{frame_num}"));

        let platform = ctx.platform();
        self.info_windows.set_content(format!("{}", platform.window_count()));
        self.info_renderer.set_content(platform.renderer_name().to_string());

        let old_zoom = prev_zoom.get();
        if (zoom - old_zoom).abs() > 0.001 {
            prev_zoom.set(zoom);
        }

        let (w, h) = (vp.width(), vp.height());
        let (old_w, old_h) = (prev_vp_w.get(), prev_vp_h.get());
        if w != old_w || h != old_h {
            prev_vp_w.set(w);
            prev_vp_h.set(h);
        }
    }
}

fn bottleneck_text(t: &kozan_primitives::timing::FrameTiming) -> String {
    if t.total_ms < 1.0 {
        return String::new();
    }
    let max = t.style_ms.max(t.layout_ms).max(t.paint_ms);
    let phase = if (max - t.layout_ms).abs() < f64::EPSILON { "Layout" }
        else if (max - t.style_ms).abs() < f64::EPSILON { "Style" }
        else { "Paint" };
    format!("Bottleneck: {phase} ({:.0}% of frame)", max / t.total_ms * 100.0)
}

fn add_sep(doc: &Document, parent: &HtmlDivElement) {
    let sep = doc.div();
    sep.class_add("kdt-sep");
    parent.child(sep);
}

fn labeled_cell(doc: &Document, initial: &str, label: &str, color_class: &str) -> (HtmlDivElement, Text) {
    let cell = doc.div();
    cell.class_add("kdt-timing-cell");

    let val = doc.create_text(initial);
    let val_el = doc.div();
    val_el.class_add("kdt-timing-value");
    val_el.class_add(color_class);
    val_el.append(val);

    let lbl = doc.div();
    lbl.class_add("kdt-timing-label");
    lbl.append(doc.create_text(label));

    cell.child(val_el).child(lbl);
    (cell, val)
}

fn build_timing_grid(doc: &Document) -> (HtmlDivElement, Text, Text, Text, Text, Text) {
    let grid = doc.div();
    grid.class_add("kdt-timing-grid");

    let (c1, v1) = labeled_cell(doc, "--", "Style", "kdt-color-style");
    let (c2, v2) = labeled_cell(doc, "--", "Layout", "kdt-color-layout");
    let (c3, v3) = labeled_cell(doc, "--", "Paint", "kdt-color-paint");
    let (c4, v4) = labeled_cell(doc, "--", "Total", "kdt-color-total");
    let (c5, v5) = labeled_cell(doc, "--", "FPS", "kdt-color-fps");

    grid.child(c1).child(c2).child(c3).child(c4).child(c5);
    (grid, v1, v2, v3, v4, v5)
}

fn build_stats_row(doc: &Document) -> (HtmlDivElement, Text, Text, Text) {
    let row = doc.div();
    row.class_add("kdt-stats-row");

    let (c1, v1) = labeled_cell(doc, "--", "Avg", "kdt-color-total");
    let (c2, v2) = labeled_cell(doc, "--", "Peak", "kdt-color-paint");
    let (c3, v3) = labeled_cell(doc, "--", "Jank", "kdt-fps-red");

    row.child(c1).child(c2).child(c3);
    (row, v1, v2, v3)
}

fn build_budget_bar(doc: &Document) -> (HtmlDivElement, HtmlDivElement, Text) {
    let row = doc.div();
    row.class_add("kdt-budget-row");

    let header = doc.div();
    header.class_add("kdt-budget-header");

    let label = doc.div();
    label.class_add("kdt-budget-label");
    label.append(doc.create_text("Frame Budget"));

    let spacer = doc.div();
    spacer.class_add("kdt-budget-spacer");

    let pct_text = doc.create_text("0%");
    let pct_el = doc.div();
    pct_el.class_add("kdt-budget-pct");
    pct_el.append(pct_text);

    header.child(label).child(spacer).child(pct_el);

    let track = doc.div();
    track.class_add("kdt-budget-track");

    let fill = doc.div();
    fill.class_add("kdt-budget-fill");
    fill.style().w(pct(0.0));
    track.child(fill);

    row.child(header).child(track);
    (row, fill, pct_text)
}

fn labeled_info(doc: &Document, label: &str) -> (HtmlDivElement, Text) {
    let item = doc.div();
    item.class_add("kdt-info-item");

    let lbl = doc.div();
    lbl.class_add("kdt-info-label");
    lbl.append(doc.create_text(label));

    let val = doc.create_text("--");
    let val_el = doc.div();
    val_el.class_add("kdt-info-value");
    val_el.append(val);

    item.child(lbl).child(val_el);
    (item, val)
}

fn build_info_row(doc: &Document, ctx: &ViewContext) -> (HtmlDivElement, Text, Text, Text, Text, Text, Text) {
    let grid = doc.div();
    grid.class_add("kdt-info-grid");

    let (i1, v1) = labeled_info(doc, "Nodes");
    let (i2, v2) = labeled_info(doc, "Viewport");
    let (i3, v3) = labeled_info(doc, "Zoom");
    let (i4, v4) = labeled_info(doc, "Frame");
    let (i5, v5) = labeled_info(doc, "Windows");
    let (i6, v6) = labeled_info(doc, "Renderer");

    v6.set_content(ctx.platform().renderer_name().to_string());

    grid.child(i1).child(i2).child(i3).child(i4).child(i5).child(i6);
    (grid, v1, v2, v3, v4, v5, v6)
}
