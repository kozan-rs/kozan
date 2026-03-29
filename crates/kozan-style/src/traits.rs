//! Core traits for the CSS value pipeline.

use crate::ComputeContext;

/// Serialize a value to CSS text.
///
/// Every CSS type (specified and computed) implements this for DevTools,
/// `getComputedStyle()`, CSSOM serialization.
pub trait ToCss {
    /// Serializes this value to a CSS string.
    fn to_css<W: core::fmt::Write>(&self, dest: &mut W) -> core::fmt::Result;

    /// Serializes to an owned `String`.
    fn to_css_string(&self) -> String {
        let mut s = String::new();
        self.to_css(&mut s).unwrap();
        s
    }
}

/// Blanket: anything that is Display also gets ToCss via Display.
/// This covers built-in numeric types (f32, i32, u32, u16).
impl<T: core::fmt::Display> ToCss for T {
    fn to_css<W: core::fmt::Write>(&self, dest: &mut W) -> core::fmt::Result {
        write!(dest, "{self}")
    }
}

/// Convert a specified-level value to its computed-level representation.
///
/// This is the bridge between what users write and what the cascade produces.
/// Resolution requires a `ComputeContext` carrying font-size, viewport, etc.
///
/// Bidirectional: `from_computed_value` is needed for CSS animations
/// (interpolating computed values back to specified for re-cascading).
pub trait ToComputedValue {
    type ComputedValue;

    /// Resolves to the computed value using the given context.
    fn to_computed_value(&self, ctx: &ComputeContext) -> Self::ComputedValue;
    /// Reconstructs a specified value from a computed value (for animations).
    fn from_computed_value(computed: &Self::ComputedValue) -> Self;
}

/// Identity impl: types that are the same at specified and computed level.
macro_rules! impl_to_computed_identity {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToComputedValue for $ty {
                type ComputedValue = Self;
                #[inline]
                fn to_computed_value(&self, _ctx: &ComputeContext) -> Self { *self }
                #[inline]
                fn from_computed_value(computed: &Self) -> Self { *computed }
            }
        )+
    };
}

impl_to_computed_identity!(f32, i32, u32, u16, u8, bool);

// All other types get ToComputedValue via #[derive(ToComputedValue)] on their definitions.

// Discrete animation for bool
impl Animate for bool {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match procedure {
            Procedure::Interpolate { progress } => Ok(if progress < 0.5 { *self } else { *other }),
            _ => Err(()),
        }
    }
}

impl ToAnimatedZero for bool {
    fn to_animated_zero(&self) -> Result<Self, ()> { Err(()) }
}

impl ComputeSquaredDistance for bool {
    fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()> {
        Ok(if self == other { 0.0 } else { 1.0 })
    }
}
/// A value that has a meaningful zero representation.
///
/// Used by animation (SMIL `by-animation`) and layout (is_zero checks).
/// Unlike `Default`, `Zero` specifically means "the additive identity" —
/// adding zero to a value leaves it unchanged.
///
/// Examples: `Length(0.0)` is zero, but `Size::Auto` is not (Auto is default, not zero).
pub trait Zero: Sized {
    /// Returns the additive identity (zero) value.
    fn zero() -> Self;

    /// Returns `true` if this value is zero.
    fn is_zero(&self) -> bool;
}

impl Zero for f32 {
    fn zero() -> Self { 0.0 }
    fn is_zero(&self) -> bool { *self == 0.0 }
}

impl Zero for i32 {
    fn zero() -> Self { 0 }
    fn is_zero(&self) -> bool { *self == 0 }
}

impl Zero for u32 {
    fn zero() -> Self { 0 }
    fn is_zero(&self) -> bool { *self == 0 }
}

impl Zero for crate::computed::Length {
    fn zero() -> Self { Self::ZERO }
    fn is_zero(&self) -> bool { self.px() == 0.0 }
}

impl Zero for crate::computed::Percentage {
    fn zero() -> Self { Self::ZERO }
    fn is_zero(&self) -> bool { self.value() == 0.0 }
}

impl Zero for crate::computed::LengthPercentage {
    fn zero() -> Self { Self::zero() }
    fn is_zero(&self) -> bool { self.is_zero() }
}

/// How to interpolate between two CSS values.
///
/// https://drafts.csswg.org/web-animations/#procedures-for-animating-properties
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Procedure {
    /// Standard interpolation: `result = (1 - progress) * from + progress * to`.
    Interpolate { progress: f64 },
    /// Addition: `result = from + to` (SMIL additive animation).
    Add,
    /// Accumulation: `result = count * from + to`.
    Accumulate { count: u64 },
}

impl Procedure {
    /// Express this procedure as a pair of weights for linear combination.
    #[inline]
    pub fn weights(self) -> (f64, f64) {
        match self {
            Self::Interpolate { progress } => (1.0 - progress, progress),
            Self::Add => (1.0, 1.0),
            Self::Accumulate { count } => (count as f64, 1.0),
        }
    }
}

/// Interpolate between two computed CSS values.
///
/// This is the core animation trait. Types that can be smoothly animated
/// implement this (lengths, colors, numbers, transforms, etc.).
/// Discrete properties (display, overflow) return `Err(())`.
pub trait Animate: Sized {
    /// Interpolates between `self` and `other` according to the procedure.
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()>;
}

/// Produce the "zero" value for animation purposes.
///
/// This is NOT the same as `Zero::zero()` or `Default::default()`.
/// For SMIL `by-animation`, the engine interpolates from the zero value
/// to the `by` value, then adds the result to the underlying value.
///
/// Types that can't produce a meaningful zero for animation return `Err(())`.
pub trait ToAnimatedZero: Sized {
    /// Produces the zero value for this type in animation context.
    fn to_animated_zero(&self) -> Result<Self, ()>;
}

/// Compute the squared distance between two animated values.
///
/// Used for paced animation timing — determines how "far apart" two values are.
/// `SquaredDistance` avoids sqrt for performance; the animation engine only
/// needs relative distances.
pub trait ComputeSquaredDistance {
    /// Returns the squared distance between two values for paced animation.
    fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()>;
}
impl Animate for f32 {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let (wa, wb) = procedure.weights();
        Ok((*self as f64 * wa + *other as f64 * wb) as f32)
    }
}

impl Animate for f64 {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let (wa, wb) = procedure.weights();
        Ok(*self * wa + *other * wb)
    }
}

impl Animate for i32 {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let (wa, wb) = procedure.weights();
        Ok((*self as f64 * wa + *other as f64 * wb).round() as i32)
    }
}

impl Animate for crate::computed::Length {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        self.px().animate(&other.px(), procedure).map(Self::new)
    }
}

impl Animate for crate::computed::Percentage {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        self.value().animate(&other.value(), procedure).map(Self::new)
    }
}

impl ToAnimatedZero for f32 {
    fn to_animated_zero(&self) -> Result<Self, ()> { Ok(0.0) }
}

impl ToAnimatedZero for i32 {
    fn to_animated_zero(&self) -> Result<Self, ()> { Ok(0) }
}

impl ToAnimatedZero for crate::computed::Length {
    fn to_animated_zero(&self) -> Result<Self, ()> { Ok(Self::ZERO) }
}

impl ToAnimatedZero for crate::computed::Percentage {
    fn to_animated_zero(&self) -> Result<Self, ()> { Ok(Self::ZERO) }
}

impl ComputeSquaredDistance for f32 {
    fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()> {
        let d = (*self as f64) - (*other as f64);
        Ok(d * d)
    }
}

impl ComputeSquaredDistance for crate::computed::Length {
    fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()> {
        self.px().compute_squared_distance(&other.px())
    }
}

// Stylo resolves to a common basis (100px) for distance, and interpolates
// length and percentage components independently.

impl Animate for crate::computed::LengthPercentage {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        use crate::computed::LengthPercentage as LP;

        match (self, other) {
            // Both pure lengths — simple interpolation
            (LP::Length(a), LP::Length(b)) => {
                Ok(LP::Length(a.animate(b, procedure)?))
            }
            // Both pure percentages
            (LP::Percentage(a), LP::Percentage(b)) => {
                Ok(LP::Percentage(a.animate(b, procedure)?))
            }
            // Mixed: decompose into (length, percentage) pairs and interpolate independently
            _ => {
                let (la, pa) = decompose(self);
                let (lb, pb) = decompose(other);
                let length = la.animate(&lb, procedure)?;
                let pct = pa.animate(&pb, procedure)?;
                if pct.is_zero() {
                    Ok(LP::Length(length))
                } else if length.is_zero() {
                    Ok(LP::Percentage(pct))
                } else {
                    // Result is calc(length + percentage)
                    Ok(LP::Calc(Box::new(crate::CalcNode::Sum(Box::from([
                        crate::CalcNode::Leaf(crate::computed::CalcLeaf::Length(length)),
                        crate::CalcNode::Leaf(crate::computed::CalcLeaf::Percentage(pct)),
                    ])))))
                }
            }
        }
    }
}

impl ToAnimatedZero for crate::computed::LengthPercentage {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Ok(crate::computed::LengthPercentage::zero())
    }
}

impl ComputeSquaredDistance for crate::computed::LengthPercentage {
    fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()> {
        // Use 100px as arbitrary basis for mixed length+percentage
        let basis = crate::computed::Length::new(100.0);
        let a = self.resolve(basis).px() as f64;
        let b = other.resolve(basis).px() as f64;
        let d = a - b;
        Ok(d * d)
    }
}

/// Decompose a LengthPercentage into (length, percentage) components.
fn decompose(lp: &crate::computed::LengthPercentage) -> (crate::computed::Length, crate::computed::Percentage) {
    use crate::computed::{Length, LengthPercentage as LP, Percentage};
    match lp {
        LP::Length(l) => (*l, Percentage::ZERO),
        LP::Percentage(p) => (Length::ZERO, *p),
        LP::Calc(_) => {
            // For calc, resolve with basis=0 to get the length part,
            // and basis=1 to infer the percentage part
            let at_zero = lp.resolve(Length::ZERO).px();
            let at_one = lp.resolve(Length::new(1.0)).px();
            let pct = at_one - at_zero; // how much 1px of basis contributes
            (Length::new(at_zero), Percentage::new(pct))
        }
    }
}
