//! CSS image and gradient values, plus background/mask per-layer list types.

use crate::{Atom, Color};
use crate::specified::LengthPercentage;
use kozan_style_macros::ToComputedValue;

/// A single CSS image value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum Image {
    None,
    Url(Atom),
    LinearGradient(Box<LinearGradient>),
    RadialGradient(Box<RadialGradient>),
    ConicGradient(Box<ConicGradient>),
}

impl Default for Image {
    fn default() -> Self { Self::None }
}

/// CSS `background-image` / `mask-image`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ImageList {
    None,
    Images(Box<[Image]>),
}

impl Default for ImageList {
    fn default() -> Self { Self::None }
}

/// CSS `linear-gradient()`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct LinearGradient {
    pub angle: f32,
    pub stops: Box<[ColorStop]>,
}

/// CSS `radial-gradient()`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct RadialGradient {
    pub shape: RadialShape,
    pub stops: Box<[ColorStop]>,
}

/// CSS `conic-gradient()`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ConicGradient {
    pub from_angle: f32,
    pub stops: Box<[ColorStop]>,
}

/// CSS radial gradient shape keyword.
#[derive(Clone, Copy, Debug, PartialEq, ToComputedValue)]
pub enum RadialShape { Circle, Ellipse }

/// A color stop in a CSS gradient.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ColorStop {
    pub color: Color,
    pub position: Option<f32>,
}

// ─── BackgroundSize ───────────────────────────────────────────────────────────

/// CSS `<bg-size>` value.
///
/// Spec: CSS Backgrounds Level 3 §3.9
/// <https://www.w3.org/TR/css-backgrounds-3/#the-background-size>
///
/// ```text
/// <bg-size> = [ <length-percentage [0,∞]> | auto ]{1,2} | cover | contain
/// ```
///
/// `Explicit { width: None, height: None }` encodes `auto auto` (the initial value).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum BackgroundSize {
    /// `cover` — scale to cover the container while preserving aspect ratio.
    Cover,
    /// `contain` — scale to fit within the container while preserving aspect ratio.
    Contain,
    /// `[ <length-percentage> | auto ]{1,2}`.
    /// `None` on an axis means `auto` for that axis.
    Explicit {
        width: Option<LengthPercentage>,
        height: Option<LengthPercentage>,
    },
}

impl Default for BackgroundSize {
    /// Initial value is `auto auto` (both axes implicit).
    fn default() -> Self {
        Self::Explicit { width: None, height: None }
    }
}

// ─── Background / mask per-layer list types ───────────────────────────────────
//
// CSS Backgrounds Level 3 §3 and CSS Masking Level 1 §6 define each background
// and mask longhand (except background-color) as a comma-separated list, one
// item per layer.  `ImageList` covers `background-image` / `mask-image`.
// The types below cover the remaining per-layer properties.
//
// All of these are non-generic, so `#[derive(ToComputedValue)]` generates the
// identity impl (`type ComputedValue = Self; to_computed_value = clone`), which
// is correct because the inner enum types are already their own computed values.

/// CSS `background-position-x` — comma-separated list of position components.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct PositionComponentList(pub Box<[crate::PositionComponent]>);

impl Default for PositionComponentList {
    fn default() -> Self { Self(Box::from([crate::PositionComponent::default()])) }
}

/// CSS `background-size` / `mask-size` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct BackgroundSizeList(pub Box<[crate::BackgroundSize]>);

impl Default for BackgroundSizeList {
    fn default() -> Self { Self(Box::from([crate::BackgroundSize::default()])) }
}

/// CSS `background-repeat` / `mask-repeat` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct BackgroundRepeatList(pub Box<[crate::BackgroundRepeat]>);

impl Default for BackgroundRepeatList {
    fn default() -> Self { Self(Box::from([crate::BackgroundRepeat::Repeat])) }
}

/// CSS `background-attachment` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct BackgroundAttachmentList(pub Box<[crate::BackgroundAttachment]>);

impl Default for BackgroundAttachmentList {
    fn default() -> Self { Self(Box::from([crate::BackgroundAttachment::Scroll])) }
}

/// CSS `background-clip` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct BackgroundClipList(pub Box<[crate::BackgroundClip]>);

impl Default for BackgroundClipList {
    fn default() -> Self { Self(Box::from([crate::BackgroundClip::BorderBox])) }
}

/// CSS `background-origin` / `mask-origin` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct BackgroundOriginList(pub Box<[crate::BackgroundOrigin]>);

impl Default for BackgroundOriginList {
    fn default() -> Self { Self(Box::from([crate::BackgroundOrigin::PaddingBox])) }
}

/// CSS `mask-mode` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct MaskModeList(pub Box<[crate::MaskMode]>);

impl Default for MaskModeList {
    fn default() -> Self { Self(Box::from([crate::MaskMode::MatchSource])) }
}

/// CSS `mask-clip` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct MaskClipList(pub Box<[crate::MaskClip]>);

impl Default for MaskClipList {
    fn default() -> Self { Self(Box::from([crate::MaskClip::BorderBox])) }
}

/// CSS `mask-composite` — comma-separated list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct MaskCompositeList(pub Box<[crate::MaskComposite]>);

impl Default for MaskCompositeList {
    fn default() -> Self { Self(Box::from([crate::MaskComposite::Add])) }
}

/// CSS `mask-position` — comma-separated list of 2D positions.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct Position2DList(pub Box<[crate::Position2D]>);

impl Default for Position2DList {
    fn default() -> Self { Self(Box::from([crate::Position2D::default()])) }
}
