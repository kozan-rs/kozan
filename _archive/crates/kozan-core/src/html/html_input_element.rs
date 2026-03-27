//! `HTMLInputElement` — the most versatile form control.
//!
//! Chrome equivalent: `HTMLInputElement` + `InputType` strategy pattern.
//! Each input type (text, password, number, range, date, color, file, etc.)
//! has different behavior. Chrome delegates to separate `InputType` subclasses.
//!
//! # Chrome's `InputType` strategy
//!
//! ```text
//! HTMLInputElement
//!   └── input_type_: InputType*
//!         ├── TextFieldInputType
//!         ├── RangeInputType
//!         ├── NumberInputType
//!         ├── ColorInputType
//!         ├── FileInputType
//!         └── ... (20+ types)
//! ```
//!
//! For Kozan Phase 1: we store `input_type` as an enum and handle behavior
//! centrally. The strategy pattern can be introduced when we need complex
//! per-type behavior (shadow DOM, custom rendering).

use super::form_control::{FormControlElement, TextControlElement};
use crate::Handle;
use kozan_macros::{Element, Props};

/// The type of an `<input>` element.
///
/// Chrome equivalent: the `InputType` class hierarchy.
/// Each variant maps to a different set of behaviors, rendering, and validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputType {
    /// `<input type="text">` — single-line text field.
    #[default]
    Text,
    /// `<input type="password">` — obscured text field.
    Password,
    /// `<input type="number">` — numeric input with spinner.
    Number,
    /// `<input type="email">` — email address.
    Email,
    /// `<input type="url">` — URL input.
    Url,
    /// `<input type="tel">` — telephone number.
    Tel,
    /// `<input type="search">` — search field.
    Search,
    /// `<input type="range">` — slider control.
    Range,
    /// `<input type="color">` — color picker.
    Color,
    /// `<input type="date">` — date picker.
    Date,
    /// `<input type="time">` — time picker.
    Time,
    /// `<input type="datetime-local">` — date+time picker.
    DatetimeLocal,
    /// `<input type="checkbox">` — boolean toggle.
    Checkbox,
    /// `<input type="radio">` — one-of-many selection.
    Radio,
    /// `<input type="file">` — file upload.
    File,
    /// `<input type="submit">` — form submit button.
    Submit,
    /// `<input type="reset">` — form reset button.
    Reset,
    /// `<input type="button">` — generic button (no default action).
    Button,
    /// `<input type="hidden">` — hidden data.
    Hidden,
    /// `<input type="image">` — image submit button.
    Image,
}

impl InputType {
    /// Parse an input type from its string representation.
    ///
    /// Unknown types default to `Text` (per HTML spec).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "text" => Self::Text,
            "password" => Self::Password,
            "number" => Self::Number,
            "email" => Self::Email,
            "url" => Self::Url,
            "tel" => Self::Tel,
            "search" => Self::Search,
            "range" => Self::Range,
            "color" => Self::Color,
            "date" => Self::Date,
            "time" => Self::Time,
            "datetime-local" => Self::DatetimeLocal,
            "checkbox" => Self::Checkbox,
            "radio" => Self::Radio,
            "file" => Self::File,
            "submit" => Self::Submit,
            "reset" => Self::Reset,
            "button" => Self::Button,
            "hidden" => Self::Hidden,
            "image" => Self::Image,
            _ => Self::Text, // HTML spec: unknown type → text
        }
    }

    /// Whether this input type is a text-editing type.
    #[must_use]
    pub fn is_text_type(&self) -> bool {
        matches!(
            self,
            Self::Text
                | Self::Password
                | Self::Number
                | Self::Email
                | Self::Url
                | Self::Tel
                | Self::Search
        )
    }

    /// Whether this input type has a checked state.
    #[must_use]
    pub fn is_checkable(&self) -> bool {
        matches!(self, Self::Checkbox | Self::Radio)
    }

    /// Whether this input type is a button.
    #[must_use]
    pub fn is_button_type(&self) -> bool {
        matches!(
            self,
            Self::Submit | Self::Reset | Self::Button | Self::Image
        )
    }

    /// Whether this input type is focusable.
    #[must_use]
    pub fn is_focusable(&self) -> bool {
        !matches!(self, Self::Hidden)
    }
}

/// An input element (`<input>`).
///
/// Chrome equivalent: `HTMLInputElement`.
/// The most versatile form control — behavior depends on `input_type`.
#[derive(Copy, Clone, Element)]
#[element(tag = "input", focusable, data = InputData)]
pub struct HtmlInputElement(Handle);

/// Element-specific data for `<input>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlInputElement)]
pub struct InputData {
    /// The input type (text, password, number, etc.).
    #[prop]
    pub input_type: InputType,
    /// Whether the checkbox/radio is checked.
    #[prop]
    pub checked: bool,
}

impl FormControlElement for HtmlInputElement {}
impl TextControlElement for HtmlInputElement {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_type_parse() {
        assert_eq!(InputType::parse("text"), InputType::Text);
        assert_eq!(InputType::parse("PASSWORD"), InputType::Password);
        assert_eq!(InputType::parse("number"), InputType::Number);
        assert_eq!(InputType::parse("checkbox"), InputType::Checkbox);
        assert_eq!(InputType::parse("unknown"), InputType::Text); // spec default
    }

    #[test]
    fn text_type_classification() {
        assert!(InputType::Text.is_text_type());
        assert!(InputType::Password.is_text_type());
        assert!(!InputType::Checkbox.is_text_type());
        assert!(!InputType::File.is_text_type());
    }

    #[test]
    fn checkable_classification() {
        assert!(InputType::Checkbox.is_checkable());
        assert!(InputType::Radio.is_checkable());
        assert!(!InputType::Text.is_checkable());
    }

    #[test]
    fn button_type_classification() {
        assert!(InputType::Submit.is_button_type());
        assert!(InputType::Reset.is_button_type());
        assert!(InputType::Button.is_button_type());
        assert!(InputType::Image.is_button_type());
        assert!(!InputType::Text.is_button_type());
        assert!(!InputType::Checkbox.is_button_type());
    }

    #[test]
    fn focusable_classification() {
        // All types are focusable except hidden.
        assert!(InputType::Text.is_focusable());
        assert!(InputType::Password.is_focusable());
        assert!(InputType::Checkbox.is_focusable());
        assert!(InputType::Submit.is_focusable());
        assert!(!InputType::Hidden.is_focusable());
    }

    #[test]
    fn input_type_default() {
        assert_eq!(InputType::default(), InputType::Text);
    }

    #[test]
    fn input_data_props() {
        use crate::dom::document::Document;

        let doc = Document::new();
        let input = doc.create::<HtmlInputElement>();

        // Default input_type is Text.
        assert_eq!(input.input_type(), InputType::Text);
        assert!(!input.checked());

        // Set input_type via data prop.
        input.set_input_type(InputType::Checkbox);
        assert_eq!(input.input_type(), InputType::Checkbox);

        // Set checked.
        input.set_checked(true);
        assert!(input.checked());
    }

    #[test]
    fn input_text_control() {
        use crate::dom::document::Document;

        let doc = Document::new();
        let input = doc.create::<HtmlInputElement>();

        // TextControlElement: value, placeholder.
        input.set_value("hello");
        assert_eq!(input.value(), "hello");

        input.set_placeholder("Enter text...");
        assert_eq!(input.placeholder(), "Enter text...");
    }

    #[test]
    fn input_form_control() {
        use crate::dom::document::Document;

        let doc = Document::new();
        let input = doc.create::<HtmlInputElement>();

        // FormControlElement: disabled, name, required.
        assert!(!input.disabled());
        input.set_disabled(true);
        assert!(input.disabled());

        input.set_name("username");
        assert_eq!(input.name(), "username");
    }
}
