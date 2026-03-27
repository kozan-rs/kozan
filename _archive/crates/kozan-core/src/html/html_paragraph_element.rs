//! `HTMLParagraphElement` — a paragraph element.

use crate::Handle;
use kozan_macros::Element;

/// A paragraph element (`<p>`).
#[derive(Copy, Clone, Element)]
#[element(tag = "p")]
pub struct HtmlParagraphElement(Handle);
