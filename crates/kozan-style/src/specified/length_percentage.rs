//! Specified `<length-percentage>` — length, percentage, or calc expression.
//!
//! This is the main type for CSS properties that accept `<length-percentage>`.
//! At specified level, all original CSS units are preserved.

use crate::calc::CalcNode;
use crate::computed;
use crate::context::ComputeContext;

/// Specified `<length-percentage>`.
#[derive(Clone, Debug, PartialEq)]
pub enum LengthPercentage {
    Length(super::Length),
    Percentage(computed::Percentage),
    Calc(Box<CalcNode<SpecifiedLeaf>>),
}

/// Leaf type for specified-level calc expressions.
///
/// Preserves all original CSS units (em, rem, vw, etc.).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecifiedLeaf {
    Length(super::Length),
    Percentage(computed::Percentage),
    Number(f32),
}

impl From<super::Length> for SpecifiedLeaf {
    fn from(l: super::Length) -> Self { Self::Length(l) }
}

impl From<computed::Percentage> for SpecifiedLeaf {
    fn from(p: computed::Percentage) -> Self { Self::Percentage(p) }
}

impl From<f32> for SpecifiedLeaf {
    fn from(n: f32) -> Self { Self::Number(n) }
}

impl crate::ToComputedValue for LengthPercentage {
    type ComputedValue = computed::LengthPercentage;

    fn to_computed_value(&self, ctx: &ComputeContext) -> computed::LengthPercentage {
        match self {
            Self::Length(l) => {
                computed::LengthPercentage::Length(l.to_computed_value(ctx))
            }
            Self::Percentage(p) => {
                computed::LengthPercentage::Percentage(*p)
            }
            Self::Calc(node) => {
                // Resolve all relative units to px, keep percentages as-is
                let computed_node = node.clone().map_leaves(&|leaf| {
                    match leaf {
                        SpecifiedLeaf::Length(l) => {
                            computed::CalcLeaf::Length(l.to_computed_value(ctx))
                        }
                        SpecifiedLeaf::Percentage(p) => {
                            computed::CalcLeaf::Percentage(p)
                        }
                        SpecifiedLeaf::Number(n) => {
                            computed::CalcLeaf::Number(n)
                        }
                    }
                });
                // Try to simplify: if the result is a single leaf, unwrap it
                simplify_computed_calc(computed_node)
            }
        }
    }

    fn from_computed_value(computed: &computed::LengthPercentage) -> Self {
        match computed {
            computed::LengthPercentage::Length(l) => {
                Self::Length(crate::ToComputedValue::from_computed_value(l))
            }
            computed::LengthPercentage::Percentage(p) => {
                Self::Percentage(*p)
            }
            computed::LengthPercentage::Calc(node) => {
                // Convert computed calc back to specified (for animations)
                let specified_node = node.clone().map_leaves(&|leaf| {
                    match leaf {
                        computed::CalcLeaf::Length(l) => {
                            SpecifiedLeaf::Length(super::Length::from_computed_value(&l))
                        }
                        computed::CalcLeaf::Percentage(p) => {
                            SpecifiedLeaf::Percentage(p)
                        }
                        computed::CalcLeaf::Number(n) => {
                            SpecifiedLeaf::Number(n)
                        }
                    }
                });
                Self::Calc(Box::new(specified_node))
            }
        }
    }
}

/// Try to simplify a computed calc node into a plain Length or Percentage.
fn simplify_computed_calc(node: CalcNode<computed::CalcLeaf>) -> computed::LengthPercentage {
    match node {
        CalcNode::Leaf(computed::CalcLeaf::Length(l)) => computed::LengthPercentage::Length(l),
        CalcNode::Leaf(computed::CalcLeaf::Percentage(p)) => computed::LengthPercentage::Percentage(p),
        other => computed::LengthPercentage::Calc(Box::new(other)),
    }
}

impl Default for LengthPercentage {
    fn default() -> Self {
        Self::Length(super::length::px(0.0))
    }
}

impl From<super::Length> for LengthPercentage {
    fn from(l: super::Length) -> Self { Self::Length(l) }
}

impl From<computed::Percentage> for LengthPercentage {
    fn from(p: computed::Percentage) -> Self { Self::Percentage(p) }
}
impl core::fmt::Display for LengthPercentage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Length(l) => write!(f, "{l}"),
            Self::Percentage(p) => write!(f, "{p}"),
            Self::Calc(node) => write!(f, "calc({node})"),
        }
    }
}

impl core::fmt::Display for SpecifiedLeaf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Length(l) => write!(f, "{l}"),
            Self::Percentage(p) => write!(f, "{p}"),
            Self::Number(n) => write!(f, "{n}"),
        }
    }
}
impl core::ops::Add for super::Length {
    type Output = LengthPercentage;
    fn add(self, rhs: Self) -> LengthPercentage {
        LengthPercentage::Calc(Box::new(CalcNode::add(
            CalcNode::Leaf(SpecifiedLeaf::Length(self)),
            CalcNode::Leaf(SpecifiedLeaf::Length(rhs)),
        )))
    }
}

impl core::ops::Sub for super::Length {
    type Output = LengthPercentage;
    fn sub(self, rhs: Self) -> LengthPercentage {
        LengthPercentage::Calc(Box::new(CalcNode::sub(
            CalcNode::Leaf(SpecifiedLeaf::Length(self)),
            CalcNode::Leaf(SpecifiedLeaf::Length(rhs)),
        )))
    }
}

// Length +/- Percentage → LengthPercentage
impl core::ops::Add<computed::Percentage> for super::Length {
    type Output = LengthPercentage;
    fn add(self, rhs: computed::Percentage) -> LengthPercentage {
        LengthPercentage::Calc(Box::new(CalcNode::add(
            CalcNode::Leaf(SpecifiedLeaf::Length(self)),
            CalcNode::Leaf(SpecifiedLeaf::Percentage(rhs)),
        )))
    }
}

impl core::ops::Sub<computed::Percentage> for super::Length {
    type Output = LengthPercentage;
    fn sub(self, rhs: computed::Percentage) -> LengthPercentage {
        LengthPercentage::Calc(Box::new(CalcNode::sub(
            CalcNode::Leaf(SpecifiedLeaf::Length(self)),
            CalcNode::Leaf(SpecifiedLeaf::Percentage(rhs)),
        )))
    }
}

// Percentage +/- Length → LengthPercentage
impl core::ops::Add<super::Length> for computed::Percentage {
    type Output = LengthPercentage;
    fn add(self, rhs: super::Length) -> LengthPercentage {
        LengthPercentage::Calc(Box::new(CalcNode::add(
            CalcNode::Leaf(SpecifiedLeaf::Percentage(self)),
            CalcNode::Leaf(SpecifiedLeaf::Length(rhs)),
        )))
    }
}

impl core::ops::Sub<super::Length> for computed::Percentage {
    type Output = LengthPercentage;
    fn sub(self, rhs: super::Length) -> LengthPercentage {
        LengthPercentage::Calc(Box::new(CalcNode::sub(
            CalcNode::Leaf(SpecifiedLeaf::Percentage(self)),
            CalcNode::Leaf(SpecifiedLeaf::Length(rhs)),
        )))
    }
}
