//! Geometry types for CSS positioning.

use crate::specified::{Length, LengthPercentage};
use crate::computed::Percentage;
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
