//! Miscellaneous at-rule types: @font-face, @import, @namespace, @page,
//! @property, @counter-style, @scope, @starting-style.

use kozan_atom::Atom;
use kozan_selector::SelectorList;
use kozan_style::DeclarationBlock;
use smallvec::SmallVec;
use super::RuleList;
use super::layer::LayerName;
use super::media::MediaQueryList;
use super::supports::SupportsCondition;

/// `@font-face { declarations }` — web font declaration.
pub struct FontFaceRule {
    /// Font descriptor declarations that are valid CSS properties
    /// (e.g. `font-family`, `font-style`, `font-weight`).
    pub declarations: DeclarationBlock,
    /// Raw descriptor key-value pairs for non-property descriptors
    /// (e.g. `src`, `unicode-range`, `font-display`, `size-adjust`,
    /// `ascent-override`, `descent-override`).
    pub descriptors: Vec<(Atom, Atom)>,
}

/// `@import url(...) [layer(...)] [supports(...)] media;`
pub struct ImportRule {
    /// The URL to import.
    pub url: Atom,
    /// Optional layer assignment.
    pub layer: Option<LayerName>,
    /// Optional supports condition.
    pub supports: Option<SupportsCondition>,
    /// Media query list (defaults to `all` if absent).
    pub media: MediaQueryList,
}

/// `@namespace [prefix] url(...)` — XML namespace declaration.
///
/// Small enough to inline in `CssRule` without boxing (~24 bytes).
pub struct NamespaceRule {
    /// Optional namespace prefix.
    pub prefix: Option<Atom>,
    /// Namespace URL.
    pub url: Atom,
}

/// `@page [:pseudo] { declarations }` — paged media rule.
pub struct PageRule {
    /// Page selectors (`:first`, `:left`, `:right`, `:blank`).
    pub selectors: SmallVec<[Atom; 1]>,
    /// Page-margin and other declarations.
    pub declarations: DeclarationBlock,
}

// @property — custom property registration (CSS Properties & Values API)

/// `@property --name { syntax: "<length>"; inherits: false; initial-value: 0px; }`
///
/// Registers a custom property with a typed syntax, inheritance behavior,
/// and initial value. Without this, custom properties are untyped strings.
pub struct PropertyRule {
    /// Custom property name (e.g. `--gap`, `--theme-color`).
    pub name: Atom,
    /// Syntax descriptor: `"<length>"`, `"<color>"`, `"*"`, etc.
    pub syntax: PropertySyntax,
    /// Whether the property inherits from parent elements.
    pub inherits: bool,
    /// Initial value as raw CSS text (parsed according to `syntax`).
    pub initial_value: Option<Atom>,
}

/// The `syntax` descriptor of `@property`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertySyntax {
    /// `"*"` — universal syntax (any value, the default for unregistered props).
    Universal,
    /// A typed syntax string (e.g. `"<length>"`, `"<color>+"`, `"<length> | auto"`).
    Typed(Atom),
}

// @counter-style — custom list markers (CSS Counter Styles Level 3)

/// `@counter-style name { system: ...; symbols: ...; ... }`
pub struct CounterStyleRule {
    /// Counter style name (e.g. `thumbs`, `circled-alpha`).
    pub name: Atom,
    /// Any descriptors that happen to be valid CSS properties.
    pub declarations: DeclarationBlock,
    /// Raw descriptor key-value pairs for counter-style descriptors
    /// (e.g. `system`, `symbols`, `suffix`, `prefix`, `range`, `pad`,
    /// `fallback`, `negative`, `speak-as`).
    pub descriptors: Vec<(Atom, Atom)>,
}

// @scope — CSS Cascading and Inheritance Level 6

/// `@scope [(start)]? [to (end)]? { rules }`
///
/// Scopes contained rules to elements matching the scope root,
/// optionally excluding elements matching the scope limit.
pub struct ScopeRule {
    /// Scope root selector (e.g. `.card`). `None` = scoped to the stylesheet owner.
    pub start: Option<SelectorList>,
    /// Scope limit selector (e.g. `.card-content`). `None` = no limit.
    pub end: Option<SelectorList>,
    /// Rules that apply within the scope.
    pub rules: RuleList,
}

// @starting-style — CSS Transitions Level 2

/// `@starting-style { rules }`
///
/// Defines styles that apply when an element first enters the document
/// or transitions from `display: none`. Used for entry animations.
pub struct StartingStyleRule {
    /// Rules that define the "before" state for transitions.
    pub rules: RuleList,
}
