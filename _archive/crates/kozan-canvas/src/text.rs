//! Text alignment, baseline, and measurement types.
//!
//! Chrome equivalent: text fields on `CanvasRenderingContext2DState`.

/// Horizontal text alignment for `fillText`/`strokeText`.
///
/// Chrome equivalent: `CanvasTextAlign`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
}

/// Vertical text baseline.
///
/// Chrome equivalent: `CanvasTextBaseline`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextBaseline {
    Top,
    Hanging,
    Middle,
    #[default]
    Alphabetic,
    Ideographic,
    Bottom,
}

/// Text direction.
///
/// Chrome equivalent: `CanvasDirection`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextDirection {
    #[default]
    Ltr,
    Rtl,
    Inherit,
}

/// Measurement result from `measureText()`.
///
/// Chrome equivalent: `TextMetrics`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextMetrics {
    pub width: f32,
    pub actual_bounding_box_left: f32,
    pub actual_bounding_box_right: f32,
    pub actual_bounding_box_ascent: f32,
    pub actual_bounding_box_descent: f32,
    pub font_bounding_box_ascent: f32,
    pub font_bounding_box_descent: f32,
    pub em_height_ascent: f32,
    pub em_height_descent: f32,
}
