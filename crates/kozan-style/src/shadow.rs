//! CSS shadow values (box-shadow, text-shadow).

use crate::Color;
use kozan_style_macros::ToComputedValue;
/// A single CSS box or text shadow value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

/// CSS `box-shadow` / `text-shadow` — `none` or list of shadows.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ShadowList {
    None,
    Shadows(Box<[Shadow]>),
}

impl Default for ShadowList {
    fn default() -> Self { Self::None }
}
