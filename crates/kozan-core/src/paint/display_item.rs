//! Display items — the individual draw commands in a display list.
//!
//! Chrome equivalent: `DisplayItem` + `PaintOp` (cc/paint).
//!
//! Each display item represents one visual operation: draw a rect,
//! draw text, draw an image, clip, transform, etc.
//!
//! # Chrome mapping
//!
//! | Chrome | Kozan |
//! |--------|-------|
//! | `DrawingDisplayItem` | `DisplayItem::Draw(DrawCommand)` |
//! | `PaintOp::DrawRectOp` | `DrawCommand::Rect` |
//! | `PaintOp::DrawTextBlobOp` | `DrawCommand::Text` |
//! | `PaintOp::DrawImageRectOp` | `DrawCommand::Image` |
//! | `ClipRectOp` | `DisplayItem::PushClip` |
//! | `SaveLayerAlphaOp` | `DisplayItem::PushOpacity` |

use kozan_primitives::color::Color;
use kozan_primitives::geometry::Rect;

/// How image content should be sized within its destination rectangle.
///
/// Chrome equivalent: `EObjectFit` from `ComputedStyleConstants.h`.
/// Mirrors CSS `object-fit` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectFit {
    #[default]
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

/// A single display item in the paint list.
///
/// Chrome equivalent: `DisplayItem` (blink) + `PaintOp` (cc/paint).
///
/// Display items are produced by the painter walking the fragment tree.
/// They are consumed by the renderer backend (vello, wgpu, etc.).
#[derive(Debug, Clone)]
pub enum DisplayItem {
    /// A concrete draw operation.
    /// Chrome: `DrawingDisplayItem` wrapping a `PaintRecord`.
    Draw(DrawCommand),

    /// Push a clip rectangle onto the clip stack.
    /// Everything drawn until the matching `PopClip` is clipped to this rect.
    /// Chrome: `ClipRectOp`.
    PushClip(ClipData),

    /// Pop the most recent clip from the stack.
    PopClip,

    /// Push a rounded clip onto the clip stack.
    /// Chrome: `ClipRRectOp`.
    PushRoundedClip(RoundedClipData),

    /// Pop the most recent rounded clip from the stack.
    PopRoundedClip,

    /// Push an opacity layer.
    /// Chrome: `SaveLayerAlphaOp`.
    PushOpacity(f32),

    /// Pop the opacity layer.
    PopOpacity,

    /// Push a 2D transform.
    /// Chrome: `ConcatOp(matrix)`.
    PushTransform(TransformData),

    /// Pop the most recent transform.
    PopTransform,

    /// Composite an external GPU surface into the display list.
    ///
    /// Chrome equivalent: `ForeignLayerDisplayItem`.
    /// This is how 3D content (wgpu scenes), video frames, and other GPU
    /// textures are integrated into the 2D rendering pipeline.
    ///
    /// The compositor blends this surface with the surrounding 2D content,
    /// respecting clipping, opacity, and z-ordering from the display list.
    ///
    /// # 2D/3D Unification
    ///
    /// A `<canvas>` element with a wgpu 3D scene produces this item.
    /// The renderer composites it into the final frame alongside vello's
    /// 2D output — enabling game viewports inside UI panels with correct
    /// clipping, layering, and event handling.
    ExternalSurface(ExternalSurfaceData),
}

/// A concrete draw command — the actual pixels to put on screen.
///
/// Chrome equivalent: the `PaintOp` variants that draw things
/// (`DrawRect`, `DrawRRect`, `DrawTextBlob`, `DrawImageRect`, etc.).
#[derive(Debug, Clone)]
pub enum DrawCommand {
    /// Fill a rectangle with a solid color.
    /// Chrome: `PaintOp::DrawRectOp` + `PaintFlags::kFill`.
    Rect { rect: Rect, color: Color },

    /// Fill a rounded rectangle with a solid color.
    /// Chrome: `PaintOp::DrawRRectOp`.
    RoundedRect {
        rect: Rect,
        radii: BorderRadii,
        color: Color,
    },

    /// Draw a rounded border ring (outer rounded rect minus inner rounded rect).
    /// Chrome: `PaintOp::DrawDRRectOp` — draws the difference between two rounded rects.
    /// Used for borders on elements with border-radius. Fills ONLY the ring area,
    /// not the interior — so rgba backgrounds behind it show correctly.
    RoundedBorderRing {
        outer_rect: Rect,
        outer_radii: BorderRadii,
        inner_rect: Rect,
        inner_radii: BorderRadii,
        color: Color,
    },

    /// Draw a border (four edges, each with its own width, color, and style).
    /// Chrome: `PaintOp::DrawDRRectOp` or individual edge drawing.
    /// The `styles` field tells the renderer to draw solid/dashed/dotted/etc.
    Border {
        rect: Rect,
        widths: BorderWidths,
        colors: BorderColors,
        styles: BorderStyles,
    },

    /// Draw pre-shaped glyph runs at a position.
    /// Chrome: `PaintOp::DrawTextBlobOp` — carries a `SkTextBlob` (pre-shaped).
    /// Glyphs shaped ONCE during layout (Parley + `HarfRust`).
    /// Renderer just draws them — ZERO font logic in the GPU layer.
    Text {
        /// Position (x, y) — top-left of the text box.
        x: f32,
        y: f32,
        /// Pre-shaped glyph runs from layout.
        /// Each run has: font data, font size, glyph IDs + positions, color.
        runs: Vec<crate::layout::inline::font_system::ShapedTextRun>,
    },

    /// Draw an image in a destination rectangle.
    /// Chrome: `PaintOp::DrawImageRectOp`.
    Image {
        /// Source image identifier (future: image resource handle).
        source_id: u64,
        /// Destination rectangle in layout coordinates.
        dest: Rect,
        /// How the image content should be sized within the destination rect.
        /// Chrome: `ComputedStyle::GetObjectFit()` read during paint.
        // TODO: object-fit from Stylo (style::computed_values::object_fit::T)
        // Placeholder until image rendering is implemented.
        _object_fit: u8,
    },

    /// Draw a line (for underlines, strikethroughs, hr, etc.).
    /// Chrome: `PaintOp::DrawLineOp`.
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
        color: Color,
    },

    /// Draw an outline (like border but outside the box, doesn't affect layout).
    /// Chrome: `PaintOp::DrawDRRectOp` for the outline ring.
    /// CSS: `outline: 2px solid blue;` — painted outside the border-box.
    Outline {
        /// The border-box rect (outline is drawn OUTSIDE this).
        rect: Rect,
        /// Border radii (outline follows the element's border-radius).
        radii: BorderRadii,
        /// Outline width in pixels.
        width: f32,
        /// Outline offset (CSS `outline-offset`, can be negative).
        offset: f32,
        /// Outline color.
        color: Color,
    },

    // TODO(M7): LinearGradient { rect, stops, angle } — Chrome: PaintOp::DrawPaintOp + cc::PaintShader::MakeLinearGradient.
    // TODO(M7): RadialGradient { rect, stops, center, radius } — Chrome: PaintOp::DrawPaintOp + cc::PaintShader::MakeRadialGradient.
    // TODO(M7): TextShadow { x, y, runs, offset_x, offset_y, blur, color } — Chrome: TextPainter::PaintTextWithShadows().
    /// Draw a box shadow.
    /// Chrome: painted via `PaintOp::DrawRRectOp` with blur filter.
    BoxShadow {
        /// The element's border box.
        rect: Rect,
        /// Horizontal offset.
        offset_x: f32,
        /// Vertical offset.
        offset_y: f32,
        /// Blur radius.
        blur: f32,
        /// Spread radius.
        spread: f32,
        /// Shadow color.
        color: Color,
    },
}

/// Data for compositing an external GPU surface.
///
/// Chrome equivalent: `ForeignLayerDisplayItem` + `cc::SurfaceLayer`.
///
/// The `surface_id` is an opaque handle to a GPU texture/surface
/// owned by external code (3D game engine, video decoder, etc.).
/// The renderer uses this ID to look up the actual GPU resource
/// and composite it at the specified rectangle.
#[derive(Debug, Clone, Copy)]
pub struct ExternalSurfaceData {
    /// Opaque identifier for the GPU surface.
    /// The platform layer resolves this to an actual GPU texture.
    pub surface_id: u64,
    /// Rectangle where the surface should be composited (layout coordinates).
    pub dest: Rect,
}

/// Clip data for `PushClip`.
#[derive(Debug, Clone, Copy)]
pub struct ClipData {
    pub rect: Rect,
}

/// Rounded clip data for `PushRoundedClip`.
#[derive(Debug, Clone, Copy)]
pub struct RoundedClipData {
    pub rect: Rect,
    pub radii: BorderRadii,
}

/// 2D transform data for `PushTransform`.
#[derive(Debug, Clone, Copy)]
pub struct TransformData {
    /// Translation in X.
    pub translate_x: f32,
    /// Translation in Y.
    pub translate_y: f32,
    /// If this transform is a scroll translate, the DOM node that owns it.
    /// The compositor can override this with its own offset without repainting.
    pub scroll_node: Option<u32>,
}

/// Border corner radii (all four corners).
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

/// Border widths (all four edges).
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderWidths {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Border colors (all four edges).
#[derive(Debug, Clone, Copy)]
pub struct BorderColors {
    pub top: Color,
    pub right: Color,
    pub bottom: Color,
    pub left: Color,
}

/// Border styles (all four edges).
#[derive(Debug, Clone, Copy)]
pub struct BorderStyles {
    pub top: style::values::specified::border::BorderStyle,
    pub right: style::values::specified::border::BorderStyle,
    pub bottom: style::values::specified::border::BorderStyle,
    pub left: style::values::specified::border::BorderStyle,
}

impl Default for BorderStyles {
    fn default() -> Self {
        Self {
            top: style::values::specified::border::BorderStyle::None,
            right: style::values::specified::border::BorderStyle::None,
            bottom: style::values::specified::border::BorderStyle::None,
            left: style::values::specified::border::BorderStyle::None,
        }
    }
}

impl Default for BorderColors {
    fn default() -> Self {
        Self {
            top: Color::BLACK,
            right: Color::BLACK,
            bottom: Color::BLACK,
            left: Color::BLACK,
        }
    }
}

impl DisplayItem {
    /// Whether this item is a draw command (produces pixels).
    #[must_use]
    pub fn is_draw(&self) -> bool {
        matches!(self, DisplayItem::Draw(_))
    }

    /// Whether this item is a push operation (needs a matching pop).
    #[must_use]
    pub fn is_push(&self) -> bool {
        matches!(
            self,
            DisplayItem::PushClip(_)
                | DisplayItem::PushRoundedClip(_)
                | DisplayItem::PushOpacity(_)
                | DisplayItem::PushTransform(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_item_classification() {
        let rect = DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 50.0),
            color: Color::RED,
        });
        assert!(rect.is_draw());
        assert!(!rect.is_push());

        let clip = DisplayItem::PushClip(ClipData {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        });
        assert!(!clip.is_draw());
        assert!(clip.is_push());
    }

    #[test]
    fn border_radii_default() {
        let radii = BorderRadii::default();
        assert_eq!(radii.top_left, 0.0);
        assert_eq!(radii.bottom_right, 0.0);
    }

    #[test]
    fn border_colors_default() {
        let colors = BorderColors::default();
        assert_eq!(colors.top, Color::BLACK);
    }
}
