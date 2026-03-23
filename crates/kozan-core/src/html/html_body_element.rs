//! `HTMLBodyElement` — the document body.
//!
//! Chrome equivalent: `core/html/html_body_element.h`.
//! Default styles come from the UA stylesheet: `body { display: block; margin: 8px; }`.

use kozan_macros::Element;

use crate::Handle;

/// The document body element (`<body>`).
///
/// Created automatically by `Document::new()`. All user content
/// goes inside `doc.body()`, not `doc.root()`.
#[derive(Copy, Clone, Element)]
#[element(tag = "body")]
pub struct HtmlBodyElement(Handle);
