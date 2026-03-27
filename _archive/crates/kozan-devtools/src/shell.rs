//! DevTools shell — badge, panel chrome, tab bar, drag, fullscreen.

use std::cell::Cell;
use std::rc::Rc;

use kozan_core::styling::units::px;
use kozan_core::{
    ClickEvent, ContainerNode, Document, Element, EventTarget, HtmlDivElement,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Node, Text,
};
use kozan_platform::ViewContext;

use crate::metrics::{FrameHistory, FrameSnapshot};
use crate::performance::PerformanceTab;
use crate::recorder::FrameRecorder;

struct ShellState {
    expanded: Cell<bool>,
    fullscreen: Cell<bool>,
    dragging: Cell<bool>,
    drag_offset_x: Cell<f32>,
    drag_offset_y: Cell<f32>,
    pos_x: Cell<f32>,
    pos_y: Cell<f32>,
    log_expanded: Cell<bool>,
}

impl ShellState {
    fn new(x: f32, y: f32) -> Self {
        Self {
            expanded: Cell::new(false),
            fullscreen: Cell::new(false),
            dragging: Cell::new(false),
            drag_offset_x: Cell::new(0.0),
            drag_offset_y: Cell::new(0.0),
            pos_x: Cell::new(x),
            pos_y: Cell::new(y),
            log_expanded: Cell::new(false),
        }
    }
}

struct Badge {
    container: HtmlDivElement,
    fps_number: Text,
    fps_number_el: HtmlDivElement,
    fps_detail: Text,
}

struct PanelParts {
    panel: HtmlDivElement,
    tab_bar: HtmlDivElement,
    content: HtmlDivElement,
    close_btn: HtmlDivElement,
    fullscreen_btn: HtmlDivElement,
    record_btn: HtmlDivElement,
    reset_btn: HtmlDivElement,
    log_toggle: HtmlDivElement,
    log_arrow: Text,
    log_body: HtmlDivElement,
    log_section: HtmlDivElement,
}

pub fn build(doc: &Document, ctx: &ViewContext) -> HtmlDivElement {
    let root = doc.div();
    root.class_add("kdt-root");

    let initial_x = (ctx.viewport().logical_width() as f32 - 420.0).max(8.0);
    root.style().top(px(8.0)).left(px(initial_x));

    let state = Rc::new(ShellState::new(initial_x, 8.0));
    let history = Rc::new(FrameHistory::new());
    let recorder = Rc::new(FrameRecorder::new(3600));

    let badge = build_badge(doc);
    let parts = build_panel(doc);

    let perf_tab = Rc::new(PerformanceTab::build(doc, ctx));

    // Content: performance tab, then separator, then event log.
    parts.content.child(perf_tab.container);
    add_separator(doc, &parts.content);
    parts.content.child(parts.log_section);

    root.child(badge.container).child(parts.panel);

    wire_badge_expand(badge.container, parts.panel, &state);
    wire_close(parts.close_btn, parts.panel, badge.container, root, &state);
    wire_fullscreen(parts.fullscreen_btn, root, &state);
    wire_record(parts.record_btn, &recorder);
    wire_reset(parts.reset_btn, &history, &Rc::clone(&perf_tab));
    wire_drag(parts.tab_bar, root, &state);
    wire_log_toggle(parts.log_toggle, parts.log_body, parts.log_arrow, &state);

    let prev_zoom = Rc::new(Cell::new(ctx.page_zoom()));
    let prev_vp_w = Rc::new(Cell::new(ctx.viewport().width()));
    let prev_vp_h = Rc::new(Cell::new(ctx.viewport().height()));

    // Throttle: capture every frame for accuracy, but only update the
    // expensive panel DOM (~20 text nodes + 6 canvas redraws) at ~16/sec.
    // Badge (2 text nodes) always updates — negligible cost.
    const PANEL_UPDATE_INTERVAL_MS: f64 = 60.0;
    let last_panel_update = Rc::new(Cell::new(0.0f64));

    ctx.request_frame({
        let state = Rc::clone(&state);
        let history = Rc::clone(&history);
        let recorder = Rc::clone(&recorder);
        move |info, ctx| {
            let doc = ctx.document();
            let snap = FrameSnapshot {
                timing: info.prev_timing,
                fps: info.fps,
                frame_number: info.frame_number,
                budget_ms: info.frame_budget.as_secs_f64() * 1000.0,
                dom_node_count: doc.node_count() as u32,
                element_count: doc.element_count() as u32,
            };

            // Always capture — keeps avg/peak/jank accurate across ALL frames.
            history.push(snap);
            recorder.capture(&snap, info.timestamp.as_secs_f64() * 1000.0);

            // Badge: always update (just 2 text mutations, no layout impact).
            update_badge(&badge, &snap);

            // Panel: throttle expensive DOM text + chart updates.
            if state.expanded.get() {
                let now_ms = info.timestamp.as_secs_f64() * 1000.0;
                if now_ms - last_panel_update.get() >= PANEL_UPDATE_INTERVAL_MS {
                    last_panel_update.set(now_ms);
                    perf_tab.update(&snap, &history, ctx, &prev_zoom, &prev_vp_w, &prev_vp_h, info.frame_number);
                }
            }

            true
        }
    });

    root
}

/// Inline separator div.
fn add_separator(doc: &Document, parent: &HtmlDivElement) {
    let sep = doc.div();
    sep.class_add("kdt-sep");
    parent.child(sep);
}

fn wire_badge_expand(badge_el: HtmlDivElement, panel_el: HtmlDivElement, state: &Rc<ShellState>) {
    let state = Rc::clone(state);
    badge_el.on::<ClickEvent>(move |_, _| {
        state.expanded.set(true);
        badge_el.set_attribute("style", "display: none");
        panel_el.class_add("kdt-panel-open");
    });
}

fn wire_close(
    close_btn: HtmlDivElement,
    panel_el: HtmlDivElement,
    badge_el: HtmlDivElement,
    root_el: HtmlDivElement,
    state: &Rc<ShellState>,
) {
    let state = Rc::clone(state);
    close_btn.on::<ClickEvent>(move |_, _| {
        state.expanded.set(false);
        panel_el.class_remove("kdt-panel-open");
        badge_el.set_attribute("style", "display: flex");
        if state.fullscreen.get() {
            state.fullscreen.set(false);
            root_el.class_remove("kdt-fullscreen");
            root_el.style().top(px(state.pos_y.get())).left(px(state.pos_x.get()));
        }
    });
}

fn wire_fullscreen(btn: HtmlDivElement, root_el: HtmlDivElement, state: &Rc<ShellState>) {
    let state = Rc::clone(state);
    btn.on::<ClickEvent>(move |_, _| {
        let fs = !state.fullscreen.get();
        state.fullscreen.set(fs);
        if fs {
            root_el.class_add("kdt-fullscreen");
            root_el.set_attribute("style", "");
        } else {
            root_el.class_remove("kdt-fullscreen");
            root_el.style().top(px(state.pos_y.get())).left(px(state.pos_x.get()));
        }
    });
}

fn wire_record(btn: HtmlDivElement, recorder: &Rc<FrameRecorder>) {
    let recorder = Rc::clone(recorder);
    btn.on::<ClickEvent>(move |_, _| {
        if recorder.is_recording() {
            recorder.stop();
            btn.class_remove("kdt-recording");
        } else {
            recorder.start();
            btn.class_add("kdt-recording");
        }
    });
}

fn wire_reset(btn: HtmlDivElement, history: &Rc<FrameHistory>, perf_tab: &Rc<PerformanceTab>) {
    let history = Rc::clone(history);
    let perf_tab = Rc::clone(perf_tab);
    btn.on::<ClickEvent>(move |_, _| {
        history.reset();
        perf_tab.reset_charts();
    });
}

fn wire_drag(handle: HtmlDivElement, root_el: HtmlDivElement, state: &Rc<ShellState>) {
    {
        let state = Rc::clone(state);
        handle.on::<MouseDownEvent>(move |e, _| {
            if state.fullscreen.get() { return; }
            state.dragging.set(true);
            state.drag_offset_x.set(e.x - state.pos_x.get());
            state.drag_offset_y.set(e.y - state.pos_y.get());
        });
    }
    {
        let state = Rc::clone(state);
        root_el.on::<MouseMoveEvent>(move |e, _| {
            if !state.dragging.get() { return; }
            let x = (e.x - state.drag_offset_x.get()).max(0.0);
            let y = (e.y - state.drag_offset_y.get()).max(0.0);
            state.pos_x.set(x);
            state.pos_y.set(y);
            root_el.style().top(px(y)).left(px(x));
        });
    }
    {
        let state = Rc::clone(state);
        root_el.on::<MouseUpEvent>(move |_, _| { state.dragging.set(false); });
    }
}

fn wire_log_toggle(toggle: HtmlDivElement, body: HtmlDivElement, arrow: Text, state: &Rc<ShellState>) {
    let state = Rc::clone(state);
    toggle.on::<ClickEvent>(move |_, _| {
        let open = !state.log_expanded.get();
        state.log_expanded.set(open);
        if open {
            body.class_add("kdt-log-open");
            arrow.set_content("v");
        } else {
            body.class_remove("kdt-log-open");
            arrow.set_content(">");
        }
    });
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

    Badge { container, fps_number, fps_number_el, fps_detail }
}

fn update_badge(badge: &Badge, snap: &FrameSnapshot) {
    badge.fps_number.set_content(format!("{:.0}", snap.fps));
    badge.fps_number_el.class_remove("kdt-fps-green");
    badge.fps_number_el.class_remove("kdt-fps-yellow");
    badge.fps_number_el.class_remove("kdt-fps-red");

    let class = if snap.fps >= 55.0 { "kdt-fps-green" }
        else if snap.fps >= 30.0 { "kdt-fps-yellow" }
        else { "kdt-fps-red" };
    badge.fps_number_el.class_add(class);
    badge.fps_detail.set_content(format!("{:.1}ms", snap.timing.total_ms));
}

fn build_panel(doc: &Document) -> PanelParts {
    let panel = doc.div();
    panel.class_add("kdt-panel");

    // Tab bar / title bar
    let tab_bar = doc.div();
    tab_bar.class_add("kdt-tab-bar");

    let tab_btn = doc.div();
    tab_btn.class_add("kdt-tab");
    tab_btn.append(doc.create_text("Performance"));

    let spacer = doc.div();
    spacer.class_add("kdt-titlebar-spacer");

    let record = doc.div();
    record.class_add("kdt-record-btn");
    record.append(doc.create_text("Rec"));

    let reset = doc.div();
    reset.class_add("kdt-action-btn");
    reset.append(doc.create_text("Reset"));

    let fullscreen = doc.div();
    fullscreen.class_add("kdt-action-btn");
    fullscreen.append(doc.create_text("[ ]"));

    // Use × (multiplication sign) for close — cleaner than X
    let close = doc.div();
    close.class_add("kdt-close-btn");
    close.append(doc.create_text("\u{00d7}"));

    tab_bar
        .child(tab_btn)
        .child(spacer)
        .child(record)
        .child(reset)
        .child(fullscreen)
        .child(close);
    panel.child(tab_bar);

    let content = doc.div();
    content.class_add("kdt-tab-content");
    content.class_add("kdt-tab-content-active");
    panel.child(content);

    // Event log
    let log_section = doc.div();
    log_section.class_add("kdt-log-section");

    let log_toggle = doc.div();
    log_toggle.class_add("kdt-log-toggle");
    let log_arrow = doc.create_text(">");
    let arrow_el = doc.div();
    arrow_el.class_add("kdt-log-arrow");
    arrow_el.append(log_arrow);
    log_toggle.child(arrow_el);
    log_toggle.append(doc.create_text("Event Log"));

    let log_body = doc.div();
    log_body.class_add("kdt-log-body");
    let log_list = doc.div();
    log_list.class_add("kdt-log-list");
    log_body.child(log_list);
    log_section.child(log_toggle).child(log_body);

    PanelParts {
        panel, tab_bar, content,
        close_btn: close, fullscreen_btn: fullscreen,
        record_btn: record, reset_btn: reset,
        log_toggle, log_arrow, log_body, log_section,
    }
}
