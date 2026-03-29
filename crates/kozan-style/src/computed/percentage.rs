//! Computed `<percentage>`.
//!
//! Stored as a fraction: 0.0 = 0%, 1.0 = 100%.
//! Resolved to px at layout time via `LengthPercentage::resolve(basis)`.

/// A computed CSS percentage as a fraction (0.5 = 50%).
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Percentage(pub f32);

impl Percentage {
    /// Zero percent.
    pub const ZERO: Self = Self(0.0);
    /// 100%.
    pub const FULL: Self = Self(1.0);

    /// Creates a percentage from a fraction (0.5 = 50%).
    #[inline]
    pub const fn new(fraction: f32) -> Self { Self(fraction) }

    /// The fraction value (0.5 = 50%).
    #[inline]
    pub const fn value(&self) -> f32 { self.0 }

    /// Resolve against a basis: `50% of 800px = 400px`.
    #[inline]
    pub fn resolve(&self, basis: super::Length) -> super::Length {
        super::Length(self.0 * basis.0)
    }

    /// Returns `true` if this percentage is exactly zero.
    #[inline]
    pub fn is_zero(&self) -> bool { self.0 == 0.0 }
}

impl core::ops::Add for Percentage {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Self(self.0 + rhs.0) }
}

impl core::ops::Sub for Percentage {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Self(self.0 - rhs.0) }
}

impl core::ops::Mul<f32> for Percentage {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self { Self(self.0 * rhs) }
}

impl core::ops::Neg for Percentage {
    type Output = Self;
    fn neg(self) -> Self { Self(-self.0) }
}

impl core::fmt::Display for Percentage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}%", self.0 * 100.0)
    }
}
