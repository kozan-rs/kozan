//! `HTMLSpanElement` — a generic inline container.
//!
//! Chrome equivalent: `HTMLSpanElement`. No additions over `HTMLElement`.

use crate::Handle;
use kozan_macros::Element;

/// A generic inline container element (`<span>`).
///
/// Like `<div>` but inline-level. No extra behavior.
#[derive(Copy, Clone, Element)]
#[element(tag = "span")]
pub struct HtmlSpanElement(Handle);
