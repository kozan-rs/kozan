//! Computed `<length>` — always CSS pixels.
//!
//! All relative units (em, rem, vw, ch, etc.) have been resolved
//! by `ToComputedValue` using the `ComputeContext`.

/// A computed CSS length: always in px.
///
/// This is the simplest possible representation — a single f32.
/// Layout and paint code works with this directly.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Length(pub f32);

impl Length {
    /// Zero length.
    pub const ZERO: Self = Self(0.0);

    /// Creates a computed length from a px value.
    #[inline]
    pub const fn new(px: f32) -> Self { Self(px) }

    /// Returns the value in CSS pixels.
    #[inline]
    pub const fn px(&self) -> f32 { self.0 }

    /// Returns `true` if this length is exactly zero.
    #[inline]
    pub fn is_zero(&self) -> bool { self.0 == 0.0 }

    /// Clamps this length to the given range.
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0))
    }

    /// Returns the minimum of two lengths.
    #[inline]
    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    /// Returns the maximum of two lengths.
    #[inline]
    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    /// Returns the absolute value of this length.
    #[inline]
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }
}

impl core::ops::Add for Length {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self { Self(self.0 + rhs.0) }
}

impl core::ops::Sub for Length {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self { Self(self.0 - rhs.0) }
}

impl core::ops::Mul<f32> for Length {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self { Self(self.0 * rhs) }
}

impl core::ops::Div<f32> for Length {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self { Self(self.0 / rhs) }
}

impl core::ops::Neg for Length {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self { Self(-self.0) }
}

impl core::ops::AddAssign for Length {
    #[inline]
    fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; }
}

impl core::ops::SubAssign for Length {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; }
}

impl core::fmt::Display for Length {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == 0.0 {
            f.write_str("0px")
        } else {
            write!(f, "{}px", self.0)
        }
    }
}
