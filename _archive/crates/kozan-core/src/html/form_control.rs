//! Form control element trait — shared behavior for all form controls.
//!
//! Chrome equivalent: `HTMLFormControlElement`.
//! Intermediate trait between `HtmlElement` and concrete form controls
//! (button, input, select, textarea).
//!
//! # What it provides
//!
//! - `disabled` / `set_disabled` (all form controls can be disabled)
//! - `name` / `set_name` (form submission key)
//! - `form_id` (associated form element)
//! - `validity` (constraint validation — future)
//!
//! # Chrome hierarchy
//!
//! ```text
//! HTMLElement
//!   └── HTMLFormControlElement      ← THIS TRAIT
//!         ├── HTMLButtonElement
//!         ├── HTMLSelectElement
//!         └── HTMLTextControlElement ← Sub-trait for text editing
//!               ├── HTMLInputElement
//!               └── HTMLTextAreaElement
//! ```

use super::html_element::HtmlElement;

/// Shared behavior for all form control elements.
///
/// Chrome equivalent: `HTMLFormControlElement`.
/// Every form control (button, input, select, textarea) implements this.
///
/// All methods have default implementations that read/write attributes.
pub trait FormControlElement: HtmlElement {
    /// Whether this control is disabled.
    ///
    /// Disabled controls don't receive events and are excluded from
    /// form submission. Chrome: `HTMLFormControlElement::IsDisabledFormControl()`.
    fn disabled(&self) -> bool {
        self.attribute("disabled").is_some()
    }

    fn set_disabled(&self, disabled: bool) {
        if disabled {
            self.set_attribute("disabled", "");
        } else {
            self.remove_attribute("disabled");
        }
    }

    /// The control's name (used as the key in form submission).
    fn name(&self) -> String {
        self.attribute("name").unwrap_or_default()
    }

    fn set_name(&self, name: impl Into<String>) {
        self.set_attribute("name", name);
    }

    /// The ID of the associated `<form>` element.
    ///
    /// Chrome: `HTMLFormControlElement::formOwner()` resolves this
    /// to the actual form element. Currently returns the raw attribute;
    /// form element resolution will be added with the form submission system.
    fn form_id(&self) -> Option<String> {
        self.attribute("form")
    }

    fn set_form_id(&self, form_id: impl Into<String>) {
        self.set_attribute("form", form_id);
    }

    /// Whether this control is required for form submission.
    fn required(&self) -> bool {
        self.attribute("required").is_some()
    }

    fn set_required(&self, required: bool) {
        if required {
            self.set_attribute("required", "");
        } else {
            self.remove_attribute("required");
        }
    }

    /// Whether this control's value satisfies its constraints.
    ///
    /// Chrome: `HTMLFormControlElement::checkValidity()`.
    /// Future: constraint validation API.
    fn check_validity(&self) -> bool {
        // Default: always valid. Override in concrete elements.
        true
    }
}

/// Shared behavior for text-editing form controls (input, textarea).
///
/// Chrome equivalent: `TextControlElement`.
/// Adds value, selection, and text editing capabilities.
pub trait TextControlElement: FormControlElement {
    /// The current text value.
    fn value(&self) -> String {
        self.attribute("value").unwrap_or_default()
    }

    fn set_value(&self, value: impl Into<String>) {
        self.set_attribute("value", value);
    }

    /// The placeholder text shown when the control is empty.
    fn placeholder(&self) -> String {
        self.attribute("placeholder").unwrap_or_default()
    }

    fn set_placeholder(&self, placeholder: impl Into<String>) {
        self.set_attribute("placeholder", placeholder);
    }

    /// Whether the control is read-only (value visible but not editable).
    fn readonly(&self) -> bool {
        self.attribute("readonly").is_some()
    }

    fn set_readonly(&self, readonly: bool) {
        if readonly {
            self.set_attribute("readonly", "");
        } else {
            self.remove_attribute("readonly");
        }
    }

    /// Maximum number of characters allowed.
    fn max_length(&self) -> Option<u32> {
        self.attribute("maxlength").and_then(|v| v.parse().ok())
    }

    fn set_max_length(&self, max: u32) {
        self.set_attribute("maxlength", max.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;
    use crate::html::HtmlButtonElement;

    #[test]
    fn button_disabled() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        assert!(!btn.disabled());

        btn.set_disabled(true);
        assert!(btn.disabled());

        btn.set_disabled(false);
        assert!(!btn.disabled());
    }

    #[test]
    fn button_name() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        assert_eq!(btn.name(), "");

        btn.set_name("submit-btn");
        assert_eq!(btn.name(), "submit-btn");
    }

    #[test]
    fn button_required() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        assert!(!btn.required());

        btn.set_required(true);
        assert!(btn.required());
    }
}
