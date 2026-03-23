//! `HTMLHeadingElement` — heading elements h1 through h6.
//!
//! Chrome equivalent: `HTMLHeadingElement`. One class for all heading levels.
//! The tag name varies at runtime ("h1" through "h6"), controlled by
//! `Document::create_with_tag()`.
//!
//! # Usage
//!
//! ```ignore
//! // Convenience method (recommended):
//! let h2 = doc.create_heading(2);
//!
//! // Or explicit:
//! let h3 = doc.create_with_tag::<HtmlHeadingElement>("h3");
//! ```

use crate::Handle;
use crate::dom::traits::Element;
use kozan_macros::{Element, Props};

/// A heading element (`<h1>` through `<h6>`).
///
/// Chrome equivalent: `HTMLHeadingElement`.
/// One type for all 6 levels. The actual level is determined by the
/// runtime tag name, not the const `TAG_NAME`.
#[derive(Copy, Clone, Element)]
#[element(data = HeadingData, manual_html)]
pub struct HtmlHeadingElement(Handle);

/// Element-specific data for heading elements.
#[derive(Default, Clone, Props)]
#[props(element = HtmlHeadingElement)]
pub struct HeadingData {
    /// The heading level (1-6). Set automatically by `Document::create_heading()`.
    #[prop]
    pub level: u8,
}

impl HtmlHeadingElement {
    /// Get the heading level from the tag name.
    ///
    /// Returns 1-6 based on the actual runtime tag.
    /// Falls back to the stored `level` data.
    #[must_use] 
    pub fn heading_level(&self) -> u8 {
        match self.tag_name() {
            "h1" => 1,
            "h2" => 2,
            "h3" => 3,
            "h4" => 4,
            "h5" => 5,
            "h6" => 6,
            _ => self.level(),
        }
    }
}


#[cfg(test)]
mod tests {
    
    use crate::dom::document::Document;
    use crate::dom::traits::Element;

    #[test]
    fn heading_level_h1_through_h6() {
        let doc = Document::new();
        for level in 1..=6u8 {
            let h = doc.create_heading(level);
            assert_eq!(
                h.heading_level(),
                level,
                "heading_level() wrong for h{level}"
            );
        }
    }

    #[test]
    fn heading_tag_name_matches_level() {
        let doc = Document::new();
        let expected_tags = ["h1", "h2", "h3", "h4", "h5", "h6"];
        for (i, expected) in expected_tags.iter().enumerate() {
            let h = doc.create_heading((i as u8) + 1);
            assert_eq!(h.tag_name(), *expected);
        }
    }

    #[test]
    fn heading_level_data_prop() {
        let doc = Document::new();
        let h3 = doc.create_heading(3);
        // The level data prop is set by create_heading.
        assert_eq!(h3.level(), 3);
    }

}
