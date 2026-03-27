//! CSS unit helpers — `px(100.0)`, `pct(50.0)`, `em(1.5)`, etc.
//!
//! These create Stylo specified types directly. No CSS parsing needed.
//!
//! ```ignore
//! use kozan::prelude::*;
//!
//! div.style().width(px(200.0));
//! div.style().height(pct(100.0));
//! div.style().margin_top(em(1.5));
//! ```

use style::values::generics::NonNegative;
use style::values::generics::length::{GenericMargin, GenericSize};
use style::values::specified::length::{AbsoluteLength, FontRelativeLength, NoCalcLength};
use style::values::specified::{self, LengthPercentage, NonNegativeLengthPercentage};

/// A CSS dimension value that can be used in any property context.
///
/// This is the user-facing type. Implements `Into<Size>`, `Into<Margin>`,
/// `Into<NonNegativeLengthPercentage>` so it can be passed to any
/// `StyleAccess` method.
#[derive(Debug, Clone, Copy)]
pub enum CssValue {
    /// A pixel value (e.g., `200px`).
    Px(f32),
    /// A percentage (e.g., `50%`).
    Pct(f32),
    /// An em value (e.g., `1.5em`).
    Em(f32),
    /// A rem value (e.g., `2rem`).
    Rem(f32),
    /// A viewport-width value (e.g., `100vw`).
    Vw(f32),
    /// A viewport-height value (e.g., `100vh`).
    Vh(f32),
    /// The `auto` keyword.
    Auto,
}

// ---- Constructor functions ----

/// Create a pixel value: `px(200.0)` → `200px`.
#[must_use]
pub fn px(v: f32) -> CssValue {
    CssValue::Px(v)
}

/// Create a percentage: `pct(50.0)` → `50%`.
#[must_use]
pub fn pct(v: f32) -> CssValue {
    CssValue::Pct(v)
}

/// Create an em value: `em(1.5)` → `1.5em`.
#[must_use]
pub fn em(v: f32) -> CssValue {
    CssValue::Em(v)
}

/// Create a rem value: `rem(2.0)` → `2rem`.
#[must_use]
pub fn rem(v: f32) -> CssValue {
    CssValue::Rem(v)
}

/// Create a viewport-width value: `vw(100.0)` → `100vw`.
#[must_use]
pub fn vw(v: f32) -> CssValue {
    CssValue::Vw(v)
}

/// Create a viewport-height value: `vh(100.0)` → `100vh`.
#[must_use]
pub fn vh(v: f32) -> CssValue {
    CssValue::Vh(v)
}

/// The `auto` keyword.
#[must_use]
pub fn auto() -> CssValue {
    CssValue::Auto
}

// ---- Internal: convert to Stylo's LengthPercentage ----

fn to_length_percentage(v: CssValue) -> LengthPercentage {
    match v {
        CssValue::Px(n) => LengthPercentage::Length(NoCalcLength::Absolute(AbsoluteLength::Px(n))),
        CssValue::Pct(n) => {
            LengthPercentage::Percentage(style::values::computed::Percentage(n / 100.0))
        }
        CssValue::Em(n) => {
            LengthPercentage::Length(NoCalcLength::FontRelative(FontRelativeLength::Em(n)))
        }
        CssValue::Rem(n) => {
            LengthPercentage::Length(NoCalcLength::FontRelative(FontRelativeLength::Rem(n)))
        }
        CssValue::Vw(n) => LengthPercentage::Length(NoCalcLength::ViewportPercentage(
            style::values::specified::length::ViewportPercentageLength::Vw(n),
        )),
        CssValue::Vh(n) => LengthPercentage::Length(NoCalcLength::ViewportPercentage(
            style::values::specified::length::ViewportPercentageLength::Vh(n),
        )),
        CssValue::Auto => {
            // Auto doesn't map to LengthPercentage — fallback to 0px.
            LengthPercentage::Length(NoCalcLength::Absolute(AbsoluteLength::Px(0.0)))
        }
    }
}

// ---- Conversions to Stylo specified types ----

/// Into `Size` (for width/height): supports auto + length/percentage.
impl From<CssValue> for specified::Size {
    fn from(v: CssValue) -> Self {
        match v {
            CssValue::Auto => GenericSize::Auto,
            other => GenericSize::LengthPercentage(NonNegative(to_length_percentage(other))),
        }
    }
}

/// Into `Margin` (for margin-*): supports auto + length/percentage.
impl From<CssValue> for specified::Margin {
    fn from(v: CssValue) -> Self {
        match v {
            CssValue::Auto => GenericMargin::Auto,
            other => GenericMargin::LengthPercentage(to_length_percentage(other)),
        }
    }
}

/// Into `NonNegativeLengthPercentage` (for padding-*): no auto.
impl From<CssValue> for NonNegativeLengthPercentage {
    fn from(v: CssValue) -> Self {
        NonNegative(to_length_percentage(v))
    }
}

/// Wrapper for CSS inset values (top/right/bottom/left).
///
/// Accepts `px()`, `pct()`, `auto()` — same as margin properties.
pub struct InsetValue(pub(crate) style::values::specified::Inset);

impl From<CssValue> for InsetValue {
    fn from(v: CssValue) -> Self {
        use style::values::generics::position::Inset;
        match v {
            CssValue::Auto => InsetValue(Inset::Auto),
            other => InsetValue(Inset::LengthPercentage(to_length_percentage(other))),
        }
    }
}

// ---- Color helpers ----

use style::color::AbsoluteColor;

/// Create an sRGB color from f32 [0.0, 1.0]: `rgb(0.9, 0.3, 0.2)`.
#[must_use]
pub fn rgb(r: f32, g: f32, b: f32) -> AbsoluteColor {
    AbsoluteColor::new(style::color::ColorSpace::Srgb, r, g, b, 1.0)
}

/// Create an sRGB color with alpha: `rgba(0.9, 0.3, 0.2, 0.5)`.
#[must_use]
pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> AbsoluteColor {
    AbsoluteColor::new(style::color::ColorSpace::Srgb, r, g, b, a)
}

/// Create a color from 0-255 u8 values: `rgb8(232, 76, 61)`.
#[must_use]
pub fn rgb8(r: u8, g: u8, b: u8) -> AbsoluteColor {
    AbsoluteColor::srgb_legacy(r, g, b, 1.0)
}

/// Create a color from hex: `hex(0xE84C3D)`.
#[must_use]
pub fn hex(v: u32) -> AbsoluteColor {
    let r = ((v >> 16) & 0xFF) as u8;
    let g = ((v >> 8) & 0xFF) as u8;
    let b = (v & 0xFF) as u8;
    AbsoluteColor::srgb_legacy(r, g, b, 1.0)
}
