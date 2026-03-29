//! CSS transform values.

use crate::specified::LengthPercentage;
use crate::computed::Percentage;
use kozan_style_macros::ToComputedValue;

/// CSS `transform` — `none` or list of transform functions.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TransformList {
    None,
    Functions(Box<[TransformFunction]>),
}

impl Default for TransformList {
    fn default() -> Self { Self::None }
}

/// Individual CSS transform function.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TransformFunction {
    Translate(LengthPercentage, LengthPercentage),
    TranslateX(LengthPercentage),
    TranslateY(LengthPercentage),
    TranslateZ(LengthPercentage),
    Translate3d(LengthPercentage, LengthPercentage, LengthPercentage),
    Scale(f32, f32),
    ScaleX(f32),
    ScaleY(f32),
    ScaleZ(f32),
    Scale3d(f32, f32, f32),
    Rotate(f32),
    RotateX(f32),
    RotateY(f32),
    RotateZ(f32),
    Rotate3d(f32, f32, f32, f32),
    Skew(f32, f32),
    SkewX(f32),
    SkewY(f32),
    Perspective(LengthPercentage),
    Matrix(Box<[f64; 6]>),
    Matrix3d(Box<[f64; 16]>),
}

/// CSS `transform-origin`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct TransformOrigin {
    pub x: LengthPercentage,
    pub y: LengthPercentage,
    pub z: LengthPercentage,
}

impl Default for TransformOrigin {
    fn default() -> Self {
        Self {
            x: LengthPercentage::Percentage(Percentage::new(0.5)),
            y: LengthPercentage::Percentage(Percentage::new(0.5)),
            z: LengthPercentage::from(crate::specified::length::px(0.0)),
        }
    }
}
