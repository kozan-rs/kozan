//! Generic wrapper types for CSS values.
//! `auto | <T>`, `none | <T>`, `normal | <T>`

use kozan_style_macros::ToComputedValue;

/// CSS value: `auto | <T>`.
/// Used by z-index, aspect-ratio, column-count, caret-color, etc.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum AutoOr<T> {
    Auto,
    Value(T),
}

impl<T> AutoOr<T> {
    /// Returns `true` if this is `Auto`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Returns the inner value, or `None` if `Auto`.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            Self::Auto => None,
        }
    }

    /// Transforms the inner value, preserving `Auto`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AutoOr<U> {
        match self {
            Self::Auto => AutoOr::Auto,
            Self::Value(v) => AutoOr::Value(f(v)),
        }
    }
}

impl<T> Default for AutoOr<T> {
    fn default() -> Self {
        Self::Auto
    }
}

impl<T> From<crate::Auto> for AutoOr<T> {
    fn from(_: crate::Auto) -> Self {
        Self::Auto
    }
}

impl From<i32> for AutoOr<i32> {
    fn from(v: i32) -> Self { Self::Value(v) }
}

impl From<f32> for AutoOr<f32> {
    fn from(v: f32) -> Self { Self::Value(v) }
}

impl From<u32> for AutoOr<u32> {
    fn from(v: u32) -> Self { Self::Value(v) }
}

/// CSS value: `none | <T>`.
/// Used by perspective, marker-*, font-size-adjust, etc.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum NoneOr<T> {
    None,
    Value(T),
}

impl<T> NoneOr<T> {
    /// Returns `true` if this is `None`.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns the inner value, or `None` if `None`.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            Self::None => None,
        }
    }

    /// Transforms the inner value, preserving `None`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> NoneOr<U> {
        match self {
            Self::None => NoneOr::None,
            Self::Value(v) => NoneOr::Value(f(v)),
        }
    }
}

impl<T> Default for NoneOr<T> {
    fn default() -> Self {
        Self::None
    }
}

impl<T> From<crate::CssNone> for NoneOr<T> {
    fn from(_: crate::CssNone) -> Self {
        Self::None
    }
}

/// CSS value: `normal | <T>`.
/// Used by line-height, letter-spacing, word-spacing.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum NormalOr<T> {
    Normal,
    Value(T),
}

impl<T> NormalOr<T> {
    /// Returns `true` if this is `Normal`.
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Returns the inner value, or `None` if `Normal`.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            Self::Normal => None,
        }
    }

    /// Transforms the inner value, preserving `Normal`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> NormalOr<U> {
        match self {
            Self::Normal => NormalOr::Normal,
            Self::Value(v) => NormalOr::Value(f(v)),
        }
    }
}

impl<T> Default for NormalOr<T> {
    fn default() -> Self {
        Self::Normal
    }
}

impl<T> From<crate::Normal> for NormalOr<T> {
    fn from(_: crate::Normal) -> Self {
        Self::Normal
    }
}

/// CSS `contain-intrinsic-size`: `none | <length> | auto <length>`.
#[derive(Clone, Debug, PartialEq, Default, ToComputedValue)]
pub enum ContainIntrinsicSize {
    #[default]
    None,
    Length(crate::specified::LengthPercentage),
    AutoLength(crate::specified::LengthPercentage),
}

/// Four-sided box values (margin, padding, border-width, etc.).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Default> Default for Edges<T> {
    fn default() -> Self {
        Self {
            top: T::default(),
            right: T::default(),
            bottom: T::default(),
            left: T::default(),
        }
    }
}

impl<T: Clone> Edges<T> {
    /// Creates edges with the same value on all four sides.
    pub fn all(v: T) -> Self {
        Self {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }

    /// Creates edges with symmetric vertical and horizontal values.
    pub fn symmetric(vertical: T, horizontal: T) -> Self {
        Self {
            top: vertical.clone(),
            bottom: vertical,
            left: horizontal.clone(),
            right: horizontal,
        }
    }
}
