//! Canvas recording replay — converts `CanvasRecording` into vello draw calls.
//!
//! Chrome equivalent: `PaintRecord::Playback(SkCanvas*)`.
//! This is the ONLY place where canvas ops touch vello types.

use std::f32::consts::PI;

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Cap, Join, Rect, RoundedRect, Shape, Stroke};
use vello::peniko::{Brush, Color as VelloColor, Fill};

use kozan_canvas::line::{LineCap, LineJoin};
use kozan_canvas::op::{CanvasOp, ResolvedStroke};
use kozan_canvas::path::PathOp;
use kozan_canvas::recording::CanvasRecording;
use kozan_canvas::style::{FillRule, PaintStyle};

/// Replays a `CanvasRecording` into a vello `Scene`.
///
/// Chrome equivalent: `PaintRecord::Playback(SkCanvas*)`.
pub struct VelloCanvasPlayer;

impl VelloCanvasPlayer {
    /// Replay a canvas recording into the scene at the given transform.
    ///
    /// `base_transform`: positions the canvas in display list coordinates.
    /// `canvas_width`/`canvas_height`: canvas bitmap dimensions — used to
    /// clip the canvas content to its element bounds.
    pub fn play(
        scene: &mut Scene,
        recording: &CanvasRecording,
        base_transform: Affine,
        canvas_width: f32,
        canvas_height: f32,
    ) {
        if recording.is_empty() {
            return;
        }

        let clip_rect = Rect::new(0.0, 0.0, canvas_width as f64, canvas_height as f64);
        scene.push_clip_layer(Fill::NonZero, base_transform, &clip_rect);

        // Each save level tracks transform + how many clip layers were pushed,
        // so Restore can pop them (matching browser's save/restore clip semantics).
        struct SaveLevel {
            transform: Affine,
            clip_depth: usize,
        }

        let mut state_stack: Vec<SaveLevel> = vec![SaveLevel {
            transform: base_transform,
            clip_depth: 0,
        }];

        for op in recording.ops() {
            let current = state_stack.last().expect("state stack never empty").transform;

            match op {
                CanvasOp::Save => {
                    state_stack.push(SaveLevel {
                        transform: current,
                        clip_depth: 0,
                    });
                }
                CanvasOp::Restore => {
                    if state_stack.len() > 1 {
                        let level = state_stack.pop().unwrap();
                        // Pop all clip layers pushed during this save level.
                        for _ in 0..level.clip_depth {
                            scene.pop_layer();
                        }
                    }
                }

                CanvasOp::Translate { tx, ty } => {
                    let top = &mut state_stack.last_mut().expect("state stack never empty").transform;
                    *top = *top * Affine::translate((*tx as f64, *ty as f64));
                }
                CanvasOp::Rotate { angle } => {
                    let top = &mut state_stack.last_mut().expect("state stack never empty").transform;
                    *top = *top * Affine::rotate(*angle as f64);
                }
                CanvasOp::Scale { sx, sy } => {
                    let top = &mut state_stack.last_mut().expect("state stack never empty").transform;
                    *top = *top * Affine::scale_non_uniform(*sx as f64, *sy as f64);
                }
                CanvasOp::SetTransform(t) => {
                    let m = affine_from_kozan(t);
                    state_stack.last_mut().expect("state stack never empty").transform =
                        base_transform * m;
                }
                CanvasOp::ResetTransform => {
                    state_stack.last_mut().expect("state stack never empty").transform =
                        base_transform;
                }

                CanvasOp::ClipRect { rect, fill_rule } => {
                    let r = to_kurbo_rect(rect);
                    scene.push_clip_layer(to_vello_fill(*fill_rule), current, &r);
                    state_stack.last_mut().expect("state stack never empty").clip_depth += 1;
                }
                CanvasOp::ClipPath { ops, fill_rule } => {
                    let path = build_kurbo_path(ops);
                    scene.push_clip_layer(to_vello_fill(*fill_rule), current, &path);
                    state_stack.last_mut().expect("state stack never empty").clip_depth += 1;
                }

                CanvasOp::FillRect { rect, paint } => {
                    let r = to_kurbo_rect(rect);
                    let brush = to_brush(&paint.style, paint.global_alpha);
                    scene.fill(
                        Fill::NonZero,
                        current,
                        &brush,
                        None,
                        &r,
                    );
                }

                CanvasOp::StrokeRect { rect, stroke } => {
                    let r = to_kurbo_rect(rect);
                    let vello_stroke = to_vello_stroke(stroke);
                    let brush = to_brush(&stroke.paint.style, stroke.paint.global_alpha);
                    scene.stroke(
                        &vello_stroke,
                        current,
                        &brush,
                        None,
                        &r,
                    );
                }

                CanvasOp::ClearRect { rect } => {
                    let r = to_kurbo_rect(rect);
                    scene.fill(
                        Fill::NonZero,
                        current,
                        VelloColor::new([0.0, 0.0, 0.0, 0.0]),
                        None,
                        &r,
                    );
                }

                CanvasOp::FillPath { ops, fill_rule, paint } => {
                    let path = build_kurbo_path(ops);
                    let brush = to_brush(&paint.style, paint.global_alpha);
                    scene.fill(
                        to_vello_fill(*fill_rule),
                        current,
                        &brush,
                        None,
                        &path,
                    );
                }

                CanvasOp::StrokePath { ops, stroke } => {
                    let path = build_kurbo_path(ops);
                    let vello_stroke = to_vello_stroke(stroke);
                    let brush = to_brush(&stroke.paint.style, stroke.paint.global_alpha);
                    scene.stroke(
                        &vello_stroke,
                        current,
                        &brush,
                        None,
                        &path,
                    );
                }

                CanvasOp::FillText { text, x, y, paint, .. } => {
                    // TODO: font resolution + glyph shaping for canvas text.
                    // Placeholder: skip text rendering until font system integration.
                    let _ = (text, x, y, paint);
                }

                CanvasOp::StrokeText { text, x, y, stroke, .. } => {
                    let _ = (text, x, y, stroke);
                }

                CanvasOp::DrawImage { .. } => {
                    // TODO: image texture upload + draw.
                }

                CanvasOp::PutImageData { .. } => {
                    // TODO: pixel buffer upload.
                }
            }
        }

        // Pop any remaining clip layers from unbalanced save/restore,
        // then pop the outer canvas clip.
        for level in state_stack.drain(..).rev() {
            for _ in 0..level.clip_depth {
                scene.pop_layer();
            }
        }
        scene.pop_layer();
    }
}

fn affine_from_kozan(t: &kozan_primitives::transform::AffineTransform) -> Affine {
    // AffineTransform stores | a c tx |
    //                        | b d ty |
    // Kurbo Affine expects [a, b, c, d, tx, ty] — same layout
    let [a, b, c, d, tx, ty] = t.to_cols_array();
    Affine::new([a as f64, b as f64, c as f64, d as f64, tx as f64, ty as f64])
}

fn to_kurbo_rect(r: &kozan_primitives::geometry::Rect) -> Rect {
    Rect::new(
        r.x() as f64,
        r.y() as f64,
        (r.x() + r.width()) as f64,
        (r.y() + r.height()) as f64,
    )
}

fn to_vello_fill(rule: FillRule) -> Fill {
    match rule {
        FillRule::NonZero => Fill::NonZero,
        FillRule::EvenOdd => Fill::EvenOdd,
    }
}

fn to_brush(style: &PaintStyle, alpha: f32) -> Brush {
    match style {
        PaintStyle::Color(c) => Brush::Solid(VelloColor::new([c.r, c.g, c.b, c.a * alpha])),
        PaintStyle::LinearGradient(g) => {
            use vello::peniko::{ColorStop, Gradient};
            let stops: Vec<ColorStop> = g
                .stops
                .iter()
                .map(|s| ColorStop {
                    offset: s.offset,
                    color: VelloColor::new([s.color.r, s.color.g, s.color.b, s.color.a]).into(),
                })
                .collect();
            Brush::Gradient(Gradient::new_linear(
                (g.start.x as f64, g.start.y as f64),
                (g.end.x as f64, g.end.y as f64),
            ).with_stops(stops.as_slice()))
        }
        PaintStyle::RadialGradient(g) => {
            use vello::peniko::{ColorStop, Gradient};
            let stops: Vec<ColorStop> = g
                .stops
                .iter()
                .map(|s| ColorStop {
                    offset: s.offset,
                    color: VelloColor::new([s.color.r, s.color.g, s.color.b, s.color.a]).into(),
                })
                .collect();
            Brush::Gradient(
                Gradient::new_two_point_radial(
                    (g.start_center.x as f64, g.start_center.y as f64),
                    g.start_radius,
                    (g.end_center.x as f64, g.end_center.y as f64),
                    g.end_radius,
                )
                .with_stops(stops.as_slice()),
            )
        }
        PaintStyle::ConicGradient(g) => {
            use vello::peniko::{ColorStop, Gradient};
            let stops: Vec<ColorStop> = g
                .stops
                .iter()
                .map(|s| ColorStop {
                    offset: s.offset,
                    color: VelloColor::new([s.color.r, s.color.g, s.color.b, s.color.a]).into(),
                })
                .collect();
            Brush::Gradient(
                Gradient::new_sweep(
                    (g.center.x as f64, g.center.y as f64),
                    g.start_angle,
                    g.start_angle + 2.0 * PI,
                )
                .with_stops(stops.as_slice()),
            )
        }
        PaintStyle::Pattern(_) => {
            // TODO: pattern image rendering
            Brush::Solid(VelloColor::new([0.0, 0.0, 0.0, 1.0]))
        }
    }
}

fn to_vello_stroke(s: &ResolvedStroke) -> Stroke {
    let mut stroke = Stroke::new(s.line_width as f64);
    stroke.start_cap = match s.line_cap {
        LineCap::Butt => Cap::Butt,
        LineCap::Round => Cap::Round,
        LineCap::Square => Cap::Square,
    };
    stroke.end_cap = stroke.start_cap;
    stroke.join = match s.line_join {
        LineJoin::Miter => Join::Miter,
        LineJoin::Round => Join::Round,
        LineJoin::Bevel => Join::Bevel,
    };
    stroke.miter_limit = s.miter_limit as f64;
    if !s.line_dash.is_empty() {
        stroke.dash_pattern = vello::kurbo::Dashes::from_vec(
            s.line_dash.iter().map(|&v| v as f64).collect(),
        );
        stroke.dash_offset = s.line_dash_offset as f64;
    }
    stroke
}

fn build_kurbo_path(ops: &[PathOp]) -> BezPath {
    let mut path = BezPath::new();
    for op in ops {
        match *op {
            PathOp::MoveTo { x, y } => path.move_to((x as f64, y as f64)),
            PathOp::LineTo { x, y } => path.line_to((x as f64, y as f64)),
            PathOp::QuadTo { cpx, cpy, x, y } => {
                path.quad_to((cpx as f64, cpy as f64), (x as f64, y as f64));
            }
            PathOp::CubicTo { cp1x, cp1y, cp2x, cp2y, x, y } => {
                path.curve_to(
                    (cp1x as f64, cp1y as f64),
                    (cp2x as f64, cp2y as f64),
                    (x as f64, y as f64),
                );
            }
            PathOp::Arc { x, y, radius, start_angle, end_angle, ccw } => {
                append_arc(&mut path, x, y, radius, radius, 0.0, start_angle, end_angle, ccw);
            }
            PathOp::ArcTo { x1, y1, x2, y2, radius } => {
                // Approximate arcTo with a line to the tangent point.
                // Full arcTo requires computing tangent intersections.
                // TODO: proper arcTo implementation with tangent calculation
                path.line_to((x1 as f64, y1 as f64));
                path.line_to((x2 as f64, y2 as f64));
                let _ = radius;
            }
            PathOp::Ellipse { x, y, radius_x, radius_y, rotation, start_angle, end_angle, ccw } => {
                append_arc(&mut path, x, y, radius_x, radius_y, rotation, start_angle, end_angle, ccw);
            }
            PathOp::Rect { x, y, w, h } => {
                path.move_to((x as f64, y as f64));
                path.line_to(((x + w) as f64, y as f64));
                path.line_to(((x + w) as f64, (y + h) as f64));
                path.line_to((x as f64, (y + h) as f64));
                path.close_path();
            }
            PathOp::RoundRect { x, y, w, h, radii } => {
                let r = RoundedRect::from_rect(
                    Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64),
                    (radii[0] as f64, radii[1] as f64, radii[2] as f64, radii[3] as f64),
                );
                path.extend(r.path_elements(0.1));
            }
            PathOp::Close => path.close_path(),
        }
    }
    path
}

/// Approximate an arc/ellipse with cubic Bezier segments.
fn append_arc(
    path: &mut BezPath,
    cx: f32, cy: f32,
    rx: f32, ry: f32,
    rotation: f32,
    start_angle: f32, end_angle: f32,
    ccw: bool,
) {
    let (start, end) = kozan_canvas::path::normalize_arc_angles(start_angle, end_angle, ccw);
    let sweep = end - start;
    if sweep.abs() < 1e-6 {
        return;
    }

    let n_segs = ((sweep.abs() / (PI / 2.0)).ceil() as usize).max(1);
    let seg_sweep = sweep / n_segs as f32;

    let cos_rot = rotation.cos();
    let sin_rot = rotation.sin();

    let point = |angle: f32| -> (f64, f64) {
        let (px, py) = kozan_canvas::path::point_on_ellipse(cx, cy, rx, ry, rotation, angle);
        (px as f64, py as f64)
    };

    let start_pt = point(start);
    path.move_to(start_pt);

    for i in 0..n_segs {
        let a0 = start + seg_sweep * i as f32;
        let a1 = a0 + seg_sweep;
        let alpha = (4.0 / 3.0) * (seg_sweep / 4.0).tan();

        let p0 = point(a0);
        let p3 = point(a1);

        let d0x = -rx * a0.sin();
        let d0y = ry * a0.cos();
        let d1x = -rx * a1.sin();
        let d1y = ry * a1.cos();

        let cp1 = (
            p0.0 + alpha as f64 * (d0x * cos_rot - d0y * sin_rot) as f64,
            p0.1 + alpha as f64 * (d0x * sin_rot + d0y * cos_rot) as f64,
        );
        let cp2 = (
            p3.0 - alpha as f64 * (d1x * cos_rot - d1y * sin_rot) as f64,
            p3.1 - alpha as f64 * (d1x * sin_rot + d1y * cos_rot) as f64,
        );

        path.curve_to(cp1, cp2, p3);
    }
}
