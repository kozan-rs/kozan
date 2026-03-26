//! `SceneBuilder` — converts a Kozan `DisplayList` into a `vello::Scene`.
//!
//! # Text rendering architecture (Chrome-level)
//!
//! ```text
//! Layout:   text + CSS font → Parley shapes (HarfRust) → ShapedTextRun
//! Paint:    reads ShapedTextRun → emits DrawCommand::Text { runs }
//! Renderer: draws pre-shaped glyphs → vello draw_glyphs() → GPU
//! ```
//!
//! The renderer has ZERO font logic. No font selection, no shaping.
//! `parley::FontData` = `peniko::Font` — same type, zero conversion.

use vello::NormalizedCoord;
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Line, Shape, Stroke};
use vello::peniko::{Fill, Mix};

use kozan_core::compositor::frame::FrameQuad;
use kozan_core::paint::DisplayList;
use kozan_core::paint::display_item::{BorderRadii as KBorderRadii, DisplayItem, DrawCommand};
use kozan_core::scroll::ScrollOffsets;
use kozan_primitives::geometry::Rect as KRect;

use crate::convert::{to_kurbo_rect, to_kurbo_rounded_rect, to_peniko_color};

/// Converts a `DisplayList` to a `vello::Scene`.
///
/// Called once per frame from `VelloSurface::render()`.
pub struct SceneBuilder;

impl SceneBuilder {
    /// Build a vello `Scene` from a `DisplayList`.
    ///
    /// `scale_factor`: maps logical pixels → physical pixels.
    /// `scroll_adjustments`: compositor overrides for scroll transforms.
    /// When a `PushTransform` has a `scroll_node`, the compositor's offset
    /// replaces the baked-in value — enabling vsync-rate scroll without repaint.
    /// `scale_factor`: content scale (device DPI × page zoom) for display list.
    /// `device_scale`: device DPI only — for compositor overlays (scrollbars)
    /// that must stay a fixed device-pixel size regardless of page zoom.
    pub fn build(
        list: &DisplayList,
        scale_factor: f64,
        device_scale: f64,
        scroll_offsets: &ScrollOffsets,
        quads: &[FrameQuad],
    ) -> Scene {
        let mut scene = Scene::new();
        let mut transforms: Vec<Affine> = vec![Affine::scale(scale_factor)];

        for item in list.iter() {
            let current_transform = *transforms.last().unwrap_or(&Affine::IDENTITY);

            match item {
                // ── Draw commands ────────────────────────────────────────────
                DisplayItem::Draw(cmd) => match cmd {
                    DrawCommand::Rect { rect, color } => {
                        scene.fill(
                            Fill::NonZero,
                            current_transform,
                            to_peniko_color(*color),
                            None,
                            &to_kurbo_rect(*rect),
                        );
                    }

                    DrawCommand::RoundedRect { rect, radii, color } => {
                        scene.fill(
                            Fill::NonZero,
                            current_transform,
                            to_peniko_color(*color),
                            None,
                            &to_kurbo_rounded_rect(*rect, *radii),
                        );
                    }

                    DrawCommand::Line {
                        x0,
                        y0,
                        x1,
                        y1,
                        width,
                        color,
                    } => {
                        let line = Line::new((*x0 as f64, *y0 as f64), (*x1 as f64, *y1 as f64));
                        scene.stroke(
                            &Stroke::new(*width as f64),
                            current_transform,
                            to_peniko_color(*color),
                            None,
                            &line,
                        );
                    }

                    // TODO(M7): border-style dashed/dotted/double/wavy — need dash path generation.
                    // Currently all borders render as solid fills regardless of style.
                    DrawCommand::Border {
                        rect,
                        widths,
                        colors,
                        styles: _,
                    } => {
                        let r = *rect;
                        let w = *widths;
                        let c = *colors;

                        if w.top > 0.0 {
                            scene.fill(
                                Fill::NonZero,
                                current_transform,
                                to_peniko_color(c.top),
                                None,
                                &to_kurbo_rect(KRect::new(r.x(), r.y(), r.width(), w.top)),
                            );
                        }
                        if w.bottom > 0.0 {
                            scene.fill(
                                Fill::NonZero,
                                current_transform,
                                to_peniko_color(c.bottom),
                                None,
                                &to_kurbo_rect(KRect::new(
                                    r.x(),
                                    r.bottom() - w.bottom,
                                    r.width(),
                                    w.bottom,
                                )),
                            );
                        }
                        if w.left > 0.0 {
                            scene.fill(
                                Fill::NonZero,
                                current_transform,
                                to_peniko_color(c.left),
                                None,
                                &to_kurbo_rect(KRect::new(r.x(), r.y(), w.left, r.height())),
                            );
                        }
                        if w.right > 0.0 {
                            scene.fill(
                                Fill::NonZero,
                                current_transform,
                                to_peniko_color(c.right),
                                None,
                                &to_kurbo_rect(KRect::new(
                                    r.right() - w.right,
                                    r.y(),
                                    w.right,
                                    r.height(),
                                )),
                            );
                        }
                    }

                    DrawCommand::Text { x, y, runs } => {
                        // TODO(M7): TextShadow — Chrome: TextPainter multi-pass with shadow offsets + blur.
                        // Draw pre-shaped glyph runs.
                        // Shaped during layout (Parley + HarfRust).
                        // parley::FontData = peniko::Font — zero conversion.
                        for run in runs {
                            let run_transform = current_transform
                                * Affine::translate((
                                    *x as f64 + run.offset as f64,
                                    *y as f64 + run.baseline as f64,
                                ));

                            let brush_color = vello::peniko::Color::new([
                                run.color[0] as f32 / 255.0,
                                run.color[1] as f32 / 255.0,
                                run.color[2] as f32 / 255.0,
                                run.color[3] as f32 / 255.0,
                            ]);

                            let glyphs = run.glyphs.iter().map(|g| vello::Glyph {
                                id: g.id,
                                x: g.x,
                                y: g.y,
                            });

                            // Variable font axis values — tells vello which instance
                            // to render (e.g., wght=700 for bold). Without these,
                            // vello renders the default instance (Regular/400).
                            let coords: &[NormalizedCoord] = &run.normalized_coords;

                            scene
                                .draw_glyphs(&run.font)
                                .font_size(run.font_size)
                                .hint(true)
                                .normalized_coords(coords)
                                .transform(run_transform)
                                .brush(brush_color)
                                .draw(Fill::NonZero, glyphs);
                        }
                    }

                    DrawCommand::RoundedBorderRing {
                        outer_rect,
                        outer_radii,
                        inner_rect,
                        inner_radii,
                        color,
                    } => {
                        // Chrome: DrawDRRectOp — compound path (outer CW + inner CCW)
                        // filled with EvenOdd produces only the ring area.
                        let outer = to_kurbo_rounded_rect(*outer_rect, *outer_radii);
                        let inner = to_kurbo_rounded_rect(*inner_rect, *inner_radii);

                        let mut path = BezPath::new();
                        // Outer path (default winding = CW)
                        path.extend(outer.path_elements(0.1));
                        // Inner path reversed (CCW) — creates the hole
                        let inner_elements: Vec<_> = inner.path_elements(0.1).collect();
                        // Reverse by collecting and re-emitting in reverse order
                        // with move_to at the end becoming the start
                        let mut inner_path = BezPath::new();
                        inner_path.extend(inner_elements);
                        // Use EvenOdd: overlapping area (interior) cancels out
                        path.extend(inner_path.elements().iter().copied());

                        scene.fill(
                            Fill::EvenOdd,
                            current_transform,
                            to_peniko_color(*color),
                            None,
                            &path,
                        );
                    }

                    DrawCommand::Outline {
                        rect,
                        radii,
                        width,
                        offset,
                        color,
                    } => {
                        let expand = width + offset;
                        let outer_rect = rect.outset(expand, expand, expand, expand);
                        let outer_radii = KBorderRadii {
                            top_left: radii.top_left + expand,
                            top_right: radii.top_right + expand,
                            bottom_right: radii.bottom_right + expand,
                            bottom_left: radii.bottom_left + expand,
                        };
                        let inner_rect = rect.outset(*offset, *offset, *offset, *offset);
                        let inner_radii = KBorderRadii {
                            top_left: radii.top_left + offset,
                            top_right: radii.top_right + offset,
                            bottom_right: radii.bottom_right + offset,
                            bottom_left: radii.bottom_left + offset,
                        };

                        let outer = to_kurbo_rounded_rect(outer_rect, outer_radii);
                        let inner = to_kurbo_rounded_rect(inner_rect, inner_radii);

                        let mut path = BezPath::new();
                        path.extend(outer.path_elements(0.1));
                        let mut inner_path = BezPath::new();
                        inner_path.extend(inner.path_elements(0.1));
                        path.extend(inner_path.elements().iter().copied());

                        scene.fill(
                            Fill::EvenOdd,
                            current_transform,
                            to_peniko_color(*color),
                            None,
                            &path,
                        );
                    }

                    // TODO(M7): LinearGradient DrawCommand — Chrome: PaintOp::DrawPaintOp + cc::PaintShader.
                    // TODO(M7): RadialGradient DrawCommand — Chrome: PaintOp::DrawPaintOp + cc::PaintShader.
                    DrawCommand::Image { .. } => {
                        // TODO(M5.1.5): image rendering.
                    }

                    DrawCommand::BoxShadow { .. } => {
                        // TODO(M6): blur compositing.
                    }
                },

                // ── Clip stack ───────────────────────────────────────────────
                DisplayItem::PushClip(data) => {
                    scene.push_clip_layer(
                        Fill::NonZero,
                        current_transform,
                        &to_kurbo_rect(data.rect),
                    );
                }
                DisplayItem::PopClip => {
                    scene.pop_layer();
                }

                DisplayItem::PushRoundedClip(data) => {
                    scene.push_clip_layer(
                        Fill::NonZero,
                        current_transform,
                        &to_kurbo_rounded_rect(data.rect, data.radii),
                    );
                }
                DisplayItem::PopRoundedClip => {
                    scene.pop_layer();
                }

                // ── Opacity stack ────────────────────────────────────────────
                DisplayItem::PushOpacity(alpha) => {
                    let bounds = vello::kurbo::Rect::new(-1e6, -1e6, 1e6, 1e6);
                    scene.push_layer(
                        Fill::NonZero,
                        Mix::Normal,
                        *alpha,
                        current_transform,
                        &bounds,
                    );
                }
                DisplayItem::PopOpacity => {
                    scene.pop_layer();
                }

                // ── Transform stack ──────────────────────────────────────────
                DisplayItem::PushTransform(data) => {
                    let (tx, ty) = match data.scroll_node {
                        Some(id) => {
                            let o = scroll_offsets.offset(id);
                            (-o.dx as f64, -o.dy as f64)
                        }
                        None => (data.translate_x as f64, data.translate_y as f64),
                    };
                    transforms.push(current_transform * Affine::translate((tx, ty)));
                }
                DisplayItem::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                }

                // ── External GPU surface ─────────────────────────────────────
                DisplayItem::ExternalSurface(_) => {
                    // TODO(M6): compositor integration for 3D/video.
                }
            }
        }

        use kozan_core::compositor::frame::QuadSpace;

        let content_transform = Affine::scale(scale_factor);
        let device_transform = Affine::scale(device_scale);

        for quad in quads {
            let color = vello::peniko::Color::new([
                quad.color.r, quad.color.g, quad.color.b, quad.color.a * quad.opacity,
            ]);

            // Content quads: rendered with content_scale (device DPI × page zoom).
            // Screen quads (scrollbar overlays): already converted to
            // screen-logical space by the layer — rendered with device_scale only.
            let (transform, r, clip_rect, radius) = match quad.space {
                QuadSpace::Content => (
                    content_transform,
                    quad.rect,
                    quad.clip,
                    quad.radius as f64,
                ),
                QuadSpace::Screen => (
                    device_transform,
                    quad.rect,
                    quad.clip,
                    quad.radius as f64,
                ),
            };

            if let Some(clip) = clip_rect {
                let cr = vello::kurbo::Rect::new(
                    clip.x() as f64, clip.y() as f64,
                    clip.right() as f64, clip.bottom() as f64,
                );
                scene.push_clip_layer(Fill::NonZero, transform, &cr);
            }

            let rrect = vello::kurbo::RoundedRect::new(
                r.x() as f64, r.y() as f64,
                r.right() as f64, r.bottom() as f64,
                radius,
            );
            scene.fill(Fill::NonZero, transform, color, None, &rrect);

            if clip_rect.is_some() {
                scene.pop_layer();
            }
        }

        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_core::paint::display_item::{ClipData, TransformData};
    use kozan_core::scroll::ScrollOffsets;
    use kozan_primitives::color::Color;
    use kozan_primitives::geometry::Rect;

    fn empty_display_list() -> DisplayList {
        DisplayList::builder().finish()
    }

    fn single_rect_display_list() -> DisplayList {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(10.0, 20.0, 100.0, 50.0),
            color: Color::RED,
        }));
        builder.finish()
    }

    #[test]
    fn empty_list_produces_empty_scene() {
        let list = empty_display_list();
        let scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
        // An empty display list should produce a scene with no encoding errors.
        // Scene::new() is the baseline — we just verify it doesn't panic.
        assert!(scene.encoding().is_empty());
    }

    #[test]
    fn single_rect_produces_nonempty_scene() {
        let list = single_rect_display_list();
        let scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn scale_factor_does_not_panic() {
        let list = single_rect_display_list();
        // High DPI scale factor
        let scene = SceneBuilder::build(&list, 2.0, 2.0, &ScrollOffsets::new(), &[]);
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn fractional_scale_factor() {
        let list = single_rect_display_list();
        let scene = SceneBuilder::build(&list, 1.5, 1.5, &ScrollOffsets::new(), &[]);
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn clip_push_pop_does_not_panic() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::PushClip(ClipData {
            rect: Rect::new(0.0, 0.0, 200.0, 200.0),
        }));
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(10.0, 10.0, 50.0, 50.0),
            color: Color::BLUE,
        }));
        builder.push(DisplayItem::PopClip);
        let list = builder.finish();

        let scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn opacity_push_pop_does_not_panic() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::PushOpacity(0.5));
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            color: Color::GREEN,
        }));
        builder.push(DisplayItem::PopOpacity);
        let list = builder.finish();

        let scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn transform_push_pop_does_not_panic() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::PushTransform(TransformData {
            translate_x: 50.0,
            translate_y: 100.0,
            scroll_node: None,
        }));
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 30.0, 30.0),
            color: Color::WHITE,
        }));
        builder.push(DisplayItem::PopTransform);
        let list = builder.finish();

        let scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn rounded_rect_does_not_panic() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::Draw(DrawCommand::RoundedRect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            radii: KBorderRadii {
                top_left: 10.0,
                top_right: 10.0,
                bottom_right: 10.0,
                bottom_left: 10.0,
            },
            color: Color::RED,
        }));
        let list = builder.finish();

        let scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn line_does_not_panic() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::Draw(DrawCommand::Line {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 100.0,
            width: 2.0,
            color: Color::BLACK,
        }));
        let list = builder.finish();

        let scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
        assert!(!scene.encoding().is_empty());
    }

    #[test]
    fn pop_transform_below_root_is_harmless() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::PopTransform);
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            color: Color::RED,
        }));
        let list = builder.finish();

        // Should not panic — the guard `if transforms.len() > 1` prevents underflow.
        let _scene = SceneBuilder::build(&list, 1.0, 1.0, &ScrollOffsets::new(), &Vec::new());
    }
}
