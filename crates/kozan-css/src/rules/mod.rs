//! CSS rule types — the document structure of a parsed stylesheet.
//!
//! Every at-rule and style rule is represented here. Rule lists use
//! `ThinArc<(), CssRule>` for 8-byte thin pointers (consistent with
//! kozan-atom and kozan-selector).

use kozan_atom::Atom;
use kozan_selector::SelectorList;
use kozan_style::DeclarationBlock;
use triomphe::ThinArc;

pub mod media;
pub mod keyframes;
pub mod layer;
pub mod supports;
pub mod container;
pub mod misc;

pub use media::*;
pub use keyframes::*;
pub use layer::*;
pub use supports::*;
pub use container::*;
pub use misc::*;

// RuleList — 8-byte thin pointer to contiguous rule storage

/// 8-byte thin pointer to an immutable list of CSS rules.
///
/// Uses `ThinArc` (same as kozan-atom's `Atom` and kozan-selector's `Selector`)
/// for cache-friendly contiguous storage with minimal pointer overhead.
pub type RuleList = ThinArc<(), CssRule>;

/// Create a `RuleList` from a `Vec` during parsing.
#[inline]
pub fn rules_from_vec(rules: Vec<CssRule>) -> RuleList {
    ThinArc::from_header_and_iter((), rules.into_iter())
}

/// Create an empty `RuleList`. Cached to avoid repeated allocations.
#[inline]
pub fn empty_rules() -> RuleList {
    use std::sync::LazyLock;
    static EMPTY: LazyLock<RuleList> = LazyLock::new(|| {
        ThinArc::from_header_and_iter((), core::iter::empty())
    });
    EMPTY.clone()
}

// Stylesheet — top-level parsed CSS document

/// A parsed CSS stylesheet.
pub struct Stylesheet {
    /// All top-level rules in source order.
    pub rules: RuleList,
    /// Source URL for error reporting (e.g. `"styles.css"`).
    pub source_url: Option<Atom>,
}

// CssRule — discriminated union of all rule types

/// Every kind of CSS rule. Large variants are boxed to keep the enum at 16 bytes
/// (tag + pointer), which is cache-friendly for array traversal.
pub enum CssRule {
    /// `selector { declarations }` — the most common rule type.
    Style(Box<StyleRule>),
    /// `@media (...) { rules }`
    Media(Box<MediaRule>),
    /// `@keyframes name { ... }`
    Keyframes(Box<KeyframesRule>),
    /// `@layer name { rules }` or `@layer name1, name2;`
    Layer(Box<LayerRule>),
    /// `@supports (...) { rules }`
    Supports(Box<SupportsRule>),
    /// `@container name (...) { rules }`
    Container(Box<ContainerRule>),
    /// `@font-face { declarations }`
    FontFace(Box<FontFaceRule>),
    /// `@import url(...) ...;`
    Import(Box<ImportRule>),
    /// `@namespace prefix url(...)` — small enough to inline (~24 bytes).
    Namespace(NamespaceRule),
    /// `@page :pseudo { declarations }`
    Page(Box<PageRule>),
    /// `@property --name { ... }` — custom property registration.
    Property(Box<PropertyRule>),
    /// `@counter-style name { ... }` — custom list markers.
    CounterStyle(Box<CounterStyleRule>),
    /// `@scope [(start)] [to (end)] { rules }` — scoped styles.
    Scope(Box<ScopeRule>),
    /// `@starting-style { rules }` — entry animation styles.
    StartingStyle(Box<StartingStyleRule>),
}

// StyleRule — selector + declarations + nested rules (CSS Nesting)

/// A style rule: selectors, declarations, and optionally nested child rules.
pub struct StyleRule {
    /// The selector list (e.g. `.container > .item, #main`).
    pub selectors: SelectorList,
    /// Property declarations within this rule.
    pub declarations: DeclarationBlock,
    /// Nested rules (CSS Nesting Level 1). Empty ThinArc if no nesting.
    pub rules: RuleList,
}
