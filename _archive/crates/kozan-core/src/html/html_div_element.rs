//! `HTMLDivElement` — a generic container element.

use kozan_macros::Element;

use crate::Handle;

/// A generic container element (`<div>`).
#[derive(Copy, Clone, Element)]
#[element(tag = "div")]
pub struct HtmlDivElement(Handle);
