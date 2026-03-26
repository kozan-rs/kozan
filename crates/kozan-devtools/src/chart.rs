//! Area chart widget — dense 1px bars creating smooth filled charts.
//!
//! Chrome: Performance Monitor chart lanes. Each lane shows a single
//! metric as a filled area chart with a solid line on top.
//!
//! Since Kozan has no Canvas or SVG, we simulate area charts with
//! densely packed 1px-wide div bars. Zero gap between bars creates
//! the visual effect of a smooth filled area. CSS `border-top` on
//! each bar creates the line trace.

use std::cell::Cell;
use std::rc::Rc;

use kozan_core::styling::units::pct;
use kozan_core::{ClickEvent, ContainerNode, Document, Element, EventTarget, HtmlDivElement, Node, Text};

use crate::metrics::HISTORY_SIZE;

/// A single chart lane — label, current value, avg/peak stats, and area chart.
pub struct AreaChart {
    /// The full lane row element.
    pub lane: HtmlDivElement,
    /// 120 x 1px bar divs.
    bars: Vec<HtmlDivElement>,
    /// Current value display.
    value_text: Text,
    /// Avg/peak sub-text.
    stats_text: Text,
    /// Whether this chart lane is visible.
    enabled: Rc<Cell<bool>>,
    /// Ring buffer of normalized values (0.0-1.0).
    values: [Cell<f32>; HISTORY_SIZE],
    /// Write position in the ring buffer.
    write_idx: Cell<usize>,
    /// Running average of raw values.
    avg_accum: Cell<f64>,
    /// Peak raw value seen.
    peak: Cell<f64>,
    /// Total samples for running average.
    total_samples: Cell<u64>,
}

impl AreaChart {
    /// Build a chart lane: [toggle] label  value+stats  |area chart|
    pub fn build(doc: &Document, label: &str, color_class: &str) -> Self {
        let lane = doc.div();
        lane.class_add("kdt-chart-lane");

        // Toggle checkbox (colored square)
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

        // Chart area container
        let chart_area = doc.div();
        chart_area.class_add("kdt-chart-area");
        chart_area.class_add(color_class);

        // Grid line at 50%
        let grid_50 = doc.div();
        grid_50.class_add("kdt-chart-grid");
        grid_50.set_attribute("style", "bottom: 50%");
        chart_area.child(grid_50);

        // 120 bars
        let mut bars = Vec::with_capacity(HISTORY_SIZE);
        for _ in 0..HISTORY_SIZE {
            let bar = doc.div();
            bar.class_add("kdt-chart-bar");
            bar.style().h(pct(0.0));
            chart_area.child(bar);
            bars.push(bar);
        }

        lane.child(chart_area);

        let enabled = Rc::new(Cell::new(true));

        // Toggle click handler
        {
            let enabled = Rc::clone(&enabled);
            let chart_area_el = chart_area;
            let toggle_el = toggle;
            toggle_el.on::<ClickEvent>(move |_, _| {
                let on = !enabled.get();
                enabled.set(on);
                if on {
                    chart_area_el.class_remove("kdt-chart-hidden");
                    toggle_el.class_remove("kdt-chart-toggle-off");
                } else {
                    chart_area_el.class_add("kdt-chart-hidden");
                    toggle_el.class_add("kdt-chart-toggle-off");
                }
            });
        }

        Self {
            lane,
            bars,
            value_text,
            stats_text,
            enabled,
            values: std::array::from_fn(|_| Cell::new(0.0)),
            write_idx: Cell::new(0),
            avg_accum: Cell::new(0.0),
            peak: Cell::new(0.0),
            total_samples: Cell::new(0),
        }
    }

    /// Push a new value and update bar heights + stats.
    ///
    /// `normalized`: 0.0-1.0 range for bar height.
    /// `raw`: actual metric value for avg/peak tracking.
    /// `display`: text for current value (e.g., "60", "0.3").
    /// `unit`: suffix for avg/peak display (e.g., "ms", "%", "").
    pub fn update(&self, normalized: f32, raw: f64, display: &str, unit: &str) {
        // Track avg/peak from raw values.
        let n = self.total_samples.get() + 1;
        self.total_samples.set(n);

        if raw > self.peak.get() {
            self.peak.set(raw);
        }

        let prev_avg = self.avg_accum.get();
        self.avg_accum.set(prev_avg + (raw - prev_avg) / n as f64);

        // Update stats sub-text.
        let avg = self.avg_accum.get();
        let peak = self.peak.get();
        self.stats_text
            .set_content(format!("{avg:.1} / {peak:.1}{unit}"));

        if !self.enabled.get() {
            self.value_text.set_content(display);
            return;
        }

        // Write new value into ring buffer.
        let idx = self.write_idx.get();
        self.values[idx].set(normalized.clamp(0.0, 1.0));
        self.write_idx.set((idx + 1) % HISTORY_SIZE);

        // Update all bar heights from ring buffer (oldest to newest).
        for (i, bar) in self.bars.iter().enumerate() {
            let ring_idx = (idx + 1 + i) % HISTORY_SIZE;
            let v = self.values[ring_idx].get();
            bar.style().h(pct(v * 100.0));
        }

        self.value_text.set_content(display);
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
    }
}
