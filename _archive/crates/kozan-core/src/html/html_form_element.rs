//! `HTMLFormElement` — a form container.
//!
//! Chrome equivalent: `HTMLFormElement`.
//! Groups form controls for submission.

use crate::Handle;
use kozan_macros::{Element, Props};

/// A form element (`<form>`).
///
/// Chrome equivalent: `HTMLFormElement`.
#[derive(Copy, Clone, Element)]
#[element(tag = "form", data = FormData)]
pub struct HtmlFormElement(Handle);

/// Element-specific data for `<form>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlFormElement)]
pub struct FormData {
    /// The URL to submit the form to.
    #[prop]
    pub action: String,
    /// The HTTP method ("get" or "post").
    #[prop]
    pub method: String,
}
