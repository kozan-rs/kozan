//! `@container` rule types — container queries.

use kozan_atom::Atom;
use smallvec::SmallVec;
use super::RuleList;
use super::media::{RangeOp, MediaFeatureValue};

/// `@container [name] (condition) { rules }` — container size query.
#[derive(Clone)]
pub struct ContainerRule {
    /// Optional container name.
    pub name: Option<Atom>,
    /// The container condition tree.
    pub condition: ContainerCondition,
    /// Rules that apply when the condition is true.
    pub rules: RuleList,
}

/// Boolean condition tree for container queries.
#[derive(Clone)]
pub enum ContainerCondition {
    /// A single container size feature test.
    Feature(ContainerSizeFeature),
    /// `not (condition)`.
    Not(Box<ContainerCondition>),
    /// `(cond1) and (cond2) [and ...]`.
    And(SmallVec<[Box<ContainerCondition>; 2]>),
    /// `(cond1) or (cond2) [or ...]`.
    Or(SmallVec<[Box<ContainerCondition>; 2]>),
}

/// A single container size feature: `(width >= 768px)`.
#[derive(Clone)]
pub struct ContainerSizeFeature {
    /// Feature name (width, height, inline-size, block-size, etc.).
    pub name: Atom,
    /// Comparison operator.
    pub op: RangeOp,
    /// The value to compare against.
    pub value: MediaFeatureValue,
}
