//! `HTMLImageElement` — an image element.
//!
//! Chrome equivalent: `HTMLImageElement` (DOM) + `LayoutImage` (layout).
//! A replaced element — its content is an external image resource.
//!
//! # Intrinsic sizing
//!
//! Before the image loads, intrinsic size is unknown (returns None).
//! After load, the natural width/height from the decoded image.
//! The layout system uses these for CSS sizing resolution.

use super::replaced::{IntrinsicSizing, ReplacedElement};
use crate::Handle;
use kozan_macros::{Element, Props};

/// An image element (`<img>`).
///
/// Chrome equivalent: `HTMLImageElement`.
/// Replaced element — content from external image resource.
#[derive(Copy, Clone, Element)]
#[element(tag = "img", data = ImageData)]
pub struct HtmlImageElement(Handle);

/// Element-specific data for `<img>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlImageElement)]
#[non_exhaustive]
pub struct ImageData {
    /// The image URL.
    #[prop]
    pub src: String,
    /// Alternative text for accessibility.
    #[prop]
    pub alt: String,
    /// The intrinsic width in CSS pixels (from decoded image or attribute).
    #[prop]
    pub natural_width: f32,
    /// The intrinsic height in CSS pixels (from decoded image or attribute).
    #[prop]
    pub natural_height: f32,
}

impl ReplacedElement for HtmlImageElement {
    fn intrinsic_sizing(&self) -> IntrinsicSizing {
        let w = self.natural_width();
        let h = self.natural_height();

        if w > 0.0 && h > 0.0 {
            IntrinsicSizing::from_size(w, h)
        } else {
            // Image not loaded yet — no intrinsic size.
            IntrinsicSizing::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;

    #[test]
    fn image_src_and_alt() {
        let doc = Document::new();
        let img = doc.create::<HtmlImageElement>();

        img.set_src("photo.jpg");
        img.set_alt("A photo");

        assert_eq!(img.src(), "photo.jpg");
        assert_eq!(img.alt(), "A photo");
    }

    #[test]
    fn image_intrinsic_sizing() {
        let doc = Document::new();
        let img = doc.create::<HtmlImageElement>();

        // No size yet.
        let sizing = img.intrinsic_sizing();
        assert!(sizing.width.is_none());

        // Simulate image load.
        img.set_natural_width(800.0);
        img.set_natural_height(600.0);

        let sizing = img.intrinsic_sizing();
        assert_eq!(sizing.width, Some(800.0));
        assert_eq!(sizing.height, Some(600.0));
        assert!((sizing.aspect_ratio.unwrap() - 1.333).abs() < 0.01);
    }
}
