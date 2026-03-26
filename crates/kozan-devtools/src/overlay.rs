//! DevTools overlay — floating performance panel.
//!
//! Chrome: Performance panel HUD overlay.
//! Two states: collapsed badge (FPS only) and expanded panel (full metrics).

use std::cell::Cell;
use std::rc::Rc;

use kozan_core::styling::units::{pct, px};
use kozan_core::{
    ClickEvent, ContainerNode, Document, Element, EventTarget, HtmlDivElement, Node,
    Text,
};
use kozan_platform::ViewContext;

use crate::metrics::{FrameHistory, FrameSnapshot};

const GRAPH_BAR_COUNT: usize = 120;
const LOG_MAX_ENTRIES: usize = 50;

/// Build the DevTools overlay — returns the root element to append to body.
pub fn build(doc: &Document, ctx: &ViewContext) -> HtmlDivElement {
    let root = doc.div();
    root.class_add("kdt-root");
    root.style().top(px(8.0)).right(px(8.0));

    let badge = build_badge(doc);
    let panel = build_panel(doc);

    root.child(badge.container).child(panel.container);

    let expanded = Rc::new(Cell::new(false));

    {
        let expanded = Rc::clone(&expanded);
        let panel_el = panel.container;
        let badge_el = badge.container;
        badge_el.on::<ClickEvent>(move |_, _| {
            expanded.set(true);
            badge_el.set_attribute("style", "display: none");
            panel_el.class_add("kdt-panel-open");
        });
    }

    {
        let expanded = Rc::clone(&expanded);
        let panel_el = panel.container;
        let badge_el = badge.container;
        panel.close_btn.on::<ClickEvent>(move |_, _| {
            expanded.set(false);
            panel_el.class_remove("kdt-panel-open");
            badge_el.set_attribute("style", "display: flex");
        });
    }

    let history = Rc::new(FrameHistory::new());

    {
        let history = Rc::clone(&history);
        panel.reset_btn.on::<ClickEvent>(move |_, _| {
            history.reset();
        });
    }

    let log_expanded = Rc::new(Cell::new(false));
    {
        let log_expanded = Rc::clone(&log_expanded);
        let log_body = panel.log_body;
        let log_arrow = panel.log_arrow;
        panel.log_toggle.on::<ClickEvent>(move |_, _| {
            let open = !log_expanded.get();
            log_expanded.set(open);
            if open {
                log_body.class_add("kdt-log-open");
                log_arrow.set_content("v");
            } else {
                log_body.class_remove("kdt-log-open");
                log_arrow.set_content(">");
            }
        });
    }

    // Capture viewport state for change detection in the frame callback.
    let prev_zoom = Rc::new(Cell::new(ctx.page_zoom()));
    let prev_vp_w = Rc::new(Cell::new(ctx.viewport().width()));
    let prev_vp_h = Rc::new(Cell::new(ctx.viewport().height()));

    ctx.request_frame({
        let history = Rc::clone(&history);
        let prev_zoom = Rc::clone(&prev_zoom);
        let prev_vp_w = Rc::clone(&prev_vp_w);
        let prev_vp_h = Rc::clone(&prev_vp_h);
        move |info, ctx| {
            let snapshot = FrameSnapshot {
                timing: info.prev_timing,
                fps: info.fps,
                frame_number: info.frame_number,
                budget_ms: info.frame_budget.as_secs_f64() * 1000.0,
            };
            history.push(snapshot);

            update_badge(&badge, &snapshot);

            if expanded.get() {
                update_panel(&panel, &snapshot, &history, info.frame_number);
                update_live_info(&panel, ctx, &prev_zoom, &prev_vp_w, &prev_vp_h);
            }

            true
        }
    });

    root
}

// ── Badge ────────────────────────────────────────────────────

struct Badge {
    container: HtmlDivElement,
    fps_number: Text,
    fps_number_el: HtmlDivElement,
    fps_detail: Text,
}

fn build_badge(doc: &Document) -> Badge {
    let container = doc.div();
    container.class_add("kdt-badge");

    let fps_number = doc.create_text("--");
    let fps_number_el = doc.div();
    fps_number_el.class_add("kdt-fps-number");
    fps_number_el.class_add("kdt-fps-green");
    fps_number_el.append(fps_number);
    container.child(fps_number_el);

    let fps_detail = doc.create_text("FPS");
    let fps_label = doc.div();
    fps_label.class_add("kdt-fps-label");
    fps_label.append(fps_detail);
    container.child(fps_label);

    Badge {
        container,
        fps_number,
        fps_number_el,
        fps_detail,
    }
}

fn update_badge(badge: &Badge, snap: &FrameSnapshot) {
    badge.fps_number.set_content(format!("{:.0}", snap.fps));

    badge.fps_number_el.class_remove("kdt-fps-green");
    badge.fps_number_el.class_remove("kdt-fps-yellow");
    badge.fps_number_el.class_remove("kdt-fps-red");
    badge.fps_number_el.class_add(fps_color_class(snap.fps));

    badge
        .fps_detail
        .set_content(format!("{:.1}ms", snap.timing.total_ms));
}

fn fps_color_class(fps: f64) -> &'static str {
    if fps >= 55.0 {
        "kdt-fps-green"
    } else if fps >= 30.0 {
        "kdt-fps-yellow"
    } else {
        "kdt-fps-red"
    }
}

// ── Expanded panel ───────────────────────────────────────────

struct Panel {
    container: HtmlDivElement,
    close_btn: HtmlDivElement,
    reset_btn: HtmlDivElement,
    style_val: Text,
    layout_val: Text,
    paint_val: Text,
    total_val: Text,
    fps_val: Text,
    bottleneck: Text,
    budget_fill: HtmlDivElement,
    budget_pct: Text,
    graph_bars: Vec<HtmlDivElement>,
    graph_stats: Text,
    info_nodes: Text,
    info_viewport: Text,
    info_zoom: Text,
    info_frame: Text,
    log_toggle: HtmlDivElement,
    log_arrow: Text,
    log_body: HtmlDivElement,
    log_list: HtmlDivElement,
    log_count: Rc<Cell<usize>>,
}

fn build_panel(doc: &Document) -> Panel {
    let container = doc.div();
    container.class_add("kdt-panel");

    let (titlebar, close_btn, reset_btn) = build_titlebar(doc);
    container.child(titlebar);

    let (timing_grid, style_val, layout_val, paint_val, total_val, fps_val) =
        build_timing_grid(doc);
    container.child(timing_grid);

    let bottleneck_text = doc.create_text("");
    let bottleneck_el = doc.div();
    bottleneck_el.class_add("kdt-bottleneck");
    bottleneck_el.append(bottleneck_text);
    container.child(bottleneck_el);

    let (budget_row, budget_fill, budget_pct) = build_budget_bar(doc);
    container.child(budget_row);

    let (graph_section, graph_bars, graph_stats) = build_frame_graph(doc);
    container.child(graph_section);

    let (info_grid, info_nodes, info_viewport, info_zoom, info_frame) = build_info_row(doc);
    container.child(info_grid);

    let (log_section, log_toggle, log_arrow, log_body, log_list) = build_log_section(doc);
    container.child(log_section);

    Panel {
        container,
        close_btn,
        reset_btn,
        style_val,
        layout_val,
        paint_val,
        total_val,
        fps_val,
        bottleneck: bottleneck_text,
        budget_fill,
        budget_pct,
        graph_bars,
        graph_stats,
        info_nodes,
        info_viewport,
        info_zoom,
        info_frame,
        log_toggle,
        log_arrow,
        log_body,
        log_list,
        log_count: Rc::new(Cell::new(0)),
    }
}

fn build_titlebar(doc: &Document) -> (HtmlDivElement, HtmlDivElement, HtmlDivElement) {
    let bar = doc.div();
    bar.class_add("kdt-titlebar");

    let title = doc.div();
    title.class_add("kdt-title");
    title.append(doc.create_text("Performance"));

    let spacer = doc.div();
    spacer.class_add("kdt-titlebar-spacer");

    let reset = doc.div();
    reset.class_add("kdt-action-btn");
    reset.append(doc.create_text("Reset"));

    let close = doc.div();
    close.class_add("kdt-close-btn");
    close.append(doc.create_text("X"));

    bar.child(title).child(spacer).child(reset).child(close);
    (bar, close, reset)
}

fn build_timing_grid(
    doc: &Document,
) -> (HtmlDivElement, Text, Text, Text, Text, Text) {
    let grid = doc.div();
    grid.class_add("kdt-timing-grid");

    let (style_cell, style_val) = timing_cell(doc, "--", "Style", "kdt-color-style");
    let (layout_cell, layout_val) = timing_cell(doc, "--", "Layout", "kdt-color-layout");
    let (paint_cell, paint_val) = timing_cell(doc, "--", "Paint", "kdt-color-paint");
    let (total_cell, total_val) = timing_cell(doc, "--", "Total", "kdt-color-total");
    let (fps_cell, fps_val) = timing_cell(doc, "--", "FPS", "kdt-color-fps");

    grid.child(style_cell)
        .child(layout_cell)
        .child(paint_cell)
        .child(total_cell)
        .child(fps_cell);

    (grid, style_val, layout_val, paint_val, total_val, fps_val)
}

fn timing_cell(
    doc: &Document,
    initial: &str,
    label: &str,
    color_class: &str,
) -> (HtmlDivElement, Text) {
    let cell = doc.div();
    cell.class_add("kdt-timing-cell");

    let val_text = doc.create_text(initial);
    let val_el = doc.div();
    val_el.class_add("kdt-timing-value");
    val_el.class_add(color_class);
    val_el.append(val_text);

    let label_el = doc.div();
    label_el.class_add("kdt-timing-label");
    label_el.append(doc.create_text(label));

    cell.child(val_el).child(label_el);
    (cell, val_text)
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

fn build_frame_graph(doc: &Document) -> (HtmlDivElement, Vec<HtmlDivElement>, Text) {
    let section = doc.div();
    section.class_add("kdt-graph-section");

    let header = doc.div();
    header.class_add("kdt-graph-header");

    let label = doc.div();
    label.class_add("kdt-graph-label");
    label.append(doc.create_text("Frames"));

    let spacer = doc.div();
    spacer.class_add("kdt-graph-spacer");

    let stats_text = doc.create_text("");
    let stats_el = doc.div();
    stats_el.class_add("kdt-graph-stats");
    stats_el.append(stats_text);

    header.child(label).child(spacer).child(stats_el);

    let graph = doc.div();
    graph.class_add("kdt-graph");

    let mut bars = Vec::with_capacity(GRAPH_BAR_COUNT);
    for _ in 0..GRAPH_BAR_COUNT {
        let bar = doc.div();
        bar.class_add("kdt-bar");
        bar.style().h(px(0.0));
        graph.child(bar);
        bars.push(bar);
    }

    section.child(header).child(graph);
    (section, bars, stats_text)
}

fn build_info_row(doc: &Document) -> (HtmlDivElement, Text, Text, Text, Text) {
    let grid = doc.div();
    grid.class_add("kdt-info-grid");

    let (nodes_item, nodes_val) = info_item(doc, "Nodes");
    let (vp_item, vp_val) = info_item(doc, "Viewport");
    let (zoom_item, zoom_val) = info_item(doc, "Zoom");
    let (frame_item, frame_val) = info_item(doc, "Frame");

    grid.child(nodes_item)
        .child(vp_item)
        .child(zoom_item)
        .child(frame_item);

    (grid, nodes_val, vp_val, zoom_val, frame_val)
}

fn info_item(doc: &Document, label: &str) -> (HtmlDivElement, Text) {
    let item = doc.div();
    item.class_add("kdt-info-item");

    let label_el = doc.div();
    label_el.class_add("kdt-info-label");
    label_el.append(doc.create_text(label));

    let val_text = doc.create_text("--");
    let val_el = doc.div();
    val_el.class_add("kdt-info-value");
    val_el.append(val_text);

    item.child(label_el).child(val_el);
    (item, val_text)
}

fn build_log_section(
    doc: &Document,
) -> (HtmlDivElement, HtmlDivElement, Text, HtmlDivElement, HtmlDivElement) {
    let section = doc.div();
    section.class_add("kdt-log-section");

    let toggle = doc.div();
    toggle.class_add("kdt-log-toggle");

    let arrow = doc.create_text(">");
    let arrow_el = doc.div();
    arrow_el.class_add("kdt-log-arrow");
    arrow_el.append(arrow);
    toggle.child(arrow_el);
    toggle.append(doc.create_text("Event Log"));

    let body = doc.div();
    body.class_add("kdt-log-body");

    let list = doc.div();
    list.class_add("kdt-log-list");
    body.child(list);

    section.child(toggle).child(body);
    (section, toggle, arrow, body, list)
}

// ── Update logic ─────────────────────────────────────────────

fn update_panel(panel: &Panel, snap: &FrameSnapshot, history: &FrameHistory, frame_num: u64) {
    let t = &snap.timing;

    panel.style_val.set_content(format!("{:.1}", t.style_ms));
    panel
        .layout_val
        .set_content(format!("{:.1}", t.layout_ms));
    panel.paint_val.set_content(format!("{:.1}", t.paint_ms));
    panel.total_val.set_content(format!("{:.1}", t.total_ms));
    panel.fps_val.set_content(format!("{:.0}", snap.fps));

    panel.bottleneck.set_content(bottleneck_text(t));

    let usage = snap.budget_usage();
    let usage_pct = (usage * 100.0).min(100.0);
    panel.budget_fill.style().w(pct(usage_pct as f32));
    panel
        .budget_pct
        .set_content(format!("{:.0}%", usage * 100.0));

    panel.budget_fill.class_remove("kdt-budget-warn");
    panel.budget_fill.class_remove("kdt-budget-over");
    if usage > 1.0 {
        panel.budget_fill.class_add("kdt-budget-over");
    } else if usage > 0.75 {
        panel.budget_fill.class_add("kdt-budget-warn");
    }

    update_graph(panel, history);

    panel.graph_stats.set_content(format!(
        "avg {:.1}ms | peak {:.1}ms | jank {}",
        history.avg_ms(),
        history.peak_ms(),
        history.jank_count()
    ));

    panel.info_frame.set_content(format!("#{frame_num}"));
}

fn bottleneck_text(t: &kozan_primitives::timing::FrameTiming) -> String {
    if t.total_ms < 1.0 {
        return String::new();
    }
    let max = t.style_ms.max(t.layout_ms).max(t.paint_ms);
    let phase = if (max - t.layout_ms).abs() < f64::EPSILON {
        "Layout"
    } else if (max - t.style_ms).abs() < f64::EPSILON {
        "Style"
    } else {
        "Paint"
    };
    let pct = max / t.total_ms * 100.0;
    format!("Bottleneck: {phase} ({pct:.0}% of frame)")
}

fn update_graph(panel: &Panel, history: &FrameHistory) {
    let budget_ms = 16.67f64;

    for (i, bar) in panel.graph_bars.iter().enumerate() {
        let frame = history.frame_at(i);
        let ms = frame.timing.total_ms;

        let height_pct = if ms <= 0.0 {
            0.0
        } else {
            (ms / (budget_ms * 2.0) * 100.0).min(100.0)
        };

        bar.style().h(pct(height_pct as f32));

        bar.class_remove("kdt-bar-warn");
        bar.class_remove("kdt-bar-jank");
        if ms > budget_ms {
            bar.class_add("kdt-bar-jank");
        } else if ms > budget_ms * 0.75 {
            bar.class_add("kdt-bar-warn");
        }
    }
}

fn update_live_info(
    panel: &Panel,
    ctx: &ViewContext,
    prev_zoom: &Rc<Cell<f64>>,
    prev_vp_w: &Rc<Cell<u32>>,
    prev_vp_h: &Rc<Cell<u32>>,
) {
    let doc = ctx.document();
    let vp = ctx.viewport();

    let nodes = doc.node_count();
    let elems = doc.element_count();
    panel
        .info_nodes
        .set_content(format!("{nodes} ({elems} el)"));

    panel.info_viewport.set_content(format!(
        "{:.0}x{:.0}",
        vp.logical_width(),
        vp.logical_height()
    ));

    let zoom = vp.page_zoom_factor();
    panel
        .info_zoom
        .set_content(format!("{:.0}%", zoom * 100.0));

    let old_zoom = prev_zoom.get();
    if (zoom - old_zoom).abs() > 0.001 {
        log_event(
            panel,
            doc,
            &format!("Zoom {:.0}% -> {:.0}%", old_zoom * 100.0, zoom * 100.0),
        );
        prev_zoom.set(zoom);
    }

    let w = vp.width();
    let h = vp.height();
    let old_w = prev_vp_w.get();
    let old_h = prev_vp_h.get();
    if w != old_w || h != old_h {
        log_event(
            panel,
            doc,
            &format!("Resize {old_w}x{old_h} -> {w}x{h}"),
        );
        prev_vp_w.set(w);
        prev_vp_h.set(h);
    }
}

fn log_event(panel: &Panel, doc: &Document, msg: &str) {
    let entry = doc.div();
    entry.class_add("kdt-log-entry");
    entry.append(doc.create_text(msg));

    if let Some(first) = panel.log_list.first_child() {
        first.insert_before(entry);
    } else {
        panel.log_list.append(entry);
    }

    let count = panel.log_count.get() + 1;
    panel.log_count.set(count);

    if count > LOG_MAX_ENTRIES {
        if let Some(last) = panel.log_list.last_child() {
            last.destroy();
            panel.log_count.set(count - 1);
        }
    }
}
