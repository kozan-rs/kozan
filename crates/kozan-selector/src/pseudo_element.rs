//! CSS pseudo-element types.
//!
//! Pseudo-elements create "virtual" sub-elements in the render tree that don't
//! exist in the DOM. They allow styling specific parts of an element's content
//! or generated content that appears before/after the element.
//!
//! # Constraints
//!
//! Per CSS Selectors Level 4 (§4):
//! - Only ONE pseudo-element is allowed per compound selector.
//! - It MUST appear at the end (rightmost position) of the selector.
//! - Only a limited subset of CSS properties apply to each pseudo-element
//!   (e.g., `::first-line` only accepts typographic properties).
//!
//! # Syntax
//!
//! CSS3 requires double-colon syntax (`::before`), but CSS2 pseudo-elements
//! (::before, ::after, ::first-line, ::first-letter) also accept the legacy
//! single-colon syntax (`:before`) for backwards compatibility. Newer
//! pseudo-elements (`::placeholder`, `::selection`, `::marker`) require `::`.
//!
//! # Specificity
//!
//! Pseudo-elements contribute to specificity column `c` (same as type
//! selectors), adding (0, 0, 1) per the CSS cascade specification.
//!
//! # Spec Reference
//!
//! <https://drafts.csswg.org/selectors-4/#pseudo-elements>
//! <https://drafts.csswg.org/css-pseudo-4/>

/// CSS pseudo-elements — virtual sub-elements for styling.
///
/// Each variant maps to a CSS pseudo-element keyword. The parser produces
/// these from `::keyword` (or `:keyword` for legacy-compatible ones).
/// The matching engine always returns `true` for pseudo-element components —
/// pseudo-element matching is handled at a higher level (style resolution
/// creates separate style contexts for pseudo-elements).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoElement {
    /// `::before` — generated content inserted before the element's content.
    /// Requires the `content` property to produce output.
    Before,
    /// `::after` — generated content inserted after the element's content.
    /// Requires the `content` property to produce output.
    After,
    /// `::first-line` — the first formatted line of a block container.
    /// Only a subset of CSS properties apply (font, color, background, etc.).
    FirstLine,
    /// `::first-letter` — the first typographic letter unit of a block.
    /// Only a subset of CSS properties apply (font, margin, padding, etc.).
    FirstLetter,
    /// `::placeholder` — placeholder text in `<input>` and `<textarea>`.
    Placeholder,
    /// `::selection` — the portion of content selected/highlighted by the user.
    /// Note: the standard name is `::selection` (Firefox uses `::-moz-selection`).
    Selection,
    /// `::marker` — the marker box of a list item (`<li>`, `display: list-item`).
    /// Controls bullet/number style, color, size, and content.
    Marker,
    /// `::backdrop` — the backdrop behind a fullscreen element or `<dialog>`.
    /// Covers the entire viewport when an element is in fullscreen mode.
    Backdrop,
    /// `::file-selector-button` — the button inside `<input type="file">`.
    /// Fully styleable tree-abiding pseudo-element.
    FileSelectorButton,
    /// `::grammar-error` — text flagged as grammatically incorrect by the UA.
    /// Highlight pseudo-element with limited property support.
    GrammarError,
    /// `::spelling-error` — text flagged as misspelled by the UA.
    /// Highlight pseudo-element with limited property support.
    SpellingError,
    // NOTE: `::highlight(name)` is a functional pseudo-element — handled as
    // `Component::Highlight(Atom)` in types.rs, not here, because it takes an argument.
}

impl PseudoElement {
    /// Whether this pseudo-element accepts the legacy single-colon syntax.
    ///
    /// CSS2.1 defined `:before`, `:after`, `:first-line`, `:first-letter`
    /// with single colons. CSS3 changed to `::` but browsers still accept
    /// the old syntax for these four. Newer pseudo-elements require `::`.
    pub const fn allows_single_colon(self) -> bool {
        matches!(self, Self::Before | Self::After | Self::FirstLine | Self::FirstLetter)
    }

    /// Whether this pseudo-element creates generated content (requires
    /// the `content` CSS property to produce visible output).
    pub const fn is_generated_content(self) -> bool {
        matches!(self, Self::Before | Self::After)
    }

    /// Specificity contribution: all pseudo-elements add (0, 0, 1).
    pub const fn specificity_count(self) -> u32 {
        1
    }
}
