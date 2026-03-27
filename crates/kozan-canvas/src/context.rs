//! The Canvas 2D rendering context — the user-facing API.
//!
//! Chrome equivalent: `CanvasRenderingContext2D` + `Canvas2DRecorderContext`.
//! Records drawing commands into a `CanvasRecording` for later replay
//! by a backend-specific player.

use std::sync::Arc;

use kozan_primitives::color::Color;
use kozan_primitives::geometry::Rect;
use kozan_primitives::transform::AffineTransform;

use crate::blend::BlendMode;
use crate::image::ImageData;
use crate::line::{LineCap, LineJoin};
use crate::op::{CanvasOp, ResolvedPaint, ResolvedStroke};
use crate::path::Path2D;
use crate::recording::CanvasRecording;
use crate::state::CanvasStateStack;
use crate::style::{
    ConicGradient, FillRule, GradientStop, LinearGradient, PaintStyle, Pattern, RadialGradient,
};
use crate::text::{TextAlign, TextBaseline, TextDirection};

/// The Canvas 2D rendering context.
///
/// Chrome equivalent: `CanvasRenderingContext2D`.
/// Owns the state stack, current path, and recording buffer.
/// All drawing methods record ops with baked-in style properties —
/// no rendering happens until `take_recording()` is consumed by a player.
#[derive(Clone)]
pub struct CanvasRenderingContext2D {
    state: CanvasStateStack,
    path: Path2D,
    recording: CanvasRecording,
}

impl CanvasRenderingContext2D {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: CanvasStateStack::new(),
            path: Path2D::new(),
            recording: CanvasRecording::new(),
        }
    }

    // ── Recording extraction ─────────────────────────────────────────

    /// Takes the recording, leaving the context ready for new commands.
    ///
    /// Chrome equivalent: `PaintOpBuffer::ReleaseAsRecord()`.
    pub fn take_recording(&mut self) -> CanvasRecording {
        self.recording.take()
    }

    // ── State management ─────────────────────────────────────────────

    /// Push the current state onto the stack and record a Save op.
    ///
    /// Chrome: `Canvas2DRecorderContext::save()` — saves both blink-side
    /// state AND records `SaveOp` for transform/clip replay.
    pub fn save(&mut self) {
        self.state.save();
        self.recording.push(CanvasOp::Save);
    }

    /// Pop the state stack and record a Restore op.
    pub fn restore(&mut self) {
        if self.state.restore() {
            self.recording.push(CanvasOp::Restore);
        }
    }

    /// Reset the context to its initial state.
    pub fn reset(&mut self) {
        self.state.reset();
        self.path.clear();
        self.recording = CanvasRecording::new();
    }

    // ── Transform ────────────────────────────────────────────────────

    pub fn translate(&mut self, tx: f32, ty: f32) {
        let s = self.state.current_mut();
        s.transform = s.transform.then(&AffineTransform::translate(tx, ty));
        self.recording.push(CanvasOp::Translate { tx, ty });
    }

    pub fn rotate(&mut self, angle: f32) {
        let s = self.state.current_mut();
        s.transform = s.transform.then(&AffineTransform::rotate(angle));
        self.recording.push(CanvasOp::Rotate { angle });
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        let s = self.state.current_mut();
        s.transform = s.transform.then(&AffineTransform::scale(sx, sy));
        self.recording.push(CanvasOp::Scale { sx, sy });
    }

    /// Multiply the current transform by the given matrix.
    pub fn transform(
        &mut self,
        a: f32, b: f32,
        c: f32, d: f32,
        e: f32, f: f32,
    ) {
        let m = AffineTransform::new(a, b, c, d, e, f);
        let s = self.state.current_mut();
        s.transform = s.transform.then(&m);
        self.recording.push(CanvasOp::SetTransform(s.transform));
    }

    /// Replace the current transform entirely.
    pub fn set_transform(
        &mut self,
        a: f32, b: f32,
        c: f32, d: f32,
        e: f32, f: f32,
    ) {
        let m = AffineTransform::new(a, b, c, d, e, f);
        self.state.current_mut().transform = m;
        self.recording.push(CanvasOp::SetTransform(m));
    }

    pub fn set_transform_matrix(&mut self, transform: AffineTransform) {
        self.state.current_mut().transform = transform;
        self.recording.push(CanvasOp::SetTransform(transform));
    }

    pub fn reset_transform(&mut self) {
        self.state.current_mut().transform = AffineTransform::IDENTITY;
        self.recording.push(CanvasOp::ResetTransform);
    }

    #[must_use]
    pub fn get_transform(&self) -> AffineTransform {
        self.state.current().transform
    }

    // ── Style properties (setters) ───────────────────────────────────

    pub fn set_fill_color(&mut self, color: Color) {
        self.state.current_mut().fill_style = PaintStyle::Color(color);
    }

    pub fn set_fill_style(&mut self, style: PaintStyle) {
        self.state.current_mut().fill_style = style;
    }

    pub fn set_stroke_color(&mut self, color: Color) {
        self.state.current_mut().stroke_style = PaintStyle::Color(color);
    }

    pub fn set_stroke_style(&mut self, style: PaintStyle) {
        self.state.current_mut().stroke_style = style;
    }

    pub fn set_line_width(&mut self, width: f32) {
        self.state.current_mut().line_width = width;
    }

    pub fn set_line_cap(&mut self, cap: LineCap) {
        self.state.current_mut().line_cap = cap;
    }

    pub fn set_line_join(&mut self, join: LineJoin) {
        self.state.current_mut().line_join = join;
    }

    pub fn set_miter_limit(&mut self, limit: f32) {
        self.state.current_mut().miter_limit = limit;
    }

    pub fn set_line_dash(&mut self, dash: Vec<f32>) {
        self.state.current_mut().line_dash = dash;
    }

    pub fn set_line_dash_offset(&mut self, offset: f32) {
        self.state.current_mut().line_dash_offset = offset;
    }

    pub fn set_global_alpha(&mut self, alpha: f32) {
        self.state.current_mut().global_alpha = alpha.clamp(0.0, 1.0);
    }

    pub fn set_global_composite_operation(&mut self, op: BlendMode) {
        self.state.current_mut().global_composite_operation = op;
    }

    pub fn set_shadow_offset_x(&mut self, x: f32) {
        self.state.current_mut().shadow.offset_x = x;
    }

    pub fn set_shadow_offset_y(&mut self, y: f32) {
        self.state.current_mut().shadow.offset_y = y;
    }

    pub fn set_shadow_blur(&mut self, blur: f32) {
        self.state.current_mut().shadow.blur = blur.max(0.0);
    }

    pub fn set_shadow_color(&mut self, color: Color) {
        self.state.current_mut().shadow.color = color;
    }

    pub fn set_image_smoothing_enabled(&mut self, enabled: bool) {
        self.state.current_mut().image_smoothing_enabled = enabled;
    }

    pub fn set_font(&mut self, font: String) {
        self.state.current_mut().font = font;
    }

    pub fn set_text_align(&mut self, align: TextAlign) {
        self.state.current_mut().text_align = align;
    }

    pub fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.state.current_mut().text_baseline = baseline;
    }

    pub fn set_direction(&mut self, direction: TextDirection) {
        self.state.current_mut().direction = direction;
    }

    // ── Style properties (getters) ───────────────────────────────────

    #[must_use]
    pub fn fill_style(&self) -> &PaintStyle { &self.state.current().fill_style }
    #[must_use]
    pub fn stroke_style(&self) -> &PaintStyle { &self.state.current().stroke_style }
    #[must_use]
    pub fn line_width(&self) -> f32 { self.state.current().line_width }
    #[must_use]
    pub fn line_cap(&self) -> LineCap { self.state.current().line_cap }
    #[must_use]
    pub fn line_join(&self) -> LineJoin { self.state.current().line_join }
    #[must_use]
    pub fn miter_limit(&self) -> f32 { self.state.current().miter_limit }
    #[must_use]
    pub fn line_dash(&self) -> &[f32] { &self.state.current().line_dash }
    #[must_use]
    pub fn line_dash_offset(&self) -> f32 { self.state.current().line_dash_offset }
    #[must_use]
    pub fn global_alpha(&self) -> f32 { self.state.current().global_alpha }
    #[must_use]
    pub fn global_composite_operation(&self) -> BlendMode { self.state.current().global_composite_operation }
    #[must_use]
    pub fn shadow_offset_x(&self) -> f32 { self.state.current().shadow.offset_x }
    #[must_use]
    pub fn shadow_offset_y(&self) -> f32 { self.state.current().shadow.offset_y }
    #[must_use]
    pub fn shadow_blur(&self) -> f32 { self.state.current().shadow.blur }
    #[must_use]
    pub fn shadow_color(&self) -> Color { self.state.current().shadow.color }
    #[must_use]
    pub fn image_smoothing_enabled(&self) -> bool { self.state.current().image_smoothing_enabled }
    #[must_use]
    pub fn font(&self) -> &str { &self.state.current().font }
    #[must_use]
    pub fn text_align(&self) -> TextAlign { self.state.current().text_align }
    #[must_use]
    pub fn text_baseline(&self) -> TextBaseline { self.state.current().text_baseline }

    // ── Gradient/Pattern factories ───────────────────────────────────

    #[must_use]
    pub fn create_linear_gradient(
        x0: f32, y0: f32, x1: f32, y1: f32,
    ) -> LinearGradient {
        LinearGradient {
            start: kozan_primitives::geometry::Point { x: x0, y: y0 },
            end: kozan_primitives::geometry::Point { x: x1, y: y1 },
            stops: Vec::new(),
        }
    }

    #[must_use]
    pub fn create_radial_gradient(
        x0: f32, y0: f32, r0: f32,
        x1: f32, y1: f32, r1: f32,
    ) -> RadialGradient {
        RadialGradient {
            start_center: kozan_primitives::geometry::Point { x: x0, y: y0 },
            start_radius: r0,
            end_center: kozan_primitives::geometry::Point { x: x1, y: y1 },
            end_radius: r1,
            stops: Vec::new(),
        }
    }

    #[must_use]
    pub fn create_conic_gradient(
        start_angle: f32, cx: f32, cy: f32,
    ) -> ConicGradient {
        ConicGradient {
            center: kozan_primitives::geometry::Point { x: cx, y: cy },
            start_angle,
            stops: Vec::new(),
        }
    }

    #[must_use]
    pub fn create_pattern(image: ImageData, repetition: crate::style::PatternRepetition) -> Pattern {
        Pattern {
            image: crate::style::PatternImage::ImageData(image),
            repetition,
        }
    }

    // ── Rectangle drawing ────────────────────────────────────────────

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if w == 0.0 || h == 0.0 {
            return;
        }
        let rect = Rect::new(x, y, w, h);
        let paint = self.resolve_fill_paint();
        self.recording.push(CanvasOp::FillRect { rect, paint });
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if w == 0.0 && h == 0.0 {
            return;
        }
        let rect = Rect::new(x, y, w, h);
        let stroke = self.resolve_stroke();
        self.recording.push(CanvasOp::StrokeRect { rect, stroke });
    }

    pub fn clear_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if w == 0.0 || h == 0.0 {
            return;
        }
        let rect = Rect::new(x, y, w, h);
        self.recording.push(CanvasOp::ClearRect { rect });
    }

    // ── Path building ────────────────────────────────────────────────

    pub fn begin_path(&mut self) {
        self.path.clear();
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(x, y);
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(x, y);
    }

    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.path.quadratic_curve_to(cpx, cpy, x, y);
    }

    pub fn bezier_curve_to(
        &mut self,
        cp1x: f32, cp1y: f32,
        cp2x: f32, cp2y: f32,
        x: f32, y: f32,
    ) {
        self.path.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y);
    }

    pub fn arc(
        &mut self, x: f32, y: f32,
        radius: f32,
        start_angle: f32, end_angle: f32,
        ccw: bool,
    ) {
        self.path.arc(x, y, radius, start_angle, end_angle, ccw);
    }

    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) {
        self.path.arc_to(x1, y1, x2, y2, radius);
    }

    pub fn ellipse(
        &mut self,
        x: f32, y: f32,
        radius_x: f32, radius_y: f32,
        rotation: f32,
        start_angle: f32, end_angle: f32,
        ccw: bool,
    ) {
        self.path.ellipse(x, y, radius_x, radius_y, rotation, start_angle, end_angle, ccw);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.path.rect(x, y, w, h);
    }

    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radii: [f32; 4]) {
        self.path.round_rect(x, y, w, h, radii);
    }

    pub fn close_path(&mut self) {
        self.path.close_path();
    }

    // ── Path drawing ─────────────────────────────────────────────────

    pub fn fill(&mut self) {
        self.fill_with_rule(FillRule::NonZero);
    }

    pub fn fill_with_rule(&mut self, fill_rule: FillRule) {
        if self.path.is_empty() {
            return;
        }
        let ops = self.path.ops().to_vec();
        let paint = self.resolve_fill_paint();
        self.recording.push(CanvasOp::FillPath { ops, fill_rule, paint });
    }

    pub fn stroke(&mut self) {
        if self.path.is_empty() {
            return;
        }
        let ops = self.path.ops().to_vec();
        let stroke = self.resolve_stroke();
        self.recording.push(CanvasOp::StrokePath { ops, stroke });
    }

    pub fn clip(&mut self) {
        self.clip_with_rule(FillRule::NonZero);
    }

    pub fn clip_with_rule(&mut self, fill_rule: FillRule) {
        if self.path.is_empty() {
            return;
        }
        let ops = self.path.ops().to_vec();
        self.recording.push(CanvasOp::ClipPath { ops, fill_rule });
    }

    // ── Text ─────────────────────────────────────────────────────────

    pub fn fill_text(&mut self, text: &str, x: f32, y: f32) {
        self.fill_text_max_width(text, x, y, None);
    }

    pub fn fill_text_max_width(&mut self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        if text.is_empty() {
            return;
        }
        let font = self.state.current().font.clone();
        let paint = self.resolve_fill_paint();
        self.recording.push(CanvasOp::FillText {
            text: text.to_string(), x, y, max_width, font, paint,
        });
    }

    pub fn stroke_text(&mut self, text: &str, x: f32, y: f32) {
        self.stroke_text_max_width(text, x, y, None);
    }

    pub fn stroke_text_max_width(&mut self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        if text.is_empty() {
            return;
        }
        let font = self.state.current().font.clone();
        let stroke = self.resolve_stroke();
        self.recording.push(CanvasOp::StrokeText {
            text: text.to_string(), x, y, max_width, font, stroke,
        });
    }

    // ── Image drawing ────────────────────────────────────────────────

    pub fn draw_image(
        &mut self,
        data: Arc<ImageData>,
        src_rect: Rect,
        dst_rect: Rect,
    ) {
        let paint = self.resolve_fill_paint();
        self.recording.push(CanvasOp::DrawImage { data, src_rect, dst_rect, paint });
    }

    // ── Pixel manipulation ───────────────────────────────────────────

    pub fn put_image_data(&mut self, data: ImageData, dx: f32, dy: f32) {
        self.recording.push(CanvasOp::PutImageData { data, dx, dy, dirty: None });
    }

    pub fn put_image_data_dirty(
        &mut self,
        data: ImageData,
        dx: f32, dy: f32,
        dirty_x: f32, dirty_y: f32, dirty_w: f32, dirty_h: f32,
    ) {
        self.recording.push(CanvasOp::PutImageData {
            data, dx, dy,
            dirty: Some(Rect::new(dirty_x, dirty_y, dirty_w, dirty_h)),
        });
    }

    // ── Internal: resolve current state into baked paint ──────────────

    fn resolve_fill_paint(&self) -> ResolvedPaint {
        let s = self.state.current();
        ResolvedPaint {
            style: s.fill_style.clone(),
            global_alpha: s.global_alpha,
            composite: s.global_composite_operation,
            shadow: s.shadow,
            image_smoothing: s.image_smoothing_enabled,
        }
    }

    fn resolve_stroke(&self) -> ResolvedStroke {
        let s = self.state.current();
        ResolvedStroke {
            paint: ResolvedPaint {
                style: s.stroke_style.clone(),
                global_alpha: s.global_alpha,
                composite: s.global_composite_operation,
                shadow: s.shadow,
                image_smoothing: s.image_smoothing_enabled,
            },
            line_width: s.line_width,
            line_cap: s.line_cap,
            line_join: s.line_join,
            miter_limit: s.miter_limit,
            line_dash: s.line_dash.clone(),
            line_dash_offset: s.line_dash_offset,
        }
    }
}

impl Default for CanvasRenderingContext2D {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: add a color stop to a gradient.
impl LinearGradient {
    pub fn add_color_stop(&mut self, offset: f32, color: Color) {
        self.stops.push(GradientStop { offset, color });
    }
}

impl RadialGradient {
    pub fn add_color_stop(&mut self, offset: f32, color: Color) {
        self.stops.push(GradientStop { offset, color });
    }
}

impl ConicGradient {
    pub fn add_color_stop(&mut self, offset: f32, color: Color) {
        self.stops.push(GradientStop { offset, color });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_rect_records_op() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.fill_rect(10.0, 20.0, 100.0, 50.0);
        let rec = ctx.take_recording();
        assert_eq!(rec.len(), 1);
        assert!(matches!(rec.ops()[0], CanvasOp::FillRect { .. }));
    }

    #[test]
    fn fill_rect_zero_size_skipped() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.fill_rect(0.0, 0.0, 0.0, 50.0);
        ctx.fill_rect(0.0, 0.0, 50.0, 0.0);
        let rec = ctx.take_recording();
        assert!(rec.is_empty());
    }

    #[test]
    fn style_baked_into_draw_op() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.set_fill_color(Color::RED);
        ctx.fill_rect(0.0, 0.0, 10.0, 10.0);

        let rec = ctx.take_recording();
        if let CanvasOp::FillRect { paint, .. } = &rec.ops()[0] {
            assert!(matches!(paint.style, PaintStyle::Color(c) if c == Color::RED));
        } else {
            panic!("expected FillRect");
        }
    }

    #[test]
    fn save_restore_bakes_correct_style() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.set_fill_color(Color::RED);
        ctx.save();
        ctx.set_fill_color(Color::BLUE);
        ctx.fill_rect(0.0, 0.0, 10.0, 10.0);
        ctx.restore();
        ctx.fill_rect(0.0, 0.0, 10.0, 10.0);

        let rec = ctx.take_recording();
        // Save, FillRect(blue), Restore, FillRect(red)
        assert_eq!(rec.len(), 4);

        if let CanvasOp::FillRect { paint, .. } = &rec.ops()[1] {
            assert!(matches!(paint.style, PaintStyle::Color(c) if c == Color::BLUE));
        } else {
            panic!("expected FillRect with blue");
        }

        if let CanvasOp::FillRect { paint, .. } = &rec.ops()[3] {
            assert!(matches!(paint.style, PaintStyle::Color(c) if c == Color::RED));
        } else {
            panic!("expected FillRect with red");
        }
    }

    #[test]
    fn transform_recorded_as_ops() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.translate(10.0, 20.0);
        ctx.rotate(1.5);
        ctx.scale(2.0, 3.0);

        let rec = ctx.take_recording();
        assert_eq!(rec.len(), 3);
        assert!(matches!(rec.ops()[0], CanvasOp::Translate { tx: 10.0, ty: 20.0 }));
        assert!(matches!(rec.ops()[1], CanvasOp::Rotate { angle } if angle == 1.5));
        assert!(matches!(rec.ops()[2], CanvasOp::Scale { sx: 2.0, sy: 3.0 }));
    }

    #[test]
    fn global_alpha_baked() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.set_global_alpha(0.5);
        ctx.fill_rect(0.0, 0.0, 10.0, 10.0);

        let rec = ctx.take_recording();
        if let CanvasOp::FillRect { paint, .. } = &rec.ops()[0] {
            assert_eq!(paint.global_alpha, 0.5);
        } else {
            panic!("expected FillRect");
        }
    }

    #[test]
    fn global_alpha_clamped() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.set_global_alpha(2.0);
        assert_eq!(ctx.global_alpha(), 1.0);
        ctx.set_global_alpha(-1.0);
        assert_eq!(ctx.global_alpha(), 0.0);
    }

    #[test]
    fn line_dash_in_stroke() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.set_line_dash(vec![5.0, 3.0]);
        ctx.set_line_dash_offset(2.0);
        ctx.move_to(0.0, 0.0);
        ctx.line_to(100.0, 100.0);
        ctx.stroke();

        let rec = ctx.take_recording();
        if let CanvasOp::StrokePath { stroke, .. } = &rec.ops()[0] {
            assert_eq!(stroke.line_dash, vec![5.0, 3.0]);
            assert_eq!(stroke.line_dash_offset, 2.0);
        } else {
            panic!("expected StrokePath");
        }
    }

    #[test]
    fn clip_records_path() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.move_to(0.0, 0.0);
        ctx.line_to(100.0, 0.0);
        ctx.line_to(50.0, 100.0);
        ctx.close_path();
        ctx.clip();

        let rec = ctx.take_recording();
        assert_eq!(rec.len(), 1);
        if let CanvasOp::ClipPath { ops, fill_rule } = &rec.ops()[0] {
            assert_eq!(ops.len(), 4);
            assert_eq!(*fill_rule, FillRule::NonZero);
        } else {
            panic!("expected ClipPath");
        }
    }

    #[test]
    fn begin_path_clears() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.move_to(0.0, 0.0);
        ctx.line_to(10.0, 10.0);
        ctx.begin_path();
        ctx.fill();

        let rec = ctx.take_recording();
        assert!(rec.is_empty());
    }

    #[test]
    fn reset_clears_everything() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.set_fill_color(Color::RED);
        ctx.translate(10.0, 20.0);
        ctx.fill_rect(0.0, 0.0, 10.0, 10.0);
        ctx.reset();

        assert_eq!(ctx.global_alpha(), 1.0);
        assert!(matches!(ctx.fill_style(), PaintStyle::Color(c) if *c == Color::BLACK));
        assert_eq!(ctx.get_transform(), AffineTransform::IDENTITY);

        let rec = ctx.take_recording();
        assert!(rec.is_empty());
    }

    #[test]
    fn set_transform_replaces() {
        let mut ctx = CanvasRenderingContext2D::new();
        ctx.translate(100.0, 200.0);
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 50.0, 50.0);

        let t = ctx.get_transform();
        let expected = AffineTransform::new(1.0, 0.0, 0.0, 1.0, 50.0, 50.0);
        assert_eq!(t, expected);
    }
}
