//! CSS parser for the Kozan UI platform.
//!
//! Turns CSS text into typed `PropertyDeclaration` values from `kozan-style`.
//! Native-only — on web, the browser IS the CSS engine.

mod error;
pub(crate) mod declaration;
mod shorthand;
mod var;
pub mod properties;
pub mod rules;
mod stylesheet;

// Re-export error types
pub use error::{CustomError, Error, SourceLocation};

// Re-export parse trait
pub use properties::Parse;

// Re-export rule types
pub use rules::{
    CssRule, RuleList, StyleRule, Stylesheet,
    MediaRule, MediaQueryList, MediaQuery, MediaCondition, MediaFeature,
    MediaQualifier, MediaType, MediaFeatureValue, RangeOp, LengthUnit,
    KeyframesRule, KeyframeBlock, KeyframeSelector,
    LayerRule, LayerName,
    SupportsRule, SupportsCondition,
    ContainerRule, ContainerCondition, ContainerSizeFeature,
    FontFaceRule, ImportRule, NamespaceRule, PageRule,
    PropertyRule, PropertySyntax, CounterStyleRule,
    ScopeRule, StartingStyleRule,
};

// Re-export stylesheet parsing
pub use stylesheet::{parse_stylesheet, parse_stylesheet_with_url};

/// Parse inline style declarations (no selectors — like a `style` attribute).
pub fn parse_inline(css: &str) -> kozan_style::DeclarationBlock {
    declaration::parse_declaration_list(css)
}

/// Parse a single property value from CSS text.
pub fn parse_value(property: kozan_style::PropertyId, css: &str) -> Option<kozan_style::PropertyDeclaration> {
    declaration::parse_single_value(property, css)
}
