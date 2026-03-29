//! CSS border-image and border-spacing value types.

use crate::specified::LengthPercentage;
use kozan_style_macros::ToComputedValue;

/// CSS `border-image-slice` — up to 4 values + optional `fill`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct BorderImageSlice {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
    pub fill: bool,
}

impl Default for BorderImageSlice {
    fn default() -> Self {
        Self { top: 100.0, right: 100.0, bottom: 100.0, left: 100.0, fill: false }
    }
}

/// CSS `border-image-repeat` — horizontal and vertical repeat modes.
#[derive(Clone, Copy, Debug, PartialEq, ToComputedValue)]
pub struct BorderImageRepeat {
    pub horizontal: BorderImageRepeatMode,
    pub vertical: BorderImageRepeatMode,
}

impl Default for BorderImageRepeat {
    fn default() -> Self {
        Self {
            horizontal: BorderImageRepeatMode::Stretch,
            vertical: BorderImageRepeatMode::Stretch,
        }
    }
}

/// CSS `border-image-repeat` mode for a single axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToComputedValue)]
pub enum BorderImageRepeatMode {
    Stretch,
    Repeat,
    Round,
    Space,
}

/// CSS `border-spacing` — horizontal and vertical spacing.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct BorderSpacing {
    pub horizontal: LengthPercentage,
    pub vertical: LengthPercentage,
}

impl Default for BorderSpacing {
    fn default() -> Self {
        Self {
            horizontal: LengthPercentage::from(crate::specified::length::px(0.0)),
            vertical: LengthPercentage::from(crate::specified::length::px(0.0)),
        }
    }
}
