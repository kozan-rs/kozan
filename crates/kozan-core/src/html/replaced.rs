//! Replaced element trait — elements with external/intrinsic content.
//!
//! Chrome equivalent: `LayoutReplaced` (layout side) +
//! `HTMLImageElement::GetNaturalDimensions` (DOM side).
//!
//! A "replaced element" is one whose content comes from outside the CSS
//! formatting model: images, video frames, canvas pixels, embedded documents.
//! They have **intrinsic dimensions** (natural width, height, aspect ratio)
//! that layout uses when no explicit CSS size is set.
//!
//! # Chrome hierarchy
//!
//! ```text
//! LayoutObject → LayoutBox → LayoutReplaced
//!                               ├── LayoutImage      (img, CSS images)
//!                               ├── LayoutVideo      (video)
//!                               ├── LayoutIFrame     (iframe)
//!                               └── LayoutSVGRoot    (svg)
//! ```
//!
//! # Kozan approach
//!
//! The `ReplacedElement` trait lives on the DOM side (Element level).
//! During layout, `DocumentLayoutView` checks if a node is replaced
//! and uses `intrinsic_sizing()` for leaf measurement.

use super::html_element::HtmlElement;

/// Intrinsic sizing information for a replaced element.
///
/// Chrome equivalent: `IntrinsicSizingInfo` / `PhysicalNaturalSizingInfo`.
/// Returned by `ReplacedElement::intrinsic_sizing()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct IntrinsicSizing {
    /// Natural width in CSS pixels. `None` if unknown (e.g., image not loaded).
    pub width: Option<f32>,
    /// Natural height in CSS pixels. `None` if unknown.
    pub height: Option<f32>,
    /// Natural aspect ratio (width / height). Derived from width/height if both
    /// are known, or set explicitly (e.g., CSS `aspect-ratio`).
    pub aspect_ratio: Option<f32>,
}

impl IntrinsicSizing {
    /// Create sizing with explicit width and height.
    #[must_use] 
    pub fn from_size(width: f32, height: f32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
            aspect_ratio: if height > 0.0 {
                Some(width / height)
            } else {
                None
            },
        }
    }

    /// Create sizing with only an aspect ratio (e.g., responsive video).
    #[must_use] 
    pub fn from_ratio(ratio: f32) -> Self {
        Self {
            width: None,
            height: None,
            aspect_ratio: Some(ratio),
        }
    }
}

/// Shared behavior for replaced elements (img, video, canvas, iframe, svg).
///
/// Chrome equivalent: the virtual `GetNaturalDimensions()` on `LayoutReplaced`.
///
/// # Implementors
///
/// - `HtmlImageElement` — intrinsic size from decoded image
/// - `HtmlVideoElement` — intrinsic size from video dimensions
/// - `HtmlCanvasElement` — intrinsic size from canvas width/height attributes
///
/// # How it connects to layout
///
/// During layout, `DocumentLayoutView` checks `item_is_replaced` on the
/// Taffy style and reads `intrinsic_sizing()` for natural dimensions.
pub trait ReplacedElement: HtmlElement {
    /// Get the element's intrinsic (natural) dimensions.
    ///
    /// Chrome equivalent: `LayoutReplaced::GetNaturalDimensions()`.
    ///
    /// Returns the current intrinsic size. May change when the external
    /// resource loads (image decode complete, video metadata received).
    fn intrinsic_sizing(&self) -> IntrinsicSizing;

    /// Notify that intrinsic dimensions have changed.
    ///
    /// Chrome equivalent: `LayoutReplaced::NaturalSizeChanged()`.
    /// Called when the underlying resource changes size (e.g., image loaded,
    /// video resolution changed). Invalidates layout.
    fn notify_intrinsic_size_changed(&self) {
        // Default: mark layout dirty via handle.
        // Future: self.handle().mark_layout_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_sizing_from_size() {
        let sizing = IntrinsicSizing::from_size(800.0, 600.0);
        assert_eq!(sizing.width, Some(800.0));
        assert_eq!(sizing.height, Some(600.0));
        assert!((sizing.aspect_ratio.unwrap() - 1.333).abs() < 0.01);
    }

    #[test]
    fn intrinsic_sizing_from_ratio() {
        let sizing = IntrinsicSizing::from_ratio(16.0 / 9.0);
        assert!(sizing.width.is_none());
        assert!(sizing.height.is_none());
        assert!((sizing.aspect_ratio.unwrap() - 1.777).abs() < 0.01);
    }

    #[test]
    fn intrinsic_sizing_default() {
        let sizing = IntrinsicSizing::default();
        assert!(sizing.width.is_none());
        assert!(sizing.height.is_none());
        assert!(sizing.aspect_ratio.is_none());
    }

    #[test]
    fn zero_height_no_ratio() {
        let sizing = IntrinsicSizing::from_size(100.0, 0.0);
        assert!(sizing.aspect_ratio.is_none());
    }
}
