//! `HTMLLabelElement` — a label for a form control.
//!
//! Chrome equivalent: `HTMLLabelElement`.
//! Associates with a form control via the `for` attribute.

use crate::Handle;
use kozan_macros::{Element, Props};

/// A label element (`<label>`).
///
/// Chrome equivalent: `HTMLLabelElement`.
/// Clicking a label activates its associated control.
#[derive(Copy, Clone, Element)]
#[element(tag = "label", data = LabelData)]
pub struct HtmlLabelElement(Handle);

/// Element-specific data for `<label>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlLabelElement)]
pub struct LabelData {
    /// The ID of the labeled control (`for` attribute).
    /// Named `html_for` because `for` is a Rust keyword.
    #[prop]
    pub html_for: String,
}
