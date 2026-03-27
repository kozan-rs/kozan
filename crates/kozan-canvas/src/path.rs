//! Path building for canvas 2D drawing operations.
//!
//! Chrome equivalent: `CanvasPath` mixin + `SkPath` verbs.

use std::f32::consts::PI;

/// Individual path segment — the atomic unit of a path recording.
///
/// Chrome equivalent: `SkPath` verbs (`kMove`, `kLine`, `kQuad`, `kCubic`, `kClose`)
/// plus the canvas-specific arc/ellipse operations that Chrome decomposes
/// into cubic Beziers before recording.
///
/// We keep the high-level ops (Arc, Ellipse, RoundRect) so renderers can
/// use native primitives when available, falling back to Bezier approximation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathOp {
    MoveTo { x: f32, y: f32 },
    LineTo { x: f32, y: f32 },
    QuadTo { cpx: f32, cpy: f32, x: f32, y: f32 },
    CubicTo { cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32 },
    Arc { x: f32, y: f32, radius: f32, start_angle: f32, end_angle: f32, ccw: bool },
    ArcTo { x1: f32, y1: f32, x2: f32, y2: f32, radius: f32 },
    Ellipse {
        x: f32, y: f32,
        radius_x: f32, radius_y: f32,
        rotation: f32,
        start_angle: f32, end_angle: f32,
        ccw: bool,
    },
    Rect { x: f32, y: f32, w: f32, h: f32 },
    RoundRect { x: f32, y: f32, w: f32, h: f32, radii: [f32; 4] },
    Close,
}

/// A path builder — accumulates path segments for fill/stroke/clip.
///
/// Chrome equivalent: the path state on `CanvasPath` mixin, backed by `SkPath`.
/// `beginPath()` clears it, drawing methods append ops, `fill()`/`stroke()`
/// consume a snapshot.
#[derive(Clone, Debug, Default)]
pub struct Path2D {
    ops: Vec<PathOp>,
}

impl Path2D {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.ops.clear();
    }

    #[must_use]
    pub fn ops(&self) -> &[PathOp] {
        &self.ops
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.ops.push(PathOp::MoveTo { x, y });
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.ops.push(PathOp::LineTo { x, y });
    }

    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.ops.push(PathOp::QuadTo { cpx, cpy, x, y });
    }

    pub fn bezier_curve_to(
        &mut self,
        cp1x: f32, cp1y: f32,
        cp2x: f32, cp2y: f32,
        x: f32, y: f32,
    ) {
        self.ops.push(PathOp::CubicTo { cp1x, cp1y, cp2x, cp2y, x, y });
    }

    /// Adds an arc to the path.
    ///
    /// Per HTML spec: if `start_angle == end_angle`, no arc is drawn.
    /// If the absolute angular distance is >= 2*PI, a full circle is drawn.
    pub fn arc(
        &mut self,
        x: f32, y: f32,
        radius: f32,
        start_angle: f32, end_angle: f32,
        ccw: bool,
    ) {
        if radius < 0.0 {
            return;
        }
        self.ops.push(PathOp::Arc { x, y, radius, start_angle, end_angle, ccw });
    }

    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) {
        if radius < 0.0 {
            return;
        }
        self.ops.push(PathOp::ArcTo { x1, y1, x2, y2, radius });
    }

    pub fn ellipse(
        &mut self,
        x: f32, y: f32,
        radius_x: f32, radius_y: f32,
        rotation: f32,
        start_angle: f32, end_angle: f32,
        ccw: bool,
    ) {
        if radius_x < 0.0 || radius_y < 0.0 {
            return;
        }
        self.ops.push(PathOp::Ellipse {
            x, y, radius_x, radius_y, rotation, start_angle, end_angle, ccw,
        });
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.ops.push(PathOp::Rect { x, y, w, h });
    }

    /// Adds a rounded rectangle with per-corner radii `[tl, tr, br, bl]`.
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radii: [f32; 4]) {
        self.ops.push(PathOp::RoundRect { x, y, w, h, radii });
    }

    pub fn close_path(&mut self) {
        self.ops.push(PathOp::Close);
    }

    /// Extend this path with all ops from another path.
    pub fn extend(&mut self, other: &Path2D) {
        self.ops.extend_from_slice(&other.ops);
    }
}

/// Compute a point on an ellipse at the given angle.
///
/// Used by renderers that decompose `Arc`/`Ellipse` path ops into
/// lines or Bezier curves.
#[must_use]
pub fn point_on_ellipse(cx: f32, cy: f32, rx: f32, ry: f32, rotation: f32, angle: f32) -> (f32, f32) {
    let cos_rot = rotation.cos();
    let sin_rot = rotation.sin();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let x = cx + rx * cos_a * cos_rot - ry * sin_a * sin_rot;
    let y = cy + rx * cos_a * sin_rot + ry * sin_a * cos_rot;
    (x, y)
}

/// Normalize an angular sweep for arc drawing.
///
/// Returns the effective start and end angles, clamped so the sweep
/// doesn't exceed a full circle.
#[must_use]
pub fn normalize_arc_angles(start: f32, end: f32, ccw: bool) -> (f32, f32) {
    let two_pi = 2.0 * PI;
    let mut sweep = end - start;

    if ccw {
        if sweep > 0.0 {
            sweep -= two_pi * ((sweep / two_pi).ceil());
        }
        if sweep == 0.0 && start != end {
            sweep = -two_pi;
        }
    } else {
        if sweep < 0.0 {
            sweep += two_pi * ((-sweep / two_pi).ceil());
        }
        if sweep == 0.0 && start != end {
            sweep = two_pi;
        }
    }

    (start, start + sweep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path() {
        let path = Path2D::new();
        assert!(path.is_empty());
        assert_eq!(path.ops().len(), 0);
    }

    #[test]
    fn basic_rect_path() {
        let mut path = Path2D::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 50.0);
        path.line_to(0.0, 50.0);
        path.close_path();
        assert_eq!(path.ops().len(), 5);
    }

    #[test]
    fn arc_negative_radius_ignored() {
        let mut path = Path2D::new();
        path.arc(0.0, 0.0, -5.0, 0.0, PI, false);
        assert!(path.is_empty());
    }

    #[test]
    fn clear_empties_path() {
        let mut path = Path2D::new();
        path.move_to(1.0, 2.0);
        path.line_to(3.0, 4.0);
        assert_eq!(path.ops().len(), 2);
        path.clear();
        assert!(path.is_empty());
    }

    #[test]
    fn extend_combines_paths() {
        let mut a = Path2D::new();
        a.move_to(0.0, 0.0);
        let mut b = Path2D::new();
        b.line_to(10.0, 10.0);
        a.extend(&b);
        assert_eq!(a.ops().len(), 2);
    }

    #[test]
    fn point_on_circle() {
        let (x, y) = point_on_ellipse(0.0, 0.0, 1.0, 1.0, 0.0, 0.0);
        assert!((x - 1.0).abs() < 1e-6);
        assert!(y.abs() < 1e-6);
    }

    #[test]
    fn normalize_full_circle_cw() {
        let (start, end) = normalize_arc_angles(0.0, 2.0 * PI, false);
        assert!((end - start - 2.0 * PI).abs() < 1e-6);
    }
}
