//! `HTMLButtonElement` — a clickable button element.
//!
//! Chrome equivalent: `HTMLButtonElement` (inherits from `HTMLFormControlElement`).
//! Adds `label`, `type`, `value`, `form` association.
//!
//! `#[derive(Element)]` generates the full trait chain including `HtmlElement`.
//! `#[derive(Props)]` generates `label()` / `set_label()` etc.

use kozan_macros::{Element, Props};

use super::form_control::FormControlElement;
use crate::Handle;

/// A clickable button element (`<button>`).
///
/// Chrome hierarchy: `HTMLElement → HTMLFormControlElement → HTMLButtonElement`.
#[derive(Copy, Clone, Element)]
#[element(tag = "button", focusable, data = ButtonData)]
pub struct HtmlButtonElement(Handle);

/// Element-specific data for `<button>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlButtonElement)]
pub struct ButtonData {
    /// The button's text label.
    #[prop]
    pub label: String,
}

impl FormControlElement for HtmlButtonElement {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;
    use crate::dom::traits::Element;

    #[test]
    fn button_tag_name() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        assert_eq!(btn.tag_name(), "button");
    }

    #[test]
    fn button_label_prop() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();

        assert_eq!(btn.label(), "");
        btn.set_label("Click me".to_string());
        assert_eq!(btn.label(), "Click me");
    }

    #[test]
    fn button_disabled_via_form_control() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();

        // FormControlElement trait: disabled via attributes.
        assert!(!btn.disabled());

        btn.set_disabled(true);
        assert!(btn.disabled());
        // The disabled attribute should be present.
        assert!(btn.attribute("disabled").is_some());

        btn.set_disabled(false);
        assert!(!btn.disabled());
        assert!(btn.attribute("disabled").is_none());
    }

    #[test]
    fn button_name_via_form_control() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();

        assert_eq!(btn.name(), "");
        btn.set_name("my-button");
        assert_eq!(btn.name(), "my-button");
    }

    #[test]
    fn button_form_id() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();

        assert!(btn.form_id().is_none());
        btn.set_form_id("my-form");
        assert_eq!(btn.form_id(), Some("my-form".to_string()));
    }

    #[test]
    fn button_required_via_form_control() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();

        assert!(!btn.required());
        btn.set_required(true);
        assert!(btn.required());
    }

    #[test]
    fn button_check_validity_default() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        // Default: always valid.
        assert!(btn.check_validity());
    }
}
