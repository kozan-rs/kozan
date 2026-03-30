//! CSS text emphasis and background position types.

use crate::{Atom, TextEmphasisFill, TextEmphasisShape};
use crate::specified::LengthPercentage;
use crate::computed::Percentage;
use kozan_style_macros::ToComputedValue;

/// CSS `text-emphasis-style` — `none`, shape with fill, or custom string.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TextEmphasisStyleValue {
    None,
    Shape(TextEmphasisFill, TextEmphasisShape),
    Custom(Atom),
}

impl Default for TextEmphasisStyleValue {
    fn default() -> Self {
        Self::None
    }
}

/// CSS `background-position-x/y` component — keyword or length.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum PositionComponent {
    Left,
    Center,
    Right,
    Top,
    Bottom,
    Length(LengthPercentage),
}

impl Default for PositionComponent {
    fn default() -> Self {
        Self::Length(LengthPercentage::Percentage(Percentage::new(0.0)))
    }
}

/// CSS `font-palette` — `normal | light | dark | <custom-ident>`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum FontPalette {
    Normal,
    Light,
    Dark,
    Custom(Atom),
}

impl Default for FontPalette {
    fn default() -> Self { Self::Normal }
}

/// CSS `initial-letter` — `normal | <number> [<integer> | drop | raise]?`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum InitialLetter {
    Normal,
    Raised { size: f32, sink: u32 },
}

impl Default for InitialLetter {
    fn default() -> Self { Self::Normal }
}

/// CSS `hyphenate-character` — `auto | <string>`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum HyphenateCharacter {
    Auto,
    String(Box<str>),
}

impl Default for HyphenateCharacter {
    fn default() -> Self { Self::Auto }
}

/// Per-value slot for `hyphenate-limit-chars`: `auto | <integer>`.
#[derive(Clone, Copy, Debug, PartialEq, ToComputedValue)]
pub enum HyphenateLimitValue {
    Auto,
    Integer(u32),
}

/// CSS `hyphenate-limit-chars` — `auto | <integer>{1,3}`.
///
/// Specifies minimum total, before-break, and after-break character counts.
#[derive(Clone, Copy, Debug, PartialEq, ToComputedValue)]
pub struct HyphenateLimitChars {
    pub total: HyphenateLimitValue,
    pub before: HyphenateLimitValue,
    pub after: HyphenateLimitValue,
}

impl Default for HyphenateLimitChars {
    fn default() -> Self {
        Self {
            total: HyphenateLimitValue::Auto,
            before: HyphenateLimitValue::Auto,
            after: HyphenateLimitValue::Auto,
        }
    }
}

/// CSS `image-orientation` — `from-image | <angle>`.
#[derive(Clone, Copy, Debug, PartialEq, ToComputedValue)]
pub enum ImageOrientation {
    FromImage,
    /// Angle in degrees (multiples of 90 per spec, but stored as-is).
    Angle(f32),
}

impl Default for ImageOrientation {
    fn default() -> Self { Self::FromImage }
}
