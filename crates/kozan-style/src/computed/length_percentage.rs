//! Computed `<length-percentage>` — px, %, or calc(px + %).
//!
//! This is the workhorse type for CSS layout properties.
//! At computed level, all relative units (em, rem, vw) are resolved to px.
//! Only px and % remain; calc expressions with mixed px+% survive.
//!
//! Resolution to final px happens at layout time via `resolve(basis)`.

use super::{Length, Percentage};
use crate::calc::CalcNode;

/// Computed `<length-percentage>`.
///
/// Three representations, optimized for the common cases:
/// - `Length`: pure px value (most common — no heap alloc)
/// - `Percentage`: pure % value (no heap alloc)
/// - `Calc`: expression tree with px + % leaves (heap allocated, rare)
#[derive(Clone, Debug, PartialEq)]
pub enum LengthPercentage {
    Length(Length),
    Percentage(Percentage),
    Calc(Box<CalcNode<CalcLeaf>>),
}

/// Leaf type for computed calc expressions.
///
/// Only three possibilities remain at computed level — all em/rem/vw/ch/etc.
/// have been resolved to px by `ToComputedValue`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CalcLeaf {
    Length(Length),
    Percentage(Percentage),
    Number(f32),
}

impl core::fmt::Display for CalcLeaf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Length(l) => write!(f, "{l}"),
            Self::Percentage(p) => write!(f, "{p}"),
            Self::Number(n) => write!(f, "{n}"),
        }
    }
}

impl LengthPercentage {
    /// Creates a zero length-percentage.
    #[inline]
    pub fn zero() -> Self { Self::Length(Length::ZERO) }

    /// Creates a computed length-percentage from a px value.
    #[inline]
    pub fn px(v: f32) -> Self { Self::Length(Length::new(v)) }

    /// Creates a computed length-percentage from a fraction (0.5 = 50%).
    #[inline]
    pub fn percent(v: f32) -> Self { Self::Percentage(Percentage::new(v)) }

    /// Returns `true` if this value is exactly zero.
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Length(l) => l.is_zero(),
            Self::Percentage(p) => p.is_zero(),
            Self::Calc(_) => false,
        }
    }

    /// Resolve percentages against a containing block dimension.
    ///
    /// `basis` is the containing block size for the relevant axis.
    /// After this call, the result is a pure px value.
    pub fn resolve(&self, basis: Length) -> Length {
        match self {
            Self::Length(l) => *l,
            Self::Percentage(p) => p.resolve(basis),
            Self::Calc(node) => resolve_calc_node(node, basis),
        }
    }

    /// Try to get as pure px, if no percentage involved.
    pub fn as_length(&self) -> Option<Length> {
        match self {
            Self::Length(l) => Some(*l),
            _ => None,
        }
    }

    /// Try to get as pure percentage.
    pub fn as_percentage(&self) -> Option<Percentage> {
        match self {
            Self::Percentage(p) => Some(*p),
            _ => None,
        }
    }

    /// Returns `true` if this is a pure length (px) value.
    #[inline]
    pub fn is_length(&self) -> bool { matches!(self, Self::Length(_)) }

    /// Returns `true` if this is a pure percentage value.
    #[inline]
    pub fn is_percentage(&self) -> bool { matches!(self, Self::Percentage(_)) }

    /// Returns `true` if this contains a calc expression.
    #[inline]
    pub fn is_calc(&self) -> bool { matches!(self, Self::Calc(_)) }
}

impl Default for LengthPercentage {
    fn default() -> Self { Self::zero() }
}

impl From<Length> for LengthPercentage {
    fn from(l: Length) -> Self { Self::Length(l) }
}

impl From<Percentage> for LengthPercentage {
    fn from(p: Percentage) -> Self { Self::Percentage(p) }
}

impl core::fmt::Display for LengthPercentage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Length(l) => write!(f, "{l}"),
            Self::Percentage(p) => write!(f, "{p}"),
            Self::Calc(node) => {
                f.write_str("calc(")?;
                write!(f, "{node}")?;
                f.write_str(")")
            }
        }
    }
}

/// Resolve a computed calc node to final px.
fn resolve_calc_node(node: &CalcNode<CalcLeaf>, basis: Length) -> Length {
    match node {
        CalcNode::Leaf(leaf) => resolve_leaf(leaf, basis),
        CalcNode::Negate(inner) => -resolve_calc_node(inner, basis),
        CalcNode::Invert(inner) => {
            let v = resolve_calc_node(inner, basis);
            Length::new(1.0 / v.px())
        }
        CalcNode::Sum(args) => {
            let mut total = Length::ZERO;
            for arg in args.iter() {
                total += resolve_calc_node(arg, basis);
            }
            total
        }
        CalcNode::Product(args) => {
            let mut result = 1.0_f32;
            for arg in args.iter() {
                result *= resolve_calc_node(arg, basis).px();
            }
            Length::new(result)
        }
        CalcNode::MinMax(args, op) => {
            let mut iter = args.iter().map(|a| resolve_calc_node(a, basis).px());
            let first = iter.next().unwrap_or(0.0);
            let result = match op {
                crate::calc::MinMaxOp::Min => iter.fold(first, f32::min),
                crate::calc::MinMaxOp::Max => iter.fold(first, f32::max),
            };
            Length::new(result)
        }
        CalcNode::Clamp { min, center, max } => {
            let min_v = resolve_calc_node(min, basis).px();
            let center_v = resolve_calc_node(center, basis).px();
            let max_v = resolve_calc_node(max, basis).px();
            Length::new(center_v.clamp(min_v, max_v))
        }
        CalcNode::Abs(inner) => Length::new(resolve_calc_node(inner, basis).px().abs()),
        CalcNode::Sign(inner) => {
            let v = resolve_calc_node(inner, basis).px();
            Length::new(if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 })
        }
    }
}

fn resolve_leaf(leaf: &CalcLeaf, basis: Length) -> Length {
    match leaf {
        CalcLeaf::Length(l) => *l,
        CalcLeaf::Percentage(p) => p.resolve(basis),
        CalcLeaf::Number(n) => Length::new(*n),
    }
}
