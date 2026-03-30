//! CSS declared values — typed wrapper for property values.
//!
//! `Declared<T>` wraps any CSS property value before cascade resolution.
//! It handles var() references and CSS-wide keywords (inherit, initial, etc.).
//!
//! Calc expressions live INSIDE the value type itself (e.g. inside
//! `specified::LengthPercentage::Calc`), not in `Declared`.
//!
//! ```ignore
//! use kozan_style::*;
//!
//! style.width(px(100.0))                // Length → Size::LP → Declared::Value
//! style.width(em(2.0))                  // FontRelative → Size::LP → Declared::Value
//! style.width(auto())                   // → Declared::Value(Size::Auto)
//! style.width(var("gap"))               // → Declared::Var
//! style.width(inherit())                // → Declared::Inherit
//! style.z_index(5)                      // i32 → Declared::Value
//! style.color(Color::rgb(255, 0, 0))    // → Declared::Value
//! ```

/// CSS-wide keyword discriminant — shared between parsing (kozan-css) and
/// code generation (kozan-style). Avoids magic numbers crossing crate boundaries.
///
/// Maps 1:1 to the keyword variants of `Declared<T>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CssWideKeyword {
    Initial = 0,
    Inherit = 1,
    Unset = 2,
    Revert = 3,
    RevertLayer = 4,
}

/// What a CSS property holds before cascade resolution.
#[derive(Clone, Debug, PartialEq)]
pub enum Declared<T> {
    /// Concrete typed value (no substitution functions).
    Value(T),
    /// Raw CSS text containing `var()`, `env()`, or `attr()`.
    /// Stored when the parser encounters substitution functions anywhere
    /// in the value. Cascade substitutes them, then re-parses to `Value(T)`.
    WithVariables(UnparsedValue),
    /// `inherit` — copy from parent's computed value.
    Inherit,
    /// `initial` — use the property's CSS spec initial value.
    Initial,
    /// `unset` — inherit if inherited, initial otherwise.
    Unset,
    /// `revert` — use value from previous cascade origin.
    Revert,
    /// `revert-layer` — use value from previous cascade layer.
    RevertLayer,
}

/// Unparsed property value containing substitution function references.
///
/// Stored as raw CSS text. The cascade substitutes `var()`/`env()`/`attr()`
/// in the string, then re-parses the result as the target property type.
///
/// Uses `triomphe::Arc<str>` — no weak count (8 bytes overhead), clone is
/// a refcount bump (O(1)), and the stylesheet source can be dropped after parsing.
#[derive(Clone, Debug, PartialEq)]
pub struct UnparsedValue {
    /// Raw CSS text (e.g. `"calc(var(--z) + 1)"`, `"env(safe-area-inset-top)"`).
    pub css: crate::Atom,
    /// Which substitution functions are present (for fast cascade skipping).
    pub references: SubstitutionRefs,
}

/// Bitflags for which substitution functions appear in an [`UnparsedValue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubstitutionRefs(u8);

impl SubstitutionRefs {
    /// Contains `var()`.
    pub const VAR: Self = Self(0b001);
    /// Contains `env()`.
    pub const ENV: Self = Self(0b010);
    /// Contains `attr()`.
    pub const ATTR: Self = Self(0b100);

    /// No substitution functions.
    pub const fn empty() -> Self { Self(0) }

    /// Returns true if no substitution functions are referenced.
    pub const fn is_empty(self) -> bool { self.0 == 0 }

    /// Returns true if `other` flags are all set in self.
    pub const fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
}

impl core::ops::BitOr for SubstitutionRefs {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}

impl<T> Declared<T> {
    /// Returns the inner value if this is a concrete `Value`, or `None`.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }

    /// Consumes self, returning the inner value if concrete.
    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }

    /// Returns `true` if this contains substitution functions.
    pub fn has_variables(&self) -> bool { matches!(self, Self::WithVariables(_)) }

    /// Returns `true` if this is a CSS-wide keyword.
    pub fn is_keyword(&self) -> bool {
        matches!(self, Self::Inherit | Self::Initial | Self::Unset | Self::Revert | Self::RevertLayer)
    }

    /// Transforms the inner value, preserving keywords and unparsed values.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Declared<U> {
        match self {
            Self::Value(v) => Declared::Value(f(v)),
            Self::WithVariables(u) => Declared::WithVariables(u),
            Self::Inherit => Declared::Inherit,
            Self::Initial => Declared::Initial,
            Self::Unset => Declared::Unset,
            Self::Revert => Declared::Revert,
            Self::RevertLayer => Declared::RevertLayer,
        }
    }
}
/// Conversion trait for style builder arguments.
///
/// Custom trait (not Into) to avoid Rust's transitive From conflicts.
/// Each concrete type gets an explicit bridge impl.
pub trait IntoDeclared<T> {
    /// Converts this value into a `Declared<T>` for the style builder.
    fn into_declared(self) -> Declared<T>;
}

// Declared<T> itself
impl<T> IntoDeclared<T> for Declared<T> {
    fn into_declared(self) -> Declared<T> { self }
}

// UnparsedValue → Declared::WithVariables for any T
impl<T> IntoDeclared<T> for UnparsedValue {
    fn into_declared(self) -> Declared<T> { Declared::WithVariables(self) }
}
impl IntoDeclared<crate::Color> for crate::Color {
    fn into_declared(self) -> Declared<crate::Color> { Declared::Value(self) }
}
impl IntoDeclared<crate::FontWeight> for crate::FontWeight {
    fn into_declared(self) -> Declared<crate::FontWeight> { Declared::Value(self) }
}
impl IntoDeclared<crate::CornerRadius> for crate::CornerRadius {
    fn into_declared(self) -> Declared<crate::CornerRadius> { Declared::Value(self) }
}
impl IntoDeclared<f32> for f32 {
    fn into_declared(self) -> Declared<f32> { Declared::Value(self) }
}
impl IntoDeclared<i32> for i32 {
    fn into_declared(self) -> Declared<i32> { Declared::Value(self) }
}
impl IntoDeclared<u32> for u32 {
    fn into_declared(self) -> Declared<u32> { Declared::Value(self) }
}
impl IntoDeclared<u16> for u16 {
    fn into_declared(self) -> Declared<u16> { Declared::Value(self) }
}

// New types
impl IntoDeclared<crate::specified::LengthPercentage> for crate::specified::LengthPercentage {
    fn into_declared(self) -> Declared<crate::specified::LengthPercentage> { Declared::Value(self) }
}
impl IntoDeclared<crate::specified::LengthPercentage> for crate::specified::Length {
    fn into_declared(self) -> Declared<crate::specified::LengthPercentage> {
        Declared::Value(crate::specified::LengthPercentage::from(self))
    }
}
impl IntoDeclared<crate::specified::LengthPercentage> for crate::computed::Percentage {
    fn into_declared(self) -> Declared<crate::specified::LengthPercentage> {
        Declared::Value(crate::specified::LengthPercentage::from(self))
    }
}

// Size<LP> bridges
impl<LP> IntoDeclared<crate::generics::Size<LP>> for crate::generics::Size<LP> {
    fn into_declared(self) -> Declared<crate::generics::Size<LP>> { Declared::Value(self) }
}
impl IntoDeclared<crate::generics::Size<crate::specified::LengthPercentage>> for crate::specified::Length {
    fn into_declared(self) -> Declared<crate::generics::Size<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::Size::LengthPercentage(
            crate::specified::LengthPercentage::from(self)
        ))
    }
}
impl IntoDeclared<crate::generics::Size<crate::specified::LengthPercentage>> for crate::computed::Percentage {
    fn into_declared(self) -> Declared<crate::generics::Size<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::Size::LengthPercentage(
            crate::specified::LengthPercentage::from(self)
        ))
    }
}
impl IntoDeclared<crate::generics::Size<crate::specified::LengthPercentage>> for crate::specified::LengthPercentage {
    fn into_declared(self) -> Declared<crate::generics::Size<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::Size::LengthPercentage(self))
    }
}

// MaxSize<LP> bridges
impl<LP> IntoDeclared<crate::generics::MaxSize<LP>> for crate::generics::MaxSize<LP> {
    fn into_declared(self) -> Declared<crate::generics::MaxSize<LP>> { Declared::Value(self) }
}
impl IntoDeclared<crate::generics::MaxSize<crate::specified::LengthPercentage>> for crate::specified::Length {
    fn into_declared(self) -> Declared<crate::generics::MaxSize<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::MaxSize::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::MaxSize<crate::specified::LengthPercentage>> for crate::computed::Percentage {
    fn into_declared(self) -> Declared<crate::generics::MaxSize<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::MaxSize::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::MaxSize<crate::specified::LengthPercentage>> for crate::specified::LengthPercentage {
    fn into_declared(self) -> Declared<crate::generics::MaxSize<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::MaxSize::LengthPercentage(self))
    }
}

// Margin<LP> bridges
impl<LP> IntoDeclared<crate::generics::Margin<LP>> for crate::generics::Margin<LP> {
    fn into_declared(self) -> Declared<crate::generics::Margin<LP>> { Declared::Value(self) }
}
impl IntoDeclared<crate::generics::Margin<crate::specified::LengthPercentage>> for crate::specified::Length {
    fn into_declared(self) -> Declared<crate::generics::Margin<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::Margin::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::Margin<crate::specified::LengthPercentage>> for crate::computed::Percentage {
    fn into_declared(self) -> Declared<crate::generics::Margin<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::Margin::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::Margin<crate::specified::LengthPercentage>> for crate::specified::LengthPercentage {
    fn into_declared(self) -> Declared<crate::generics::Margin<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::Margin::LengthPercentage(self))
    }
}

// LPOrAuto bridges
impl<LP> IntoDeclared<crate::generics::LengthPercentageOrAuto<LP>> for crate::generics::LengthPercentageOrAuto<LP> {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrAuto<LP>> { Declared::Value(self) }
}
impl IntoDeclared<crate::generics::LengthPercentageOrAuto<crate::specified::LengthPercentage>> for crate::specified::Length {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrAuto<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::LengthPercentageOrAuto::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::LengthPercentageOrAuto<crate::specified::LengthPercentage>> for crate::computed::Percentage {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrAuto<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::LengthPercentageOrAuto::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::LengthPercentageOrAuto<crate::specified::LengthPercentage>> for crate::specified::LengthPercentage {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrAuto<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::LengthPercentageOrAuto::LengthPercentage(self))
    }
}

// LPOrNormal bridges
impl<LP> IntoDeclared<crate::generics::LengthPercentageOrNormal<LP>> for crate::generics::LengthPercentageOrNormal<LP> {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrNormal<LP>> { Declared::Value(self) }
}
impl IntoDeclared<crate::generics::LengthPercentageOrNormal<crate::specified::LengthPercentage>> for crate::specified::Length {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrNormal<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::LengthPercentageOrNormal::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::LengthPercentageOrNormal<crate::specified::LengthPercentage>> for crate::computed::Percentage {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrNormal<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::LengthPercentageOrNormal::LengthPercentage(crate::specified::LengthPercentage::from(self)))
    }
}
impl IntoDeclared<crate::generics::LengthPercentageOrNormal<crate::specified::LengthPercentage>> for crate::specified::LengthPercentage {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrNormal<crate::specified::LengthPercentage>> {
        Declared::Value(crate::generics::LengthPercentageOrNormal::LengthPercentage(self))
    }
}

// Generic wrappers: identity impls
impl<T> IntoDeclared<crate::AutoOr<T>> for crate::AutoOr<T> {
    fn into_declared(self) -> Declared<crate::AutoOr<T>> { Declared::Value(self) }
}
impl<T> IntoDeclared<crate::NoneOr<T>> for crate::NoneOr<T> {
    fn into_declared(self) -> Declared<crate::NoneOr<T>> { Declared::Value(self) }
}
impl<T> IntoDeclared<crate::NormalOr<T>> for crate::NormalOr<T> {
    fn into_declared(self) -> Declared<crate::NormalOr<T>> { Declared::Value(self) }
}
/// Marker: `inherit()`.
pub struct Inherit;
/// Creates an `inherit` CSS-wide keyword value.
pub fn inherit() -> Inherit { Inherit }

/// Marker: `initial()`.
pub struct Initial;
/// Creates an `initial` CSS-wide keyword value.
pub fn initial() -> Initial { Initial }

/// Marker: `unset()`.
pub struct Unset;
/// Creates an `unset` CSS-wide keyword value.
pub fn unset() -> Unset { Unset }

/// Marker: `revert()`.
pub struct Revert;
/// Creates a `revert` CSS-wide keyword value.
pub fn revert() -> Revert { Revert }

/// Marker: `revert_layer()`.
pub struct RevertLayer;
/// Creates a `revert-layer` CSS-wide keyword value.
pub fn revert_layer() -> RevertLayer { RevertLayer }

impl<T> IntoDeclared<T> for Inherit {
    fn into_declared(self) -> Declared<T> { Declared::Inherit }
}
impl<T> IntoDeclared<T> for Initial {
    fn into_declared(self) -> Declared<T> { Declared::Initial }
}
impl<T> IntoDeclared<T> for Unset {
    fn into_declared(self) -> Declared<T> { Declared::Unset }
}
impl<T> IntoDeclared<T> for Revert {
    fn into_declared(self) -> Declared<T> { Declared::Revert }
}
impl<T> IntoDeclared<T> for RevertLayer {
    fn into_declared(self) -> Declared<T> { Declared::RevertLayer }
}
/// Marker for CSS `auto` keyword.
pub struct Auto;
/// Creates a CSS `auto` keyword value.
pub fn auto() -> Auto { Auto }

/// Marker for CSS `none` keyword.
pub struct CssNone;
/// Creates a CSS `none` keyword value.
pub fn css_none() -> CssNone { CssNone }

/// Marker for CSS `normal` keyword.
pub struct Normal;
/// Creates a CSS `normal` keyword value.
pub fn normal() -> Normal { Normal }

// Auto → Size::Auto
impl<LP> IntoDeclared<crate::generics::Size<LP>> for Auto {
    fn into_declared(self) -> Declared<crate::generics::Size<LP>> {
        Declared::Value(crate::generics::Size::Auto)
    }
}

// CssNone → MaxSize::None
impl<LP> IntoDeclared<crate::generics::MaxSize<LP>> for CssNone {
    fn into_declared(self) -> Declared<crate::generics::MaxSize<LP>> {
        Declared::Value(crate::generics::MaxSize::None)
    }
}

// Auto → Margin::Auto
impl<LP> IntoDeclared<crate::generics::Margin<LP>> for Auto {
    fn into_declared(self) -> Declared<crate::generics::Margin<LP>> {
        Declared::Value(crate::generics::Margin::Auto)
    }
}

// Auto → LPOrAuto::Auto
impl<LP> IntoDeclared<crate::generics::LengthPercentageOrAuto<LP>> for Auto {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrAuto<LP>> {
        Declared::Value(crate::generics::LengthPercentageOrAuto::Auto)
    }
}

// Normal → LPOrNormal::Normal
impl<LP> IntoDeclared<crate::generics::LengthPercentageOrNormal<LP>> for Normal {
    fn into_declared(self) -> Declared<crate::generics::LengthPercentageOrNormal<LP>> {
        Declared::Value(crate::generics::LengthPercentageOrNormal::Normal)
    }
}

// Auto/None/Normal for AutoOr/NoneOr/NormalOr (legacy wrappers)
impl<T> IntoDeclared<crate::AutoOr<T>> for Auto {
    fn into_declared(self) -> Declared<crate::AutoOr<T>> {
        Declared::Value(crate::AutoOr::Auto)
    }
}
impl<T> IntoDeclared<crate::NoneOr<T>> for CssNone {
    fn into_declared(self) -> Declared<crate::NoneOr<T>> {
        Declared::Value(crate::NoneOr::None)
    }
}
impl<T> IntoDeclared<crate::NormalOr<T>> for Normal {
    fn into_declared(self) -> Declared<crate::NormalOr<T>> {
        Declared::Value(crate::NormalOr::Normal)
    }
}

impl IntoDeclared<crate::AutoOr<i32>> for i32 {
    fn into_declared(self) -> Declared<crate::AutoOr<i32>> {
        Declared::Value(crate::AutoOr::Value(self))
    }
}
impl IntoDeclared<crate::AutoOr<f32>> for f32 {
    fn into_declared(self) -> Declared<crate::AutoOr<f32>> {
        Declared::Value(crate::AutoOr::Value(self))
    }
}
impl IntoDeclared<crate::AutoOr<u32>> for u32 {
    fn into_declared(self) -> Declared<crate::AutoOr<u32>> {
        Declared::Value(crate::AutoOr::Value(self))
    }
}

impl IntoDeclared<crate::Color> for crate::AbsoluteColor {
    fn into_declared(self) -> Declared<crate::Color> {
        Declared::Value(crate::Color::Absolute(self))
    }
}

impl IntoDeclared<crate::Color> for crate::SystemColor {
    fn into_declared(self) -> Declared<crate::Color> {
        Declared::Value(crate::Color::System(self))
    }
}

impl IntoDeclared<crate::FontWeight> for u16 {
    fn into_declared(self) -> Declared<crate::FontWeight> {
        Declared::Value(crate::FontWeight::from(self))
    }
}

impl IntoDeclared<crate::CornerRadius> for f32 {
    fn into_declared(self) -> Declared<crate::CornerRadius> {
        Declared::Value(crate::CornerRadius::from(self))
    }
}
