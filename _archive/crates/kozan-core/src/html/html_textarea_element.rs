//! `HTMLTextAreaElement` — a multi-line text editing control.
//!
//! Chrome equivalent: `HTMLTextAreaElement`.

use super::form_control::{FormControlElement, TextControlElement};
use crate::Handle;
use kozan_macros::{Element, Props};

/// A multi-line text editing control (`<textarea>`).
///
/// Chrome equivalent: `HTMLTextAreaElement`.
#[derive(Copy, Clone, Element)]
#[element(tag = "textarea", focusable, data = TextAreaData)]
pub struct HtmlTextAreaElement(Handle);

/// Element-specific data for `<textarea>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlTextAreaElement)]
#[non_exhaustive]
pub struct TextAreaData {
    /// The visible width in average character widths.
    #[prop]
    pub cols: u32,
    /// The visible number of text lines.
    #[prop]
    pub rows: u32,
    /// How the text wraps: "soft" (default) or "hard".
    #[prop]
    pub wrap: String,
}

impl FormControlElement for HtmlTextAreaElement {}
impl TextControlElement for HtmlTextAreaElement {}
