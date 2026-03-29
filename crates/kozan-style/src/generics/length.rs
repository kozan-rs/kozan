//! Generic length-based property types.
//!
//! Each type is parameterized over `LP` (LengthPercentage), so the same
//! enum definition works at both specified and computed levels.

use kozan_style_macros::ToComputedValue;

/// CSS `width`, `height`, `min-width`, `min-height`.
///
/// `<length-percentage> | auto | min-content | max-content | fit-content | stretch`
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum Size<LP> {
    LengthPercentage(LP),
    Auto,
    MinContent,
    MaxContent,
    FitContent,
    FitContentFunction(LP),
    Stretch,
}

impl<LP> Size<LP> {
    /// Returns `true` if this is `auto`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl<LP: Default> Default for Size<LP> {
    fn default() -> Self {
        Self::Auto
    }
}

impl<LP: core::fmt::Display> core::fmt::Display for Size<LP> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LengthPercentage(lp) => write!(f, "{lp}"),
            Self::Auto => f.write_str("auto"),
            Self::MinContent => f.write_str("min-content"),
            Self::MaxContent => f.write_str("max-content"),
            Self::FitContent => f.write_str("fit-content"),
            Self::FitContentFunction(lp) => write!(f, "fit-content({lp})"),
            Self::Stretch => f.write_str("stretch"),
        }
    }
}

/// CSS `max-width`, `max-height`.
///
/// `<length-percentage> | none | min-content | max-content | fit-content | stretch`
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum MaxSize<LP> {
    LengthPercentage(LP),
    None,
    MinContent,
    MaxContent,
    FitContent,
    FitContentFunction(LP),
    Stretch,
}

impl<LP> MaxSize<LP> {
    /// Returns `true` if this is `none`.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl<LP> Default for MaxSize<LP> {
    fn default() -> Self {
        Self::None
    }
}

impl<LP: core::fmt::Display> core::fmt::Display for MaxSize<LP> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LengthPercentage(lp) => write!(f, "{lp}"),
            Self::None => f.write_str("none"),
            Self::MinContent => f.write_str("min-content"),
            Self::MaxContent => f.write_str("max-content"),
            Self::FitContent => f.write_str("fit-content"),
            Self::FitContentFunction(lp) => write!(f, "fit-content({lp})"),
            Self::Stretch => f.write_str("stretch"),
        }
    }
}

/// CSS `margin-*`.
///
/// `<length-percentage> | auto`
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum Margin<LP> {
    LengthPercentage(LP),
    Auto,
}

impl<LP> Margin<LP> {
    /// Returns `true` if this is `auto`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl<LP: Default> Default for Margin<LP> {
    fn default() -> Self {
        Self::LengthPercentage(LP::default())
    }
}

impl<LP: core::fmt::Display> core::fmt::Display for Margin<LP> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LengthPercentage(lp) => write!(f, "{lp}"),
            Self::Auto => f.write_str("auto"),
        }
    }
}

/// `<length-percentage> | auto` — for insets, flex-basis, etc.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum LengthPercentageOrAuto<LP> {
    LengthPercentage(LP),
    Auto,
}

impl<LP> LengthPercentageOrAuto<LP> {
    /// Returns `true` if this is `auto`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl<LP> Default for LengthPercentageOrAuto<LP> {
    fn default() -> Self {
        Self::Auto
    }
}

impl<LP: core::fmt::Display> core::fmt::Display for LengthPercentageOrAuto<LP> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LengthPercentage(lp) => write!(f, "{lp}"),
            Self::Auto => f.write_str("auto"),
        }
    }
}

/// `<length-percentage> | normal` — for line-height, letter-spacing, etc.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum LengthPercentageOrNormal<LP> {
    LengthPercentage(LP),
    Normal,
}

impl<LP> LengthPercentageOrNormal<LP> {
    /// Returns `true` if this is `normal`.
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }
}

impl<LP> Default for LengthPercentageOrNormal<LP> {
    fn default() -> Self {
        Self::Normal
    }
}

impl<LP: core::fmt::Display> core::fmt::Display for LengthPercentageOrNormal<LP> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LengthPercentage(lp) => write!(f, "{lp}"),
            Self::Normal => f.write_str("normal"),
        }
    }
}
