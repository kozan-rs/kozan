//! `HTMLCanvasElement` — a canvas for 2D/WebGL rendering.
//!
//! Chrome equivalent: `HTMLCanvasElement` + `CanvasRenderingContext2D`.
//!
//! The element owns a persistent `CanvasRenderingContext2D` in the arena.
//! `Canvas2D` is a Copy handle that forwards drawing calls through
//! `DocumentCell`, matching the web's `canvas.getContext('2d')` pattern.

use std::sync::Arc;

use kozan_canvas::CanvasRecording;
use kozan_canvas::CanvasRenderingContext2D;
use kozan_primitives::color::Color;
use kozan_primitives::geometry::Rect;
use kozan_primitives::transform::AffineTransform;

use super::replaced::{IntrinsicSizing, ReplacedElement};
use crate::Handle;
use crate::dom::traits::HasHandle;
use kozan_canvas::blend::BlendMode;
use kozan_canvas::image::ImageData;
use kozan_canvas::line::{LineCap, LineJoin};
use kozan_canvas::style::{
    ConicGradient, FillRule, LinearGradient, PaintStyle, Pattern, RadialGradient,
};
use kozan_canvas::text::{TextAlign, TextBaseline, TextDirection};
use kozan_macros::{Element, Props};

const DEFAULT_CANVAS_WIDTH: f32 = 300.0;
const DEFAULT_CANVAS_HEIGHT: f32 = 150.0;

/// Chrome equivalent: `HTMLCanvasElement`.
#[derive(Copy, Clone, Element)]
#[element(tag = "canvas", data = CanvasData)]
pub struct HtmlCanvasElement(Handle);

/// Element-specific data stored in the Document arena.
///
/// `context` is the persistent `CanvasRenderingContext2D` (Chrome: owned by the element).
/// `committed` is the immutable snapshot read by the paint phase (Chrome: `PaintRecord`).
#[derive(Clone, Props)]
#[props(element = HtmlCanvasElement)]
pub struct CanvasData {
    #[prop]
    pub canvas_width: f32,
    #[prop]
    pub canvas_height: f32,
    pub(crate) context: CanvasRenderingContext2D,
    pub(crate) committed: Option<Arc<CanvasRecording>>,
}

impl Default for CanvasData {
    fn default() -> Self {
        Self {
            canvas_width: DEFAULT_CANVAS_WIDTH,
            canvas_height: DEFAULT_CANVAS_HEIGHT,
            context: CanvasRenderingContext2D::new(),
            committed: None,
        }
    }
}

impl HtmlCanvasElement {
    /// Chrome: `HTMLCanvasElement::getContext("2d")`.
    /// Returns a Copy handle — keep it, reuse it, just like the web.
    #[must_use]
    pub fn context_2d(&self) -> Canvas2D {
        Canvas2D(self.handle())
    }
}

impl ReplacedElement for HtmlCanvasElement {
    fn intrinsic_sizing(&self) -> IntrinsicSizing {
        IntrinsicSizing::from_size(self.canvas_width(), self.canvas_height())
    }
}

// ── Canvas2D — the web-like rendering context handle ─────────────────

/// The Canvas 2D rendering context — a handle to the persistent context
/// stored in the element's arena data.
///
/// Chrome equivalent: the `CanvasRenderingContext2D` JS object.
/// Copy, 16 bytes. Get it once via `canvas.context_2d()`, keep forever.
/// Every draw call records ops through `DocumentCell`. The paint lifecycle
/// auto-flushes recordings — zero manual commit.
///
/// ```ignore
/// let ctx = canvas.context_2d();
/// ctx.set_fill_color(Color::RED);
/// ctx.fill_rect(10.0, 20.0, 100.0, 50.0);
/// // Done. Paint phase reads automatically.
/// ```
#[derive(Copy, Clone)]
pub struct Canvas2D(pub(crate) Handle);

/// Write to the canvas context through DocumentCell.
macro_rules! canvas_write {
    ($self:expr, |$ctx:ident| $body:expr) => {{
        let h = $self.0;
        h.cell.write(|doc| {
            doc.canvas_draw(h.id.index(), |$ctx| { $body });
        });
    }};
}

/// Read from the canvas context through DocumentCell.
macro_rules! canvas_read {
    ($self:expr, |$ctx:ident| $body:expr) => {{
        let h = $self.0;
        h.cell.read(|doc| doc.canvas_read(h.id.index(), |$ctx| $body))
    }};
}

impl Canvas2D {
    // ── State ────────────────────────────────────────────────────────

    pub fn save(&self) { canvas_write!(self, |ctx| ctx.save()); }
    pub fn restore(&self) { canvas_write!(self, |ctx| ctx.restore()); }
    pub fn reset(&self) { canvas_write!(self, |ctx| ctx.reset()); }

    // ── Transform ────────────────────────────────────────────────────

    pub fn translate(&self, tx: f32, ty: f32) {
        canvas_write!(self, |ctx| ctx.translate(tx, ty));
    }
    pub fn rotate(&self, angle: f32) {
        canvas_write!(self, |ctx| ctx.rotate(angle));
    }
    pub fn scale(&self, sx: f32, sy: f32) {
        canvas_write!(self, |ctx| ctx.scale(sx, sy));
    }
    pub fn transform(&self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        canvas_write!(self, |ctx| ctx.transform(a, b, c, d, e, f));
    }
    pub fn set_transform(&self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        canvas_write!(self, |ctx| ctx.set_transform(a, b, c, d, e, f));
    }
    pub fn set_transform_matrix(&self, t: AffineTransform) {
        canvas_write!(self, |ctx| ctx.set_transform_matrix(t));
    }
    pub fn reset_transform(&self) {
        canvas_write!(self, |ctx| ctx.reset_transform());
    }
    #[must_use]
    pub fn get_transform(&self) -> AffineTransform {
        canvas_read!(self, |ctx| ctx.get_transform())
    }

    // ── Style setters ────────────────────────────────────────────────

    pub fn set_fill_color(&self, color: Color) {
        canvas_write!(self, |ctx| ctx.set_fill_color(color));
    }
    pub fn set_fill_style(&self, style: PaintStyle) {
        canvas_write!(self, |ctx| ctx.set_fill_style(style));
    }
    pub fn set_stroke_color(&self, color: Color) {
        canvas_write!(self, |ctx| ctx.set_stroke_color(color));
    }
    pub fn set_stroke_style(&self, style: PaintStyle) {
        canvas_write!(self, |ctx| ctx.set_stroke_style(style));
    }
    pub fn set_line_width(&self, w: f32) {
        canvas_write!(self, |ctx| ctx.set_line_width(w));
    }
    pub fn set_line_cap(&self, cap: LineCap) {
        canvas_write!(self, |ctx| ctx.set_line_cap(cap));
    }
    pub fn set_line_join(&self, join: LineJoin) {
        canvas_write!(self, |ctx| ctx.set_line_join(join));
    }
    pub fn set_miter_limit(&self, limit: f32) {
        canvas_write!(self, |ctx| ctx.set_miter_limit(limit));
    }
    pub fn set_line_dash(&self, dash: Vec<f32>) {
        canvas_write!(self, |ctx| ctx.set_line_dash(dash));
    }
    pub fn set_line_dash_offset(&self, offset: f32) {
        canvas_write!(self, |ctx| ctx.set_line_dash_offset(offset));
    }
    pub fn set_global_alpha(&self, alpha: f32) {
        canvas_write!(self, |ctx| ctx.set_global_alpha(alpha));
    }
    pub fn set_global_composite_operation(&self, op: BlendMode) {
        canvas_write!(self, |ctx| ctx.set_global_composite_operation(op));
    }
    pub fn set_shadow_offset_x(&self, x: f32) {
        canvas_write!(self, |ctx| ctx.set_shadow_offset_x(x));
    }
    pub fn set_shadow_offset_y(&self, y: f32) {
        canvas_write!(self, |ctx| ctx.set_shadow_offset_y(y));
    }
    pub fn set_shadow_blur(&self, blur: f32) {
        canvas_write!(self, |ctx| ctx.set_shadow_blur(blur));
    }
    pub fn set_shadow_color(&self, color: Color) {
        canvas_write!(self, |ctx| ctx.set_shadow_color(color));
    }
    pub fn set_image_smoothing_enabled(&self, enabled: bool) {
        canvas_write!(self, |ctx| ctx.set_image_smoothing_enabled(enabled));
    }
    pub fn set_font(&self, font: String) {
        canvas_write!(self, |ctx| ctx.set_font(font));
    }
    pub fn set_text_align(&self, align: TextAlign) {
        canvas_write!(self, |ctx| ctx.set_text_align(align));
    }
    pub fn set_text_baseline(&self, baseline: TextBaseline) {
        canvas_write!(self, |ctx| ctx.set_text_baseline(baseline));
    }
    pub fn set_direction(&self, direction: TextDirection) {
        canvas_write!(self, |ctx| ctx.set_direction(direction));
    }

    // ── Style getters ────────────────────────────────────────────────

    #[must_use]
    pub fn line_width(&self) -> f32 { canvas_read!(self, |ctx| ctx.line_width()) }
    #[must_use]
    pub fn line_cap(&self) -> LineCap { canvas_read!(self, |ctx| ctx.line_cap()) }
    #[must_use]
    pub fn line_join(&self) -> LineJoin { canvas_read!(self, |ctx| ctx.line_join()) }
    #[must_use]
    pub fn miter_limit(&self) -> f32 { canvas_read!(self, |ctx| ctx.miter_limit()) }
    #[must_use]
    pub fn line_dash_offset(&self) -> f32 { canvas_read!(self, |ctx| ctx.line_dash_offset()) }
    #[must_use]
    pub fn global_alpha(&self) -> f32 { canvas_read!(self, |ctx| ctx.global_alpha()) }
    #[must_use]
    pub fn global_composite_operation(&self) -> BlendMode {
        canvas_read!(self, |ctx| ctx.global_composite_operation())
    }
    #[must_use]
    pub fn shadow_offset_x(&self) -> f32 { canvas_read!(self, |ctx| ctx.shadow_offset_x()) }
    #[must_use]
    pub fn shadow_offset_y(&self) -> f32 { canvas_read!(self, |ctx| ctx.shadow_offset_y()) }
    #[must_use]
    pub fn shadow_blur(&self) -> f32 { canvas_read!(self, |ctx| ctx.shadow_blur()) }
    #[must_use]
    pub fn shadow_color(&self) -> Color { canvas_read!(self, |ctx| ctx.shadow_color()) }
    #[must_use]
    pub fn image_smoothing_enabled(&self) -> bool {
        canvas_read!(self, |ctx| ctx.image_smoothing_enabled())
    }
    #[must_use]
    pub fn text_align(&self) -> TextAlign { canvas_read!(self, |ctx| ctx.text_align()) }
    #[must_use]
    pub fn text_baseline(&self) -> TextBaseline { canvas_read!(self, |ctx| ctx.text_baseline()) }

    // ── Rectangle drawing ────────────────────────────────────────────

    pub fn fill_rect(&self, x: f32, y: f32, w: f32, h: f32) {
        canvas_write!(self, |ctx| ctx.fill_rect(x, y, w, h));
    }
    pub fn stroke_rect(&self, x: f32, y: f32, w: f32, h: f32) {
        canvas_write!(self, |ctx| ctx.stroke_rect(x, y, w, h));
    }
    pub fn clear_rect(&self, x: f32, y: f32, w: f32, h: f32) {
        canvas_write!(self, |ctx| ctx.clear_rect(x, y, w, h));
    }

    // ── Path building ────────────────────────────────────────────────

    pub fn begin_path(&self) { canvas_write!(self, |ctx| ctx.begin_path()); }
    pub fn move_to(&self, x: f32, y: f32) { canvas_write!(self, |ctx| ctx.move_to(x, y)); }
    pub fn line_to(&self, x: f32, y: f32) { canvas_write!(self, |ctx| ctx.line_to(x, y)); }
    pub fn quadratic_curve_to(&self, cpx: f32, cpy: f32, x: f32, y: f32) {
        canvas_write!(self, |ctx| ctx.quadratic_curve_to(cpx, cpy, x, y));
    }
    pub fn bezier_curve_to(&self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        canvas_write!(self, |ctx| ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y));
    }
    pub fn arc(&self, x: f32, y: f32, r: f32, start: f32, end: f32, ccw: bool) {
        canvas_write!(self, |ctx| ctx.arc(x, y, r, start, end, ccw));
    }
    pub fn arc_to(&self, x1: f32, y1: f32, x2: f32, y2: f32, r: f32) {
        canvas_write!(self, |ctx| ctx.arc_to(x1, y1, x2, y2, r));
    }
    pub fn ellipse(
        &self, x: f32, y: f32, rx: f32, ry: f32,
        rotation: f32, start: f32, end: f32, ccw: bool,
    ) {
        canvas_write!(self, |ctx| ctx.ellipse(x, y, rx, ry, rotation, start, end, ccw));
    }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) {
        canvas_write!(self, |ctx| ctx.rect(x, y, w, h));
    }
    pub fn round_rect(&self, x: f32, y: f32, w: f32, h: f32, radii: [f32; 4]) {
        canvas_write!(self, |ctx| ctx.round_rect(x, y, w, h, radii));
    }
    pub fn close_path(&self) { canvas_write!(self, |ctx| ctx.close_path()); }

    // ── Path drawing ─────────────────────────────────────────────────

    pub fn fill(&self) { canvas_write!(self, |ctx| ctx.fill()); }
    pub fn fill_with_rule(&self, rule: FillRule) {
        canvas_write!(self, |ctx| ctx.fill_with_rule(rule));
    }
    pub fn stroke(&self) { canvas_write!(self, |ctx| ctx.stroke()); }
    pub fn clip(&self) { canvas_write!(self, |ctx| ctx.clip()); }
    pub fn clip_with_rule(&self, rule: FillRule) {
        canvas_write!(self, |ctx| ctx.clip_with_rule(rule));
    }

    // ── Text ─────────────────────────────────────────────────────────

    pub fn fill_text(&self, text: &str, x: f32, y: f32) {
        let t = text.to_string();
        canvas_write!(self, |ctx| ctx.fill_text(&t, x, y));
    }
    pub fn fill_text_max_width(&self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        let t = text.to_string();
        canvas_write!(self, |ctx| ctx.fill_text_max_width(&t, x, y, max_width));
    }
    pub fn stroke_text(&self, text: &str, x: f32, y: f32) {
        let t = text.to_string();
        canvas_write!(self, |ctx| ctx.stroke_text(&t, x, y));
    }
    pub fn stroke_text_max_width(&self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        let t = text.to_string();
        canvas_write!(self, |ctx| ctx.stroke_text_max_width(&t, x, y, max_width));
    }

    // ── Image ────────────────────────────────────────────────────────

    pub fn draw_image(&self, data: Arc<ImageData>, src: Rect, dst: Rect) {
        canvas_write!(self, |ctx| ctx.draw_image(data, src, dst));
    }

    // ── Pixel manipulation ───────────────────────────────────────────

    pub fn put_image_data(&self, data: ImageData, dx: f32, dy: f32) {
        canvas_write!(self, |ctx| ctx.put_image_data(data, dx, dy));
    }

    // ── Gradient/Pattern factories (static, no element access) ───────

    #[must_use]
    pub fn create_linear_gradient(x0: f32, y0: f32, x1: f32, y1: f32) -> LinearGradient {
        CanvasRenderingContext2D::create_linear_gradient(x0, y0, x1, y1)
    }
    #[must_use]
    pub fn create_radial_gradient(
        x0: f32, y0: f32, r0: f32, x1: f32, y1: f32, r1: f32,
    ) -> RadialGradient {
        CanvasRenderingContext2D::create_radial_gradient(x0, y0, r0, x1, y1, r1)
    }
    #[must_use]
    pub fn create_conic_gradient(start_angle: f32, cx: f32, cy: f32) -> ConicGradient {
        CanvasRenderingContext2D::create_conic_gradient(start_angle, cx, cy)
    }
    #[must_use]
    pub fn create_pattern(
        image: ImageData,
        repetition: kozan_canvas::style::PatternRepetition,
    ) -> Pattern {
        CanvasRenderingContext2D::create_pattern(image, repetition)
    }
}

// ── Fragment-side replaced content ───────────────────────────────────

/// Chrome: the canvas `PaintRecord` attached to the fragment during layout.
#[derive(Debug, Clone)]
pub struct CanvasContent {
    pub recording: Arc<CanvasRecording>,
    pub canvas_width: f32,
    pub canvas_height: f32,
}

impl crate::layout::fragment::ReplacedContent for CanvasContent {
    fn to_draw_command(&self) -> crate::paint::display_item::DrawCommand {
        crate::paint::display_item::DrawCommand::Canvas {
            recording: Arc::clone(&self.recording),
            x: 0.0,
            y: 0.0,
            canvas_width: self.canvas_width,
            canvas_height: self.canvas_height,
            layout_width: self.canvas_width,
            layout_height: self.canvas_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;

    #[test]
    fn canvas_default_size() {
        let doc = Document::new();
        let canvas = doc.create::<HtmlCanvasElement>();
        let sizing = canvas.intrinsic_sizing();
        assert_eq!(sizing.width, Some(300.0));
        assert_eq!(sizing.height, Some(150.0));
    }

    #[test]
    fn canvas_custom_size() {
        let doc = Document::new();
        let canvas = doc.create::<HtmlCanvasElement>();
        canvas.set_canvas_width(1024.0);
        canvas.set_canvas_height(768.0);
        let sizing = canvas.intrinsic_sizing();
        assert_eq!(sizing.width, Some(1024.0));
        assert_eq!(sizing.height, Some(768.0));
    }

    #[test]
    fn context_2d_draws_through_handle() {
        let doc = Document::new();
        let canvas = doc.create::<HtmlCanvasElement>();
        let ctx = canvas.context_2d();

        ctx.set_fill_color(Color::RED);
        ctx.fill_rect(10.0, 20.0, 100.0, 50.0);
        ctx.begin_path();
        ctx.move_to(0.0, 0.0);
        ctx.line_to(50.0, 50.0);
        ctx.stroke();

        // Verify recording accumulated in arena (not committed yet — needs flush)
        canvas
            .handle()
            .write_data::<CanvasData, ()>(|data| {
                assert!(!data.context.take_recording().is_empty());
            })
            .expect("canvas data exists");
    }

    #[test]
    fn state_persists_across_calls() {
        let doc = Document::new();
        let canvas = doc.create::<HtmlCanvasElement>();
        let ctx = canvas.context_2d();

        ctx.set_global_alpha(0.5);
        assert_eq!(ctx.global_alpha(), 0.5);

        ctx.set_line_width(3.0);
        assert_eq!(ctx.line_width(), 3.0);

        // State persists — same context, same arena data
        assert_eq!(ctx.global_alpha(), 0.5);
    }
}
