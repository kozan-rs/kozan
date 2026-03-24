//! `HtmlElement` — shared behavior for ALL HTML elements.
//!
//! Like Chrome's `HTMLElement` (500+ lines of shared logic).
//! Every HTML element (Div, Button, Input, etc.) implements this trait.
//! SVG/MathML elements would NOT implement it.
//!
//! All methods have **default implementations** — concrete elements
//! get them for free. Override only what differs (like Chrome's virtual methods).
//!
//! # Chrome equivalence
//!
//! | Chrome method             | Kozan trait method      |
//! |---------------------------|-------------------------|
//! | `hidden()`                | `hidden()`              |
//! | `title()`                 | `title()`               |
//! | `lang()`                  | `lang()`                |
//! | `dir()`                   | `dir()`                 |
//! | `tabIndex`                | `tab_index()`           |
//! | `click()`                 | `click()`               |
//! | `focus()`                 | `focus()`               |
//! | `blur()`                  | `blur()`                |
//! | `draggable`               | `draggable()`           |
//! | `spellcheck`              | `spellcheck()`          |
//! | `CollectStyleForPresAttr` | `presentation_style()`  |

use crate::dom::traits::Element;

/// Shared behavior for all HTML elements.
///
/// Every method has a default implementation that reads/writes attributes
/// through the `Element` trait. Concrete elements inherit all behavior
/// and override only what they need.
///
/// `HtmlDivElement` overrides: `presentation_style()` (for legacy `align`).
/// `HtmlButtonElement` overrides: nothing (adds props via `ButtonData` instead).
pub trait HtmlElement: Element {
    // ---- Global HTML attributes (default impls read/write from attributes) ----

    /// The `hidden` attribute. Elements with `hidden` should not be rendered.
    fn hidden(&self) -> bool {
        self.attribute("hidden").is_some()
    }

    fn set_hidden(&self, hidden: bool) {
        if hidden {
            self.set_attribute("hidden", "");
        } else {
            self.remove_attribute("hidden");
        }
    }

    /// The `title` attribute (tooltip text).
    fn title(&self) -> String {
        self.attribute("title").unwrap_or_default()
    }

    fn set_title(&self, title: impl Into<String>) {
        self.set_attribute("title", title);
    }

    /// The `lang` attribute (language code).
    fn lang(&self) -> String {
        self.attribute("lang").unwrap_or_default()
    }

    fn set_lang(&self, lang: impl Into<String>) {
        self.set_attribute("lang", lang);
    }

    /// The `dir` attribute (text direction: "ltr", "rtl", "auto").
    fn dir(&self) -> String {
        self.attribute("dir").unwrap_or_default()
    }

    fn set_dir(&self, dir: impl Into<String>) {
        self.set_attribute("dir", dir);
    }

    /// The `tabindex` attribute. Controls focus order.
    /// Returns the element's default if not explicitly set.
    fn tab_index(&self) -> i32 {
        self.attribute("tabindex")
            .and_then(|v| v.parse().ok())
            .unwrap_or(if Self::IS_FOCUSABLE { 0 } else { -1 })
    }

    fn set_tab_index(&self, index: i32) {
        self.set_attribute("tabindex", index.to_string());
    }

    /// The `draggable` attribute.
    fn draggable(&self) -> bool {
        self.attribute("draggable").is_some_and(|v| v == "true")
    }

    fn set_draggable(&self, draggable: bool) {
        self.set_attribute("draggable", if draggable { "true" } else { "false" });
    }

    /// The `spellcheck` attribute.
    fn spellcheck(&self) -> bool {
        self.attribute("spellcheck").is_none_or(|v| v != "false")
    }

    fn set_spellcheck(&self, spellcheck: bool) {
        self.set_attribute("spellcheck", if spellcheck { "true" } else { "false" });
    }

    // ---- Behavioral hooks (override in concrete elements) ----

    /// Map presentation attributes to a `PropertyDeclarationBlock`.
    ///
    /// Like Chrome's `CollectStyleForPresentationAttribute`. Override in elements
    /// that expose legacy presentation attributes (e.g. `align`, `width`, `color`).
    fn collect_presentation_styles(&self) {
        // Default: no presentation attributes to map.
    }

    /// Called when an attribute changes.
    ///
    /// Like Chrome's `HTMLElement::ParseAttribute`.
    /// Default: no-op. Override to react to attribute changes.
    fn attribute_changed(&self, _name: &str, _old: Option<&str>, _new: Option<&str>) {
        // Default: no special handling.
    }

    // ---- Actions ----

    /// Programmatically click this element.
    fn click(&self) {
        // Future: dispatch ClickEvent through the event system.
    }

    /// Focus this element.
    fn focus(&self) {
        // Future: focus management through the document.
    }

    /// Blur (unfocus) this element.
    fn blur(&self) {
        // Future: focus management through the document.
    }
}
