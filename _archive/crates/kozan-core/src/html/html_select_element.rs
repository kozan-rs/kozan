//! `HTMLSelectElement` — a dropdown/listbox control.
//!
//! Chrome equivalent: `HTMLSelectElement`.
//! Can be single-select (dropdown) or multi-select (listbox).

use super::form_control::FormControlElement;
use crate::Handle;
use kozan_macros::{Element, Props};

/// A select (dropdown/listbox) element (`<select>`).
///
/// Chrome equivalent: `HTMLSelectElement`.
#[derive(Copy, Clone, Element)]
#[element(tag = "select", focusable, data = SelectData)]
pub struct HtmlSelectElement(Handle);

/// Element-specific data for `<select>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlSelectElement)]
pub struct SelectData {
    /// Whether multiple options can be selected.
    #[prop]
    pub multiple: bool,
    /// The number of visible rows (for listbox mode).
    #[prop]
    pub size: u32,
}

impl FormControlElement for HtmlSelectElement {}
