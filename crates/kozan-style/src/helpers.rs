//! Constructor helpers for complex CSS value types.
//!
//! List types use `Box<[T]>` — frozen after creation.
//! - **from_vec**: `Type::from_vec(vec![...])` — takes ownership, shrinks to exact fit.
//! - **from_slice**: `Type::from_slice(&[...])` — clones items into a boxed slice.

use crate::*;

// Shadow

/// Creates a box shadow with the given parameters.
pub fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: Color, inset: bool) -> Shadow {
    Shadow { offset_x: x, offset_y: y, blur, spread, color, inset }
}

/// Creates an outset shadow with no spread (drop shadow).
pub fn drop_shadow(x: f32, y: f32, blur: f32, color: Color) -> Shadow {
    Shadow { offset_x: x, offset_y: y, blur, spread: 0.0, color, inset: false }
}

impl ShadowList {
    /// Creates a shadow list from a vec.
    pub fn from_vec(items: Vec<Shadow>) -> Self { Self::Shadows(items.into_boxed_slice()) }
    /// Creates a shadow list by cloning a slice.
    pub fn from_slice(items: &[Shadow]) -> Self { Self::Shadows(Box::from(items)) }
}

/// `shadows![drop_shadow(0.0, 2.0, 4.0, Color::BLACK)]`
#[macro_export]
macro_rules! shadows {
    ($($item:expr),+ $(,)?) => {
        $crate::ShadowList::from_vec(vec![$($item),+])
    };
}

// Filter

/// Creates a `blur()` filter with the given radius in px.
pub fn blur(px: f32) -> FilterFunction { FilterFunction::Blur(px) }
/// Creates a `brightness()` filter (1.0 = unchanged).
pub fn brightness(v: f32) -> FilterFunction { FilterFunction::Brightness(v) }
/// Creates a `contrast()` filter (1.0 = unchanged).
pub fn contrast(v: f32) -> FilterFunction { FilterFunction::Contrast(v) }
/// Creates a `grayscale()` filter (0.0..1.0).
pub fn grayscale(v: f32) -> FilterFunction { FilterFunction::Grayscale(v) }
/// Creates a `hue-rotate()` filter in degrees.
pub fn hue_rotate(deg: f32) -> FilterFunction { FilterFunction::HueRotate(deg) }
/// Creates an `invert()` filter (0.0..1.0).
pub fn invert(v: f32) -> FilterFunction { FilterFunction::Invert(v) }
/// Creates a `saturate()` filter (1.0 = unchanged).
pub fn saturate(v: f32) -> FilterFunction { FilterFunction::Saturate(v) }
/// Creates a `sepia()` filter (0.0..1.0).
pub fn sepia(v: f32) -> FilterFunction { FilterFunction::Sepia(v) }

impl FilterList {
    /// Creates a filter list from a vec.
    pub fn from_vec(items: Vec<FilterFunction>) -> Self { Self::Filters(items.into_boxed_slice()) }
    /// Creates a filter list by cloning a slice.
    pub fn from_slice(items: &[FilterFunction]) -> Self { Self::Filters(Box::from(items)) }
}

/// `filters![blur(5.0), brightness(1.2)]`
#[macro_export]
macro_rules! filters {
    ($($item:expr),+ $(,)?) => {
        $crate::FilterList::from_vec(vec![$($item),+])
    };
}

// Transform

/// Creates a `translate(x, y)` transform function.
pub fn translate(x: specified::Length, y: specified::Length) -> TransformFunction {
    TransformFunction::Translate(x.into(), y.into())
}
/// Creates a `translateX()` transform function.
pub fn translate_x(x: specified::Length) -> TransformFunction { TransformFunction::TranslateX(x.into()) }
/// Creates a `translateY()` transform function.
pub fn translate_y(y: specified::Length) -> TransformFunction { TransformFunction::TranslateY(y.into()) }
/// Creates a `translateZ()` transform function.
pub fn translate_z(z: specified::Length) -> TransformFunction { TransformFunction::TranslateZ(z.into()) }
/// Creates a `translate3d()` transform function.
pub fn translate_3d(x: specified::Length, y: specified::Length, z: specified::Length) -> TransformFunction {
    TransformFunction::Translate3d(x.into(), y.into(), z.into())
}
/// Creates a `rotate()` transform in degrees.
pub fn rotate(deg: f32) -> TransformFunction { TransformFunction::Rotate(deg) }
/// Creates a `rotateX()` transform in degrees.
pub fn rotate_x(deg: f32) -> TransformFunction { TransformFunction::RotateX(deg) }
/// Creates a `rotateY()` transform in degrees.
pub fn rotate_y(deg: f32) -> TransformFunction { TransformFunction::RotateY(deg) }
/// Creates a `rotateZ()` transform in degrees.
pub fn rotate_z(deg: f32) -> TransformFunction { TransformFunction::RotateZ(deg) }
/// Creates a `rotate3d()` transform around an arbitrary axis.
pub fn rotate_3d(x: f32, y: f32, z: f32, deg: f32) -> TransformFunction { TransformFunction::Rotate3d(x, y, z, deg) }
/// Creates a uniform `scale()` transform.
pub fn scale(v: f32) -> TransformFunction { TransformFunction::Scale(v, v) }
/// Creates a `scale()` transform with separate X and Y factors.
pub fn scale_xy(x: f32, y: f32) -> TransformFunction { TransformFunction::Scale(x, y) }
/// Creates a `scaleX()` transform.
pub fn scale_x(x: f32) -> TransformFunction { TransformFunction::ScaleX(x) }
/// Creates a `scaleY()` transform.
pub fn scale_y(y: f32) -> TransformFunction { TransformFunction::ScaleY(y) }
/// Creates a `skew()` transform in degrees.
pub fn skew(x: f32, y: f32) -> TransformFunction { TransformFunction::Skew(x, y) }
/// Creates a `skewX()` transform in degrees.
pub fn skew_x(x: f32) -> TransformFunction { TransformFunction::SkewX(x) }
/// Creates a `skewY()` transform in degrees.
pub fn skew_y(y: f32) -> TransformFunction { TransformFunction::SkewY(y) }
/// Creates a `perspective()` transform.
pub fn perspective(d: specified::Length) -> TransformFunction { TransformFunction::Perspective(d.into()) }

impl TransformList {
    /// Creates a transform list from a vec.
    pub fn from_vec(items: Vec<TransformFunction>) -> Self { Self::Functions(items.into_boxed_slice()) }
    /// Creates a transform list by cloning a slice.
    pub fn from_slice(items: &[TransformFunction]) -> Self { Self::Functions(Box::from(items)) }
}

/// `transforms![translate(px(10.0), px(20.0)), rotate(45.0)]`
#[macro_export]
macro_rules! transforms {
    ($($item:expr),+ $(,)?) => {
        $crate::TransformList::from_vec(vec![$($item),+])
    };
}

// Grid

/// Creates a fractional (`fr`) grid track size.
pub fn fr(v: f32) -> TrackSize { TrackSize::Fr(v) }
/// Creates a fixed-length grid track size.
pub fn track_length(v: specified::Length) -> TrackSize { TrackSize::Length(v.into()) }
/// Creates a `fit-content()` grid track size.
pub fn fit_content(v: specified::Length) -> TrackSize { TrackSize::FitContent(v.into()) }

/// Creates a `minmax()` grid track size.
pub fn minmax(min: TrackSize, max: TrackSize) -> TrackSize {
    TrackSize::MinMax(Box::new(min), Box::new(max))
}

/// Creates a numeric grid line placement.
pub fn grid_line(n: i32) -> GridLine { GridLine::Line(n) }
/// Creates a `span N` grid line placement.
pub fn grid_span(n: i32) -> GridLine { GridLine::Span(n) }

/// `grid_areas!["header header", "sidebar main", "footer footer"]`
#[macro_export]
macro_rules! grid_areas {
    ($($row:expr),+ $(,)?) => {
        $crate::GridTemplateAreas::Areas(Box::from([
            $(
                $row.split_whitespace()
                    .map(|s| if s == "." { None } else { Some($crate::Atom::new(s)) })
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            ),+
        ]))
    };
}

// Font

/// Creates a named font family entry.
pub fn font_named(name: &str) -> FamilyEntry { FamilyEntry::Named(Atom::new(name)) }
/// Creates a generic font family entry.
pub fn font_generic(family: GenericFamily) -> FamilyEntry { FamilyEntry::Generic(family) }

impl FontFamily {
    /// Creates a font family list from a vec.
    pub fn from_vec(items: Vec<FamilyEntry>) -> Self { Self(items.into_boxed_slice()) }
    /// Creates a font family list by cloning a slice.
    pub fn from_slice(items: &[FamilyEntry]) -> Self { Self(Box::from(items)) }
}

/// `font_family![font_named("Inter"), font_generic(GenericFamily::SansSerif)]`
#[macro_export]
macro_rules! font_family {
    ($($item:expr),+ $(,)?) => {
        $crate::FontFamily::from_vec(vec![$($item),+])
    };
}

// Gradient

/// Creates a gradient color stop with an optional position.
pub fn color_stop(color: Color, position: Option<f32>) -> ColorStop { ColorStop { color, position } }
/// Creates a gradient color stop at a specific percentage.
pub fn stop_at(color: Color, pct: f32) -> ColorStop { ColorStop { color, position: Some(pct) } }

// Calc: css_min!, css_max!, css_clamp!
// Uses Box::from([...]) — fixed-size array on heap, no Vec overhead.

/// `css_min![px(100.0), percent(50.0)]`
#[macro_export]
macro_rules! css_min {
    ($($arg:expr),+ $(,)?) => {
        $crate::specified::LengthPercentage::Calc(Box::new(
            $crate::CalcNode::MinMax(
                Box::from([$($crate::CalcNode::Leaf($crate::specified::SpecifiedLeaf::from($arg))),+]),
                $crate::MinMaxOp::Min,
            )
        ))
    };
}

/// `css_max![px(200.0), em(10.0)]`
#[macro_export]
macro_rules! css_max {
    ($($arg:expr),+ $(,)?) => {
        $crate::specified::LengthPercentage::Calc(Box::new(
            $crate::CalcNode::MinMax(
                Box::from([$($crate::CalcNode::Leaf($crate::specified::SpecifiedLeaf::from($arg))),+]),
                $crate::MinMaxOp::Max,
            )
        ))
    };
}

/// `css_clamp![px(100.0), percent(50.0), px(800.0)]`
#[macro_export]
macro_rules! css_clamp {
    ($min:expr, $center:expr, $max:expr) => {
        $crate::specified::LengthPercentage::Calc(Box::new(
            $crate::CalcNode::Clamp {
                min: Box::new($crate::CalcNode::Leaf($crate::specified::SpecifiedLeaf::from($min))),
                center: Box::new($crate::CalcNode::Leaf($crate::specified::SpecifiedLeaf::from($center))),
                max: Box::new($crate::CalcNode::Leaf($crate::specified::SpecifiedLeaf::from($max))),
            }
        ))
    };
}

// Color

/// `rgb(255, 0, 0)` — opaque sRGB color from u8.
pub fn rgb(r: u8, g: u8, b: u8) -> Color { Color::rgb(r, g, b) }

/// `rgba(255, 0, 0, 128)` — sRGB color with alpha from u8.
pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color { Color::rgba(r, g, b, a) }

/// `hex(0xFF0000)` — opaque sRGB from 24-bit integer hex.
pub fn hex(v: u32) -> Color { Color::from_hex(v) }

/// `hex_str("#FF0000")` or `hex_str("FF0000")` — parse hex string to color.
pub fn hex_str(s: &str) -> Color {
    let s = s.strip_prefix('#').unwrap_or(s);
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    if s.len() == 8 {
        // #RRGGBBAA
        Color::Absolute(AbsoluteColor::from_u8(
            (v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8,
        ))
    } else {
        // #RRGGBB
        Color::from_hex(v)
    }
}

/// `oklch(0.7, 0.15, 180.0)` — opaque OKLCh color.
pub fn oklch(l: f32, c: f32, h: f32) -> Color { Color::Absolute(AbsoluteColor::oklch(l, c, h, 1.0)) }

/// `oklcha(0.7, 0.15, 180.0, 0.8)` — OKLCh color with alpha.
pub fn oklcha(l: f32, c: f32, h: f32, a: f32) -> Color { Color::Absolute(AbsoluteColor::oklch(l, c, h, a)) }

/// `oklab(0.7, -0.1, 0.1)` — opaque OKLab color.
pub fn oklab(l: f32, a: f32, b: f32) -> Color { Color::Absolute(AbsoluteColor::oklab(l, a, b, 1.0)) }

/// `hsl(120.0, 1.0, 0.5)` — opaque HSL color (h in degrees, s/l in 0..1).
pub fn hsl(h: f32, s: f32, l: f32) -> Color { Color::Absolute(AbsoluteColor::hsl(h, s, l, 1.0)) }

/// `hsla(120.0, 1.0, 0.5, 0.8)` — HSL color with alpha.
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color { Color::Absolute(AbsoluteColor::hsl(h, s, l, a)) }

/// `p3(1.0, 0.0, 0.0)` — opaque Display P3 color.
pub fn p3(r: f32, g: f32, b: f32) -> Color { Color::Absolute(AbsoluteColor::display_p3(r, g, b, 1.0)) }

/// `current_color()` — the `currentColor` keyword.
pub fn current_color() -> Color { Color::CurrentColor }

/// `light_dark(light, dark)` — picks based on `color-scheme`.
pub fn light_dark(light: Color, dark: Color) -> Color { Color::light_dark(light, dark) }

// Duration / Timing

/// Creates a duration from seconds.
pub fn secs(v: f32) -> std::time::Duration { std::time::Duration::from_secs_f32(v) }
/// Creates a duration from milliseconds.
pub fn ms(v: u64) -> std::time::Duration { std::time::Duration::from_millis(v) }
/// Creates a `cubic-bezier()` timing function.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> TimingFunction { TimingFunction::CubicBezier(x1, y1, x2, y2) }
/// Creates a `steps()` timing function.
pub fn steps(count: u32, position: StepPosition) -> TimingFunction { TimingFunction::Steps(count, position) }

// SVG

impl StrokeDasharray {
    /// Creates a dasharray from a vec.
    pub fn from_vec(values: Vec<specified::LengthPercentage>) -> Self { Self::Values(values.into_boxed_slice()) }
    /// Creates a dasharray by cloning a slice.
    pub fn from_slice(values: &[specified::LengthPercentage]) -> Self { Self::Values(Box::from(values)) }
}
