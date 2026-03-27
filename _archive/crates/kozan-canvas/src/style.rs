//! Paint style types for canvas fill and stroke operations.
//!
//! Chrome equivalent: `CanvasStyle` (color, gradient, or pattern).

use kozan_primitives::color::Color;
use kozan_primitives::geometry::Point;

/// How a fill or stroke is painted.
///
/// Chrome equivalent: the union of color/`CanvasGradient`/`CanvasPattern`
/// stored in `CanvasRenderingContext2DState::fill_style_`.
#[derive(Clone, Debug)]
pub enum PaintStyle {
    Color(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    ConicGradient(ConicGradient),
    Pattern(Pattern),
}

/// A linear gradient between two points.
///
/// Chrome equivalent: `CanvasGradient` with type `kLinear`.
#[derive(Clone, Debug)]
pub struct LinearGradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<GradientStop>,
}

/// A radial gradient between two circles.
///
/// Chrome equivalent: `CanvasGradient` with type `kRadial`.
#[derive(Clone, Debug)]
pub struct RadialGradient {
    pub start_center: Point,
    pub start_radius: f32,
    pub end_center: Point,
    pub end_radius: f32,
    pub stops: Vec<GradientStop>,
}

/// A conic (sweep) gradient around a center point.
///
/// Chrome equivalent: `CanvasGradient` with type `kConic`.
#[derive(Clone, Debug)]
pub struct ConicGradient {
    pub center: Point,
    pub start_angle: f32,
    pub stops: Vec<GradientStop>,
}

/// A color stop within a gradient.
#[derive(Clone, Copy, Debug)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

/// A repeated image pattern.
///
/// Chrome equivalent: `CanvasPattern`.
#[derive(Clone, Debug)]
pub struct Pattern {
    pub image: PatternImage,
    pub repetition: PatternRepetition,
}

/// The source image for a pattern.
#[derive(Clone, Debug)]
pub enum PatternImage {
    ImageData(crate::image::ImageData),
}

/// How a pattern repeats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PatternRepetition {
    #[default]
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
}

/// Winding rule for fill and clip operations.
///
/// Chrome equivalent: `SkPathFillType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

impl Default for PaintStyle {
    fn default() -> Self {
        Self::Color(Color::BLACK)
    }
}
