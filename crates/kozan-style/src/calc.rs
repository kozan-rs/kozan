//! CSS calc expression tree — generic over leaf type.
//!
//! `CalcNode<L>` is the recursive AST for calc(), min(), max(), clamp(), etc.
//! The leaf type `L` differs between specified and computed levels:
//! - Specified: `SpecifiedLeaf` (has em/rem/vw/vh/etc.)
//! - Computed: `CalcLeaf` (only px + % + number)
//!
//! This ensures type safety — the compiler prevents mixing levels.

/// A generic calc expression tree node.
///
/// `L` is the leaf value type. The tree structure is the same at
/// both specified and computed levels; only the leaves differ.
#[derive(Clone, Debug, PartialEq)]
pub enum CalcNode<L> {
    /// Concrete value leaf.
    Leaf(L),
    /// Negate: `-a`.
    Negate(Box<CalcNode<L>>),
    /// Invert: `1/a` (leaf must resolve to a number).
    Invert(Box<CalcNode<L>>),
    /// Sum: `a + b + c`.
    Sum(Box<[CalcNode<L>]>),
    /// Product: `a * b * c`.
    Product(Box<[CalcNode<L>]>),
    /// `min(a, b, ...)` or `max(a, b, ...)`.
    MinMax(Box<[CalcNode<L>]>, MinMaxOp),
    /// `clamp(min, center, max)`.
    Clamp {
        min: Box<CalcNode<L>>,
        center: Box<CalcNode<L>>,
        max: Box<CalcNode<L>>,
    },
    /// `abs(a)`.
    Abs(Box<CalcNode<L>>),
    /// `sign(a)` — returns -1, 0, or 1.
    Sign(Box<CalcNode<L>>),
}

/// Whether a MinMax node is `min()` or `max()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinMaxOp {
    Min,
    Max,
}

impl<L> CalcNode<L> {
    /// Transform every leaf in the tree.
    pub fn map_leaves<M>(self, f: &impl Fn(L) -> M) -> CalcNode<M> {
        match self {
            Self::Leaf(l) => CalcNode::Leaf(f(l)),
            Self::Negate(a) => CalcNode::Negate(Box::new(a.map_leaves(f))),
            Self::Invert(a) => CalcNode::Invert(Box::new(a.map_leaves(f))),
            Self::Sum(args) => CalcNode::Sum(
                args.into_vec().into_iter().map(|a| a.map_leaves(f)).collect()
            ),
            Self::Product(args) => CalcNode::Product(
                args.into_vec().into_iter().map(|a| a.map_leaves(f)).collect()
            ),
            Self::MinMax(args, op) => CalcNode::MinMax(
                args.into_vec().into_iter().map(|a| a.map_leaves(f)).collect(),
                op,
            ),
            Self::Clamp { min, center, max } => CalcNode::Clamp {
                min: Box::new(min.map_leaves(f)),
                center: Box::new(center.map_leaves(f)),
                max: Box::new(max.map_leaves(f)),
            },
            Self::Abs(a) => CalcNode::Abs(Box::new(a.map_leaves(f))),
            Self::Sign(a) => CalcNode::Sign(Box::new(a.map_leaves(f))),
        }
    }

    /// Transform every leaf by reference — does NOT clone the tree first.
    ///
    /// Prefer this over `node.clone().map_leaves(f)` when `node` is behind
    /// a shared reference: avoids allocating a full tree copy just to discard
    /// it after the transform.
    pub fn map_leaves_ref<M>(&self, f: &impl Fn(&L) -> M) -> CalcNode<M> {
        match self {
            Self::Leaf(l) => CalcNode::Leaf(f(l)),
            Self::Negate(a) => CalcNode::Negate(Box::new(a.map_leaves_ref(f))),
            Self::Invert(a) => CalcNode::Invert(Box::new(a.map_leaves_ref(f))),
            Self::Sum(args) => CalcNode::Sum(
                args.iter().map(|a| a.map_leaves_ref(f)).collect()
            ),
            Self::Product(args) => CalcNode::Product(
                args.iter().map(|a| a.map_leaves_ref(f)).collect()
            ),
            Self::MinMax(args, op) => CalcNode::MinMax(
                args.iter().map(|a| a.map_leaves_ref(f)).collect(),
                *op,
            ),
            Self::Clamp { min, center, max } => CalcNode::Clamp {
                min: Box::new(min.map_leaves_ref(f)),
                center: Box::new(center.map_leaves_ref(f)),
                max: Box::new(max.map_leaves_ref(f)),
            },
            Self::Abs(a) => CalcNode::Abs(Box::new(a.map_leaves_ref(f))),
            Self::Sign(a) => CalcNode::Sign(Box::new(a.map_leaves_ref(f))),
        }
    }

    /// Transform every leaf, potentially returning a different node (not just a leaf).
    pub fn flat_map_leaves<M>(self, f: &impl Fn(L) -> CalcNode<M>) -> CalcNode<M> {
        match self {
            Self::Leaf(l) => f(l),
            Self::Negate(a) => CalcNode::Negate(Box::new(a.flat_map_leaves(f))),
            Self::Invert(a) => CalcNode::Invert(Box::new(a.flat_map_leaves(f))),
            Self::Sum(args) => CalcNode::Sum(
                args.into_vec().into_iter().map(|a| a.flat_map_leaves(f)).collect()
            ),
            Self::Product(args) => CalcNode::Product(
                args.into_vec().into_iter().map(|a| a.flat_map_leaves(f)).collect()
            ),
            Self::MinMax(args, op) => CalcNode::MinMax(
                args.into_vec().into_iter().map(|a| a.flat_map_leaves(f)).collect(),
                op,
            ),
            Self::Clamp { min, center, max } => CalcNode::Clamp {
                min: Box::new(min.flat_map_leaves(f)),
                center: Box::new(center.flat_map_leaves(f)),
                max: Box::new(max.flat_map_leaves(f)),
            },
            Self::Abs(a) => CalcNode::Abs(Box::new(a.flat_map_leaves(f))),
            Self::Sign(a) => CalcNode::Sign(Box::new(a.flat_map_leaves(f))),
        }
    }
}

impl<L: core::fmt::Display> core::fmt::Display for CalcNode<L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Leaf(l) => write!(f, "{l}"),
            Self::Negate(a) => write!(f, "(-1 * {a})"),
            Self::Invert(a) => write!(f, "(1 / {a})"),
            Self::Sum(args) => {
                f.write_str("(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { f.write_str(" + ")?; }
                    write!(f, "{arg}")?;
                }
                f.write_str(")")
            }
            Self::Product(args) => {
                f.write_str("(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { f.write_str(" * ")?; }
                    write!(f, "{arg}")?;
                }
                f.write_str(")")
            }
            Self::MinMax(args, op) => {
                match op {
                    MinMaxOp::Min => f.write_str("min(")?,
                    MinMaxOp::Max => f.write_str("max(")?,
                }
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { f.write_str(", ")?; }
                    write!(f, "{arg}")?;
                }
                f.write_str(")")
            }
            Self::Clamp { min, center, max } => {
                write!(f, "clamp({min}, {center}, {max})")
            }
            Self::Abs(a) => write!(f, "abs({a})"),
            Self::Sign(a) => write!(f, "sign({a})"),
        }
    }
}
impl<L> CalcNode<L> {
    /// `a + b`
    pub fn add(a: CalcNode<L>, b: CalcNode<L>) -> Self {
        Self::Sum(Box::from([a, b]))
    }

    /// `a - b` (implemented as `a + (-b)`)
    pub fn sub(a: CalcNode<L>, b: CalcNode<L>) -> Self {
        Self::Sum(Box::from([a, Self::Negate(Box::new(b))]))
    }

    /// `a * n`
    pub fn mul_number(a: CalcNode<L>, n: f32) -> Self
    where L: From<f32>
    {
        Self::Product(Box::from([a, Self::Leaf(L::from(n))]))
    }

    /// `a / n`
    pub fn div_number(a: CalcNode<L>, n: f32) -> Self
    where L: From<f32>
    {
        Self::Product(Box::from([a, Self::Invert(Box::new(Self::Leaf(L::from(n))))]))
    }
}

use crate::declared::{UnparsedValue, SubstitutionRefs};

// Operator overloading: any expression touching UnparsedValue becomes raw CSS.
// Pure typed expressions (no var/env) stay as CalcNode.

impl core::ops::Add<UnparsedValue> for UnparsedValue {
    type Output = UnparsedValue;
    fn add(self, rhs: UnparsedValue) -> UnparsedValue {
        UnparsedValue {
            css: crate::Atom::new(&format!("calc({} + {})", self.css, rhs.css)),
            references: self.references | rhs.references,
        }
    }
}

impl core::ops::Sub<UnparsedValue> for UnparsedValue {
    type Output = UnparsedValue;
    fn sub(self, rhs: UnparsedValue) -> UnparsedValue {
        UnparsedValue {
            css: crate::Atom::new(&format!("calc({} - {})", self.css, rhs.css)),
            references: self.references | rhs.references,
        }
    }
}

macro_rules! impl_unparsed_ops_with_display {
    ($ty:ty) => {
        impl core::ops::Add<$ty> for UnparsedValue {
            type Output = UnparsedValue;
            fn add(self, rhs: $ty) -> UnparsedValue {
                UnparsedValue {
                    css: crate::Atom::new(&format!("calc({} + {rhs})", self.css)),
                    references: self.references,
                }
            }
        }
        impl core::ops::Sub<$ty> for UnparsedValue {
            type Output = UnparsedValue;
            fn sub(self, rhs: $ty) -> UnparsedValue {
                UnparsedValue {
                    css: crate::Atom::new(&format!("calc({} - {rhs})", self.css)),
                    references: self.references,
                }
            }
        }
        impl core::ops::Add<UnparsedValue> for $ty {
            type Output = UnparsedValue;
            fn add(self, rhs: UnparsedValue) -> UnparsedValue {
                UnparsedValue {
                    css: crate::Atom::new(&format!("calc({self} + {})", rhs.css)),
                    references: rhs.references,
                }
            }
        }
        impl core::ops::Sub<UnparsedValue> for $ty {
            type Output = UnparsedValue;
            fn sub(self, rhs: UnparsedValue) -> UnparsedValue {
                UnparsedValue {
                    css: crate::Atom::new(&format!("calc({self} - {})", rhs.css)),
                    references: rhs.references,
                }
            }
        }
    };
}

impl_unparsed_ops_with_display!(crate::specified::Length);
impl_unparsed_ops_with_display!(crate::computed::Percentage);

/// Creates a `var(--name)` reference for the builder.
pub fn var(name: &str) -> UnparsedValue {
    UnparsedValue {
        css: crate::Atom::new(&format!("var(--{name})")),
        references: SubstitutionRefs::VAR,
    }
}

/// Creates a `var(--name, fallback)` reference for the builder.
pub fn var_or(name: &str, fallback: &str) -> UnparsedValue {
    UnparsedValue {
        css: crate::Atom::new(&format!("var(--{name}, {fallback})")),
        references: SubstitutionRefs::VAR,
    }
}

/// Creates an `env(name)` reference for the builder.
pub fn env(name: &str) -> UnparsedValue {
    UnparsedValue {
        css: crate::Atom::new(&format!("env({name})")),
        references: SubstitutionRefs::ENV,
    }
}

/// Creates an `env(name, fallback)` reference for the builder.
pub fn env_or(name: &str, fallback: &str) -> UnparsedValue {
    UnparsedValue {
        css: crate::Atom::new(&format!("env({name}, {fallback})")),
        references: SubstitutionRefs::ENV,
    }
}

/// Creates an `attr(name)` reference for the builder.
pub fn attr(name: &str) -> UnparsedValue {
    UnparsedValue {
        css: crate::Atom::new(&format!("attr({name})")),
        references: SubstitutionRefs::ATTR,
    }
}

/// Creates raw unparsed CSS text for the builder.
pub fn unparsed(css: &str) -> UnparsedValue {
    let mut refs = SubstitutionRefs::empty();
    if css.contains("var(") { refs = refs | SubstitutionRefs::VAR; }
    if css.contains("env(") { refs = refs | SubstitutionRefs::ENV; }
    if css.contains("attr(") { refs = refs | SubstitutionRefs::ATTR; }
    UnparsedValue { css: crate::Atom::new(css), references: refs }
}
