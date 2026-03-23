//! `HTMLAnchorElement` — a hyperlink element.
//!
//! Chrome equivalent: `HTMLAnchorElement`.
//! Adds `href`, `target`, `rel` attributes.

use crate::Handle;
use kozan_macros::{Element, Props};

/// A hyperlink element (`<a>`).
///
/// Chrome equivalent: `HTMLAnchorElement`.
#[derive(Copy, Clone, Element)]
#[element(tag = "a", data = AnchorData)]
pub struct HtmlAnchorElement(Handle);

/// Element-specific data for anchor elements.
#[derive(Default, Clone, Props)]
#[props(element = HtmlAnchorElement)]
pub struct AnchorData {
    /// The hyperlink URL.
    #[prop]
    pub href: String,
    /// The browsing context for navigation ("_blank", "_self", etc.).
    #[prop]
    pub target: String,
    /// The relationship of the linked resource ("noopener", "noreferrer", etc.).
    #[prop]
    pub rel: String,
}
