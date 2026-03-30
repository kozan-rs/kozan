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
#[derive(Clone, Debug)]
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
///
/// Spec: CSS Properties and Values API Level 1 §3.2
/// <https://www.w3.org/TR/css-properties-values-api-1/#syntax-strings>
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertySyntax {
    /// `"*"` — universal syntax (any value, the default for unregistered props).
    Universal,
    /// A typed syntax string (e.g. `"<length>"`, `"<color>+"`, `"<length> | auto"`).
    Typed(Atom),
}

/// A single syntax type token as defined by CSS Properties & Values API Level 1 §3.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxType {
    Length,
    Number,
    Percentage,
    LengthPercentage,
    Color,
    Image,
    Url,
    Integer,
    Angle,
    Time,
    Resolution,
    TransformFunction,
    TransformList,
    CustomIdent,
    String,
}

/// How many values a syntax component accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxMultiplier {
    /// Exactly one value.
    Once,
    /// One or more, space-separated (`+`).
    SpaceList,
    /// One or more, comma-separated (`#`).
    CommaList,
}

/// One arm of a `|`-separated syntax — a typed component with optional multiplier,
/// or a literal `<custom-ident>` keyword.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyntaxComponent {
    Type(SyntaxType, SyntaxMultiplier),
    /// A literal keyword (e.g. `auto` in `<length> | auto`).
    Keyword(Atom),
}

impl PropertySyntax {
    /// Parse the raw syntax string into structured components for validation.
    ///
    /// Returns `None` for `Universal` (all values accepted) or if the syntax
    /// string cannot be parsed (implementation falls back to accepting any value).
    pub fn parse_components(&self) -> Option<Vec<SyntaxComponent>> {
        let raw = match self {
            Self::Universal => return None,
            Self::Typed(atom) => atom.as_ref(),
        };
        let mut components = Vec::new();
        for part in raw.split('|') {
            let part = part.trim();
            if part.is_empty() { continue; }
            if let Some(inner) = part.strip_prefix('<').and_then(|s| {
                // Strip trailing `>` and optional `+`/`#` multiplier
                s.strip_suffix(">+").map(|n| (n, SyntaxMultiplier::SpaceList))
                    .or_else(|| s.strip_suffix(">#").map(|n| (n, SyntaxMultiplier::CommaList)))
                    .or_else(|| s.strip_suffix('>').map(|n| (n, SyntaxMultiplier::Once)))
            }) {
                let (name, mult) = inner;
                let ty = match name {
                    "length" => SyntaxType::Length,
                    "number" => SyntaxType::Number,
                    "percentage" => SyntaxType::Percentage,
                    "length-percentage" => SyntaxType::LengthPercentage,
                    "color" => SyntaxType::Color,
                    "image" => SyntaxType::Image,
                    "url" => SyntaxType::Url,
                    "integer" => SyntaxType::Integer,
                    "angle" => SyntaxType::Angle,
                    "time" => SyntaxType::Time,
                    "resolution" => SyntaxType::Resolution,
                    "transform-function" => SyntaxType::TransformFunction,
                    "transform-list" => SyntaxType::TransformList,
                    "custom-ident" => SyntaxType::CustomIdent,
                    "string" => SyntaxType::String,
                    _ => return None, // unknown type — accept all
                };
                components.push(SyntaxComponent::Type(ty, mult));
            } else {
                // Literal keyword
                components.push(SyntaxComponent::Keyword(Atom::from(part)));
            }
        }
        if components.is_empty() { None } else { Some(components) }
    }

    /// Validate a CSS value string against this syntax.
    ///
    /// Returns `true` if the value is valid for this syntax, `false` if it is
    /// invalid at computed-value time per CSS Properties & Values API Level 1 §7.
    ///
    /// Uses heuristic checks — not full CSS parsing — so it may accept some
    /// edge cases that a strict validator would reject. False positives (accepting
    /// invalid values) are safer than false negatives (rejecting valid values).
    pub fn validate(&self, value: &str) -> bool {
        let value = value.trim();
        let components = match self.parse_components() {
            None => return true, // Universal or unparseable syntax → accept all
            Some(c) => c,
        };
        // Check against any arm of the `|`-separated list.
        for component in &components {
            if component_matches(component, value) {
                return true;
            }
        }
        false
    }
}

/// Check if a CSS value string matches a single syntax component.
fn component_matches(component: &SyntaxComponent, value: &str) -> bool {
    match component {
        SyntaxComponent::Keyword(kw) => value.eq_ignore_ascii_case(kw.as_ref()),
        SyntaxComponent::Type(ty, mult) => type_matches_list(*ty, *mult, value),
    }
}

/// Check if a list of values (based on multiplier) all satisfy the type.
fn type_matches_list(ty: SyntaxType, mult: SyntaxMultiplier, value: &str) -> bool {
    match mult {
        SyntaxMultiplier::Once => type_matches(ty, value.trim()),
        SyntaxMultiplier::SpaceList => {
            value.split_whitespace().all(|v| type_matches(ty, v))
                && !value.trim().is_empty()
        }
        SyntaxMultiplier::CommaList => {
            value.split(',').all(|v| type_matches(ty, v.trim()))
                && !value.trim().is_empty()
        }
    }
}

/// Heuristic check: does `value` look like a valid instance of `ty`?
///
/// Not a complete CSS parser — catches obviously wrong values.
/// Per the spec, we err on the side of acceptance (false positives are safe).
fn type_matches(ty: SyntaxType, value: &str) -> bool {
    if value.is_empty() { return false; }
    // CSS-wide keywords are always valid for any registered property.
    if matches!(value, "initial" | "inherit" | "unset" | "revert" | "revert-layer") {
        return true;
    }
    match ty {
        SyntaxType::Integer => is_integer(value),
        SyntaxType::Number => is_number(value),
        SyntaxType::Percentage => value.ends_with('%') && is_number(value.trim_end_matches('%')),
        SyntaxType::Length => is_length(value),
        SyntaxType::LengthPercentage => is_length(value)
            || (value.ends_with('%') && is_number(value.trim_end_matches('%'))),
        SyntaxType::Angle => is_angle(value),
        SyntaxType::Time => is_time(value),
        SyntaxType::Resolution => is_resolution(value),
        SyntaxType::Color => is_color(value),
        SyntaxType::Url => value.starts_with("url(") && value.ends_with(')'),
        SyntaxType::Image => {
            value.starts_with("url(") && value.ends_with(')')
                || value.starts_with("linear-gradient(")
                || value.starts_with("radial-gradient(")
                || value.starts_with("conic-gradient(")
                || value == "none"
        }
        SyntaxType::TransformFunction => is_transform_function(value),
        SyntaxType::TransformList => {
            // Space-separated transform functions
            let mut rest = value;
            while !rest.is_empty() {
                let end = find_function_end(rest).unwrap_or(rest.len());
                if !is_transform_function(&rest[..end]) { return false; }
                rest = rest[end..].trim_start();
            }
            true
        }
        SyntaxType::CustomIdent => is_custom_ident(value),
        SyntaxType::String => value.starts_with('"') && value.ends_with('"')
            || value.starts_with('\'') && value.ends_with('\''),
    }
}

fn is_integer(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s).strip_prefix('+').unwrap_or(s);
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_number(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s).strip_prefix('+').unwrap_or(s);
    if s.is_empty() { return false; }
    let (int_part, dec_part) = s.split_once('.').unwrap_or((s, ""));
    let int_ok = !int_part.is_empty() && int_part.bytes().all(|b| b.is_ascii_digit())
        || int_part.is_empty() && !dec_part.is_empty();
    let dec_ok = dec_part.is_empty() || dec_part.bytes().all(|b| b.is_ascii_digit());
    int_ok && dec_ok
}

fn is_length(s: &str) -> bool {
    if s == "0" { return true; }
    const UNITS: &[&str] = &[
        "px", "em", "rem", "ex", "ch", "vw", "vh", "vmin", "vmax", "vb", "vi",
        "svw", "svh", "lvw", "lvh", "dvw", "dvh", "cqw", "cqh", "cqi", "cqb",
        "pt", "pc", "in", "cm", "mm", "Q", "cap", "ic", "lh", "rlh", "rcap",
    ];
    for unit in UNITS {
        if let Some(num) = s.strip_suffix(unit) {
            if is_number(num) { return true; }
        }
    }
    // calc() — accept without full validation
    s.starts_with("calc(")
}

fn is_angle(s: &str) -> bool {
    if s == "0" { return true; }
    for unit in &["deg", "rad", "grad", "turn"] {
        if let Some(num) = s.strip_suffix(unit) {
            if is_number(num) { return true; }
        }
    }
    false
}

fn is_time(s: &str) -> bool {
    for unit in &["s", "ms"] {
        if let Some(num) = s.strip_suffix(unit) {
            if is_number(num) { return true; }
        }
    }
    false
}

fn is_resolution(s: &str) -> bool {
    for unit in &["dpi", "dpcm", "dppx", "x"] {
        if let Some(num) = s.strip_suffix(unit) {
            if is_number(num) { return true; }
        }
    }
    false
}

fn is_color(s: &str) -> bool {
    // Hex colors
    if s.starts_with('#') && (s.len() == 4 || s.len() == 5 || s.len() == 7 || s.len() == 9) {
        return s[1..].bytes().all(|b| b.is_ascii_hexdigit());
    }
    // Color functions
    if s.starts_with("rgb(") || s.starts_with("rgba(")
        || s.starts_with("hsl(") || s.starts_with("hsla(")
        || s.starts_with("lab(") || s.starts_with("lch(")
        || s.starts_with("oklab(") || s.starts_with("oklch(")
        || s.starts_with("color(")
    {
        return s.ends_with(')');
    }
    // Keywords — exhaustive list not practical; accept any ident as potential color keyword
    // (the cascade's type system will catch real mismatches at apply time)
    is_custom_ident(s) && !s.contains('(')
}

fn is_transform_function(s: &str) -> bool {
    const TRANSFORMS: &[&str] = &[
        "matrix(", "matrix3d(", "translate(", "translateX(", "translateY(",
        "translateZ(", "translate3d(", "scale(", "scaleX(", "scaleY(", "scaleZ(",
        "scale3d(", "rotate(", "rotateX(", "rotateY(", "rotateZ(", "rotate3d(",
        "skew(", "skewX(", "skewY(", "perspective(",
    ];
    TRANSFORMS.iter().any(|prefix| s.starts_with(prefix) && s.ends_with(')'))
}

fn is_custom_ident(s: &str) -> bool {
    if s.is_empty() { return false; }
    // CSS ident: starts with a letter or `-` (then letter), rest alphanumeric/-/_
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if first == '-' {
        let second = match chars.next() { Some(c) => c, None => return false };
        if !second.is_ascii_alphabetic() && second != '_' && second != '-' { return false; }
    } else if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Find the end of a CSS function call (including nested parens), returns byte offset.
fn find_function_end(s: &str) -> Option<usize> {
    let open = s.find('(')?;
    let mut depth = 0u32;
    for (i, b) in s.bytes().enumerate() {
        if b == b'(' { depth += 1; }
        if b == b')' {
            depth -= 1;
            if depth == 0 { return Some(i + 1); }
        }
        let _ = open; // suppress unused warning
    }
    None
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
