//! Area chart widget — Canvas 2D drawn smooth filled area charts.
//!
//! Chrome: Performance Monitor chart lanes. Each lane shows a single
//! metric as a gradient-filled area chart with a solid line on top.
//!
//! Draws directly via Canvas 2D — zero DOM nodes per chart, zero
//! layout cost per update.

use std::cell::Cell;
use std::rc::Rc;

use kozan_core::{
    Canvas2D, ClickEvent, ContainerNode, Document, Element, EventTarget, HtmlCanvasElement,
    HtmlDivElement, Text,
};
use kozan_primitives::color::Color;

use crate::metrics::HISTORY_SIZE;

const CHART_HEIGHT: f32 = 32.0;

/// A single chart lane — label, current value, avg/peak stats, and canvas area chart.
pub struct AreaChart {
    /// The full lane row element.
    pub lane: HtmlDivElement,
    /// Canvas element — needed for reading display size after layout.
    canvas: HtmlCanvasElement,
    /// Canvas 2D rendering context handle.
    ctx: Canvas2D,
    /// Current value display.
    value_text: Text,
    /// Avg/peak sub-text.
    stats_text: Text,
    /// Whether this chart lane is visible.
    enabled: Rc<Cell<bool>>,
    /// Ring buffer of normalized values (0.0–1.0).
    values: [Cell<f32>; HISTORY_SIZE],
    /// Write position in the ring buffer.
    write_idx: Cell<usize>,
    /// Running average of raw values.
    avg_accum: Cell<f64>,
    /// Peak raw value seen.
    peak: Cell<f64>,
    /// Total samples for running average.
    total_samples: Cell<u64>,
    /// Line color (trace on top, solid).
    line_color: Color,
    /// Fill color top (area gradient start — line_color at 0.25 alpha).
    fill_top: Color,
    /// Fill color bottom (area gradient end — line_color at 0.02 alpha).
    fill_bottom: Color,
}

impl AreaChart {
    /// Build a chart lane: [toggle] label  value+stats  |canvas chart|
    ///
    /// `line_color`: the solid color for the chart trace line.
    /// The area fill is a vertical gradient from `line_color@0.25` to `line_color@0.02`.
    pub fn build(doc: &Document, label: &str, color_class: &str, line_color: Color) -> Self {
        let lane = doc.div();
        lane.class_add("kdt-chart-lane");

        // Toggle dot (colored square)
        let toggle = doc.div();
        toggle.class_add("kdt-chart-toggle");
        toggle.class_add(color_class);
        lane.child(toggle);

        // Label
        let label_el = doc.div();
        label_el.class_add("kdt-chart-label");
        label_el.append(doc.create_text(label));
        lane.child(label_el);

        // Value block (current value + avg/peak sub-text)
        let value_block = doc.div();
        value_block.class_add("kdt-chart-value-block");

        let value_text = doc.create_text("--");
        let value_el = doc.div();
        value_el.class_add("kdt-chart-value");
        value_el.append(value_text);
        value_block.child(value_el);

        let stats_text = doc.create_text("");
        let stats_el = doc.div();
        stats_el.class_add("kdt-chart-stats");
        stats_el.append(stats_text);
        value_block.child(stats_el);

        lane.child(value_block);

        // Canvas chart
        let canvas = doc.create::<HtmlCanvasElement>();
        canvas.set_canvas_width(HISTORY_SIZE as f32);
        canvas.set_canvas_height(CHART_HEIGHT);
        canvas.class_add("kdt-chart-canvas");
        let ctx = canvas.context_2d();
        lane.child(canvas);

        let fill_top = Color::rgba(line_color.r, line_color.g, line_color.b, 0.25);
        let fill_bottom = Color::rgba(line_color.r, line_color.g, line_color.b, 0.02);
        let enabled = Rc::new(Cell::new(true));

        // Toggle click handler
        {
            let enabled = Rc::clone(&enabled);
            let canvas_el = canvas;
            let toggle_el = toggle;
            toggle_el.on::<ClickEvent>(move |_, _| {
                let on = !enabled.get();
                enabled.set(on);
                if on {
                    canvas_el.class_remove("kdt-chart-hidden");
                    toggle_el.class_remove("kdt-chart-toggle-off");
                } else {
                    canvas_el.class_add("kdt-chart-hidden");
                    toggle_el.class_add("kdt-chart-toggle-off");
                }
            });
        }

        Self {
            lane,
            canvas,
            ctx,
            value_text,
            stats_text,
            enabled,
            values: std::array::from_fn(|_| Cell::new(0.0)),
            write_idx: Cell::new(0),
            avg_accum: Cell::new(0.0),
            peak: Cell::new(0.0),
            total_samples: Cell::new(0),
            line_color,
            fill_top,
            fill_bottom,
        }
    }

    /// Push a new value and redraw the chart.
    ///
    /// `normalized`: 0.0–1.0 range for bar height.
    /// `raw`: actual metric value for avg/peak tracking.
    /// `display`: text for current value (e.g., "60", "0.3").
    /// `unit`: suffix for avg/peak display (e.g., "ms", "%", "").
    pub fn update(&self, normalized: f32, raw: f64, display: &str, unit: &str) {
        let n = self.total_samples.get() + 1;
        self.total_samples.set(n);

        if raw > self.peak.get() {
            self.peak.set(raw);
        }

        let prev_avg = self.avg_accum.get();
        self.avg_accum.set(prev_avg + (raw - prev_avg) / n as f64);

        let avg = self.avg_accum.get();
        let peak = self.peak.get();
        self.stats_text
            .set_content(format!("{avg:.1} / {peak:.1}{unit}"));

        if !self.enabled.get() {
            self.value_text.set_content(display);
            return;
        }

        let idx = self.write_idx.get();
        self.values[idx].set(normalized.clamp(0.0, 1.0));
        self.write_idx.set((idx + 1) % HISTORY_SIZE);

        // Sync canvas buffer to actual display size — prevents stretching.
        // Like web: canvas.width = canvas.getBoundingClientRect().width
        self.sync_canvas_size();

        self.redraw();
        self.value_text.set_content(display);
    }

    /// Sync canvas pixel buffer to match CSS display size.
    ///
    /// Only resizes when the display size actually changed. This runs at
    /// most ~16/sec (throttled by shell), so the offset_width/height
    /// reads are negligible.
    fn sync_canvas_size(&self) {
        let display_w = self.canvas.offset_width();
        let display_h = self.canvas.offset_height();
        if display_w < 1.0 || display_h < 1.0 {
            return;
        }
        let cur_w = self.canvas.canvas_width();
        let cur_h = self.canvas.canvas_height();
        if (cur_w - display_w).abs() > 0.5 || (cur_h - display_h).abs() > 0.5 {
            self.canvas.set_canvas_width(display_w);
            self.canvas.set_canvas_height(display_h);
        }
    }

    /// Redraw the entire area chart on canvas with gradient fill.
    fn redraw(&self) {
        let ctx = self.ctx;
        let w = self.canvas.canvas_width();
        let h = self.canvas.canvas_height();
        let idx = self.write_idx.get();

        // Clear + subtle background.
        ctx.clear_rect(0.0, 0.0, w, h);
        ctx.set_fill_color(Color::rgba(1.0, 1.0, 1.0, 0.015));
        ctx.fill_rect(0.0, 0.0, w, h);

        // 50% grid line.
        ctx.set_stroke_color(Color::rgba(1.0, 1.0, 1.0, 0.04));
        ctx.set_line_width(0.5);
        ctx.begin_path();
        ctx.move_to(0.0, h * 0.5);
        ctx.line_to(w, h * 0.5);
        ctx.stroke();

        // X step: spread HISTORY_SIZE samples across actual canvas width.
        let x_step = if HISTORY_SIZE > 1 { w / (HISTORY_SIZE - 1) as f32 } else { w };

        // Area fill — gradient approximated with two passes (no canvas gradients yet).

        // Pass 1: full area at bottom alpha
        ctx.begin_path();
        ctx.move_to(0.0, h);
        for i in 0..HISTORY_SIZE {
            let ring_idx = (idx + 1 + i) % HISTORY_SIZE;
            let v = self.values[ring_idx].get();
            ctx.line_to(i as f32 * x_step, h * (1.0 - v));
        }
        ctx.line_to(w, h);
        ctx.close_path();
        ctx.set_fill_color(self.fill_bottom);
        ctx.fill();

        // Pass 2: upper portion at higher alpha
        ctx.begin_path();
        ctx.move_to(0.0, h);
        for i in 0..HISTORY_SIZE {
            let ring_idx = (idx + 1 + i) % HISTORY_SIZE;
            let v = self.values[ring_idx].get();
            ctx.line_to(i as f32 * x_step, h * (1.0 - v));
        }
        ctx.line_to(w, h);
        ctx.close_path();
        ctx.set_fill_color(self.fill_top);
        ctx.fill();

        // Line trace on top.
        ctx.begin_path();
        for i in 0..HISTORY_SIZE {
            let ring_idx = (idx + 1 + i) % HISTORY_SIZE;
            let v = self.values[ring_idx].get();
            let x = i as f32 * x_step;
            let y = h * (1.0 - v);
            if i == 0 {
                ctx.move_to(x, y);
            } else {
                ctx.line_to(x, y);
            }
        }
        ctx.set_stroke_color(self.line_color);
        ctx.set_line_width(1.5);
        ctx.stroke();
    }

    /// Clear accumulated avg/peak stats and chart history.
    pub fn reset(&self) {
        self.avg_accum.set(0.0);
        self.peak.set(0.0);
        self.total_samples.set(0);
        for v in &self.values {
            v.set(0.0);
        }
        self.write_idx.set(0);
        self.stats_text.set_content("");

        self.ctx
            .clear_rect(0.0, 0.0, self.canvas.canvas_width(), self.canvas.canvas_height());
    }
}
