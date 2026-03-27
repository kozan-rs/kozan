//! Canvas operation types — the recorded drawing commands.
//!
//! Chrome equivalent: `cc::PaintOp` variants in `cc/paint/paint_op.h`.

use std::sync::Arc;

use kozan_primitives::geometry::Rect;
use kozan_primitives::transform::AffineTransform;

use crate::blend::BlendMode;
use crate::image::ImageData;
use crate::line::{LineCap, LineJoin};
use crate::path::PathOp;
use crate::shadow::ShadowState;
use crate::style::{FillRule, PaintStyle};

/// Resolved paint properties baked into each fill draw op at record time.
///
/// Chrome equivalent: `PaintFlags` configured from `CanvasRenderingContext2DState`
/// for fill operations. Style properties are NOT recorded as separate ops —
/// they are snapshot into this struct when the draw call happens.
#[derive(Clone, Debug)]
pub struct ResolvedPaint {
    pub style: PaintStyle,
    pub global_alpha: f32,
    pub composite: BlendMode,
    pub shadow: ShadowState,
    pub image_smoothing: bool,
}

impl Default for ResolvedPaint {
    fn default() -> Self {
        Self {
            style: PaintStyle::default(),
            global_alpha: 1.0,
            composite: BlendMode::default(),
            shadow: ShadowState::default(),
            image_smoothing: true,
        }
    }
}

/// Resolved stroke properties baked into each stroke draw op.
///
/// Chrome equivalent: `PaintFlags` with stroke width/cap/join/miter
/// extracted from `CanvasRenderingContext2DState`.
#[derive(Clone, Debug)]
pub struct ResolvedStroke {
    pub paint: ResolvedPaint,
    pub line_width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub line_dash: Vec<f32>,
    pub line_dash_offset: f32,
}

impl Default for ResolvedStroke {
    fn default() -> Self {
        Self {
            paint: ResolvedPaint::default(),
            line_width: 1.0,
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            miter_limit: 10.0,
            line_dash: Vec::new(),
            line_dash_offset: 0.0,
        }
    }
}

/// A single recorded canvas operation.
///
/// Chrome equivalent: `PaintOp` variants in `cc/paint/paint_op.h`.
///
/// Style properties (fillStyle, lineWidth, globalAlpha, shadow, etc.) are
/// baked into each draw op as `ResolvedPaint`/`ResolvedStroke` at record time.
/// Only transform and clip state changes are recorded as separate ops.
/// This matches Chrome's two-level state model exactly.
#[derive(Clone, Debug)]
pub enum CanvasOp {
    // ── State stack (transform + clip) ───────────────────────────────
    Save,
    Restore,

    // ── Transform ops ────────────────────────────────────────────────
    Translate { tx: f32, ty: f32 },
    Rotate { angle: f32 },
    Scale { sx: f32, sy: f32 },
    SetTransform(AffineTransform),
    ResetTransform,

    // ── Clip ops ─────────────────────────────────────────────────────
    ClipRect { rect: Rect, fill_rule: FillRule },
    ClipPath { ops: Vec<PathOp>, fill_rule: FillRule },

    // ── Rectangle draw ops ───────────────────────────────────────────
    FillRect { rect: Rect, paint: ResolvedPaint },
    StrokeRect { rect: Rect, stroke: ResolvedStroke },
    ClearRect { rect: Rect },

    // ── Path draw ops ────────────────────────────────────────────────
    FillPath { ops: Vec<PathOp>, fill_rule: FillRule, paint: ResolvedPaint },
    StrokePath { ops: Vec<PathOp>, stroke: ResolvedStroke },

    // ── Text draw ops ────────────────────────────────────────────────
    FillText {
        text: String,
        x: f32,
        y: f32,
        max_width: Option<f32>,
        font: String,
        paint: ResolvedPaint,
    },
    StrokeText {
        text: String,
        x: f32,
        y: f32,
        max_width: Option<f32>,
        font: String,
        stroke: ResolvedStroke,
    },

    // ── Image draw ops ───────────────────────────────────────────────
    DrawImage {
        data: Arc<ImageData>,
        src_rect: Rect,
        dst_rect: Rect,
        paint: ResolvedPaint,
    },

    // ── Pixel manipulation ───────────────────────────────────────────
    PutImageData {
        data: ImageData,
        dx: f32,
        dy: f32,
        dirty: Option<Rect>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_paint_default_matches_spec() {
        let paint = ResolvedPaint::default();
        assert_eq!(paint.global_alpha, 1.0);
        assert_eq!(paint.composite, BlendMode::SourceOver);
        assert!(!paint.shadow.is_visible());
        assert!(paint.image_smoothing);
    }

    #[test]
    fn resolved_stroke_default_matches_spec() {
        let stroke = ResolvedStroke::default();
        assert_eq!(stroke.line_width, 1.0);
        assert_eq!(stroke.line_cap, LineCap::Butt);
        assert_eq!(stroke.line_join, LineJoin::Miter);
        assert_eq!(stroke.miter_limit, 10.0);
        assert!(stroke.line_dash.is_empty());
    }
}
