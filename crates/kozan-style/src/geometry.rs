//! Geometry types for CSS positioning and motion paths.

use crate::specified::{Length, LengthPercentage};
use crate::computed::Percentage;
use crate::Url;
use kozan_style_macros::ToComputedValue;

/// A 2D position value (background-position, object-position, etc.).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct Position2D {
    pub x: LengthPercentage,
    pub y: LengthPercentage,
}

impl Default for Position2D {
    fn default() -> Self {
        Self {
            x: LengthPercentage::Percentage(Percentage::new(0.5)),
            y: LengthPercentage::Percentage(Percentage::new(0.5)),
        }
    }
}

/// Border corner radius — can have different horizontal and vertical values.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct CornerRadius {
    pub horizontal: LengthPercentage,
    pub vertical: LengthPercentage,
}

impl CornerRadius {
    /// Creates a zero corner radius.
    pub fn zero() -> Self {
        Self {
            horizontal: LengthPercentage::from(crate::specified::length::px(0.0)),
            vertical: LengthPercentage::from(crate::specified::length::px(0.0)),
        }
    }

    /// Creates a corner radius with equal horizontal and vertical values.
    pub fn uniform(v: LengthPercentage) -> Self {
        Self {
            horizontal: v.clone(),
            vertical: v,
        }
    }
}

impl Default for CornerRadius {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<LengthPercentage> for CornerRadius {
    fn from(v: LengthPercentage) -> Self {
        Self::uniform(v)
    }
}

impl From<Length> for CornerRadius {
    fn from(v: Length) -> Self {
        Self::uniform(LengthPercentage::from(v))
    }
}

impl From<f32> for CornerRadius {
    fn from(v: f32) -> Self {
        Self::uniform(LengthPercentage::from(crate::specified::length::px(v)))
    }
}

// ---- Motion Path (offset-*) types ----

/// CSS `offset-path` — `none | path(<string>) | url(<string>) | ray(<angle> <size>? contain?)`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum OffsetPath {
    None,
    /// SVG path data string, e.g. `path("M 0 0 L 100 100")`.
    Path(Box<str>),
    /// URL reference to an SVG shape element.
    Url(Url),
    /// `ray(<angle> [closest-side | closest-corner | farthest-side | farthest-corner | sides]? contain?)`.
    Ray {
        angle: f32,
        size: RaySize,
        contain: bool,
    },
}

impl Default for OffsetPath {
    fn default() -> Self { Self::None }
}

/// Size keyword for `ray()` function in `offset-path`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToComputedValue)]
pub enum RaySize {
    ClosestSide,
    ClosestCorner,
    FarthestSide,
    FarthestCorner,
    Sides,
}

impl Default for RaySize {
    fn default() -> Self { Self::ClosestSide }
}

/// CSS `offset-rotate` — `auto | reverse | <angle> | auto <angle> | reverse <angle>`.
#[derive(Clone, Copy, Debug, PartialEq, ToComputedValue)]
pub enum OffsetRotate {
    /// `auto` — rotate by the direction of the motion path.
    Auto,
    /// `reverse` — auto + 180deg.
    Reverse,
    /// Fixed angle in degrees.
    Angle(f32),
    /// `auto <angle>` — auto + additional rotation.
    AutoAngle(f32),
    /// `reverse <angle>` — reverse + additional rotation.
    ReverseAngle(f32),
}

impl Default for OffsetRotate {
    fn default() -> Self { Self::Auto }
}

/// CSS `offset-position` — `normal | auto | <position>`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum OffsetPosition {
    Normal,
    Auto,
    Position(Position2D),
}

impl Default for OffsetPosition {
    fn default() -> Self { Self::Normal }
}
