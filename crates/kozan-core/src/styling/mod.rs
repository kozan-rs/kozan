//! Styling system — powered by Mozilla's Stylo CSS engine.
//!
//! This is the ONLY module that imports Stylo types. All other code
//! (dom, layout, paint, public API) uses re-exports from here.

pub mod builder;
pub(crate) mod data;
pub(crate) mod engine;
pub(crate) mod font_metrics;
pub(crate) mod node;
pub mod taffy_bridge;
pub(crate) mod traits;
pub(crate) mod traversal;
pub mod units;

#[cfg(test)]
mod tests;

// ── Internal exports (used by dom) ──

pub(crate) use engine::StyleEngine;

// ═══════════════════════════════════════════════════════════════════
// Public re-exports from Stylo
// ═══════════════════════════════════════════════════════════════════

/// Computed style values for an element. All CSS properties accessible via
/// property struct getters (e.g. `cv.get_box().clone_display()`).
pub use style::properties::ComputedValues;

/// Stylo color type — used by `bg()`, `border_color()`, etc.
pub use style::color::AbsoluteColor;

/// CSS border-style keyword — `Solid`, `Dashed`, `Dotted`, `None`, etc.
pub use style::values::specified::border::BorderStyle;

/// A parsed CSS declaration block (inline styles, stylesheet rules).
pub use style::properties::PropertyDeclarationBlock;

/// Arc wrapper used by Stylo.
pub use servo_arc::Arc;

/// Returns initial (default) computed values wrapped in Arc.
/// Stylo equivalent of the old `ComputedStyle::default()`.
/// Uses a static cache to avoid repeated allocation.
pub fn initial_values_arc() -> servo_arc::Arc<ComputedValues> {
    use std::sync::OnceLock;
    static INITIAL: OnceLock<servo_arc::Arc<ComputedValues>> = OnceLock::new();
    INITIAL
        .get_or_init(|| {
            ComputedValues::initial_values_with_font_override(
                style::properties::style_structs::Font::initial_values(),
            )
        })
        .clone()
}

/// Stylo's shared lock — needed to read locked declarations.
pub use style::shared_lock::{SharedRwLock, SharedRwLockReadGuard};

/// Property struct groups — access computed values by category.
/// Usage: `computed_values.get_box().clone_display()`
pub use style::properties::style_structs;

/// Computed value types that exist as standalone re-exports.
pub mod values {
    // Box model
    pub use style::values::computed::Clear;
    pub use style::values::computed::Display;
    pub use style::values::computed::Float;
    pub use style::values::computed::Overflow;

    // Position
    pub use style::values::computed::PositionProperty as Position;

    // Flex alignment
    pub use style::values::computed::ContentDistribution;
    pub use style::values::computed::FlexBasis;
    pub use style::values::computed::JustifyItems;
    pub use style::values::computed::SelfAlignment;

    // Size + spacing
    pub use style::values::computed::CSSPixelLength;
    pub use style::values::computed::Length;
    pub use style::values::computed::LengthPercentage;
    pub use style::values::computed::LengthPercentageOrAuto;
    pub use style::values::computed::Margin;
    pub use style::values::computed::MaxSize;
    pub use style::values::computed::NonNegativeLength;
    pub use style::values::computed::NonNegativeLengthPercentage;
    pub use style::values::computed::Size;

    // Text + font
    pub use style::values::computed::FontFamily;
    pub use style::values::computed::FontStyle;
    pub use style::values::computed::FontWeight;
    pub use style::values::computed::LineHeight;
    pub use style::values::computed::Opacity;
    pub use style::values::computed::TextAlign;
    pub use style::values::computed::TextDecorationLine;

    // Border
    pub use style::values::computed::BorderStyle;

    // Color
    pub use style::color::AbsoluteColor as Color;

    // Grid
    pub use style::values::computed::GridAutoFlow;

    // Transform
    pub use style::values::computed::Transform;
}
