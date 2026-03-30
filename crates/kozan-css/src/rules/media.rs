//! `@media` rule types — media queries and condition trees.

use kozan_atom::Atom;
use smallvec::SmallVec;
use super::RuleList;

/// `@media (condition) { rules }` — conditional rule based on media features.
pub struct MediaRule {
    /// The media query list (comma-separated).
    pub queries: MediaQueryList,
    /// Rules that apply when the media condition is true.
    pub rules: RuleList,
}

/// Comma-separated list of media queries: `screen and (min-width: 768px), print`.
pub struct MediaQueryList(pub SmallVec<[MediaQuery; 2]>);

impl MediaQueryList {
    /// An empty query list that matches nothing.
    pub fn empty() -> Self {
        Self(SmallVec::new())
    }
}

/// A single media query: `[not|only] type [and (condition)]`.
pub struct MediaQuery {
    /// Optional qualifier: `not` or `only`.
    pub qualifier: Option<MediaQualifier>,
    /// Media type: `all`, `screen`, `print`, or custom.
    pub media_type: MediaType,
    /// Optional condition: `(min-width: 768px) and (color)`.
    pub condition: Option<MediaCondition>,
}

/// Media query qualifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaQualifier {
    Not,
    Only,
}

/// Media type keyword.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaType {
    All,
    Screen,
    Print,
    Custom(Atom),
}

/// Boolean condition tree for media features.
///
/// Represents `(feature) and (feature)`, `not (feature)`, etc.
pub enum MediaCondition {
    /// A single media feature test.
    Feature(MediaFeature),
    /// `not (condition)`.
    Not(Box<MediaCondition>),
    /// `(cond1) and (cond2) [and ...]`.
    And(SmallVec<[Box<MediaCondition>; 2]>),
    /// `(cond1) or (cond2) [or ...]`.
    Or(SmallVec<[Box<MediaCondition>; 2]>),
}

/// A single media feature test inside parentheses.
pub enum MediaFeature {
    /// `(name: value)` — plain feature with exact value.
    Plain { name: Atom, value: MediaFeatureValue },
    /// `(name op value)` — range syntax (e.g. `min-width: 768px` or `width >= 768px`).
    Range { name: Atom, op: RangeOp, value: MediaFeatureValue },
    /// `(name)` — boolean test (e.g. `(color)`, `(hover)`).
    Boolean(Atom),
}

/// Comparison operator for range media features.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeOp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Lightweight value for media features — NOT full CSS property values.
///
/// Media features only accept a small subset of CSS values.
#[derive(Clone)]
pub enum MediaFeatureValue {
    /// Length value with unit (e.g. `768px`, `48em`).
    Length(f32, LengthUnit),
    /// Bare number (e.g. `2` for `color`).
    Number(f32),
    /// Integer value.
    Integer(i32),
    /// Ratio (e.g. `16/9`).
    Ratio(u32, u32),
    /// Identifier value (e.g. `landscape`, `coarse`).
    Ident(Atom),
}

/// Length units used in media features.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthUnit {
    // Absolute
    Px,
    Cm,
    Mm,
    In,
    Pt,
    Pc,
    // Font-relative
    Em,
    Rem,
    Ch,
    Ex,
    // Viewport (default)
    Vw,
    Vh,
    Vmin,
    Vmax,
    Vi,
    Vb,
    // Viewport (small)
    Svw,
    Svh,
    // Viewport (large)
    Lvw,
    Lvh,
    // Viewport (dynamic)
    Dvw,
    Dvh,
    // Container query
    Cqw,
    Cqh,
    Cqi,
    Cqb,
}
