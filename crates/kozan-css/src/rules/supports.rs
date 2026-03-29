//! `@supports` rule types — feature queries.

use smallvec::SmallVec;
use super::RuleList;

/// `@supports (condition) { rules }` — conditional rule based on CSS support.
pub struct SupportsRule {
    /// The supports condition tree.
    pub condition: SupportsCondition,
    /// Whether the condition evaluated to true at parse time.
    pub enabled: bool,
    /// Rules that apply when the condition is true.
    pub rules: RuleList,
}

/// Boolean condition tree for `@supports`.
pub enum SupportsCondition {
    /// `(property: value)` — tests whether a declaration is supported.
    Declaration(Box<str>),
    /// `not (condition)`.
    Not(Box<SupportsCondition>),
    /// `(cond1) and (cond2) [and ...]`.
    And(SmallVec<[Box<SupportsCondition>; 2]>),
    /// `(cond1) or (cond2) [or ...]`.
    Or(SmallVec<[Box<SupportsCondition>; 2]>),
    /// `selector(...)` — tests whether a selector is supported.
    Selector(Box<str>),
}
