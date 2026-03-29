//! CSS specificity calculation.
//!
//! Specificity determines which CSS rule wins when multiple rules match the
//! same element. It's a triple (a, b, c) compared lexicographically:
//!
//! | Column | Counts                                      | Example       |
//! |--------|---------------------------------------------|---------------|
//! | a      | ID selectors                                | `#main` → 1  |
//! | b      | Class selectors, attribute selectors, pseudo-classes | `.foo` → 1 |
//! | c      | Type selectors, pseudo-elements              | `div` → 1   |
//!
//! # Special Cases
//!
//! - `:where()` contributes ZERO specificity (its arguments are ignored).
//! - `:is()`, `:not()`, `:has()` contribute the MAX specificity of their arguments.
//! - `*` (universal selector) contributes nothing.
//!
//! # Packed Representation
//!
//! Packed into a single `u32` for O(1) comparison: `a(8 bits) | b(12 bits) | c(12 bits)`.
//! This gives room for 255 IDs, 4095 classes, 4095 types per selector — more
//! than any real-world CSS uses. The packed form also makes `Ord` comparison
//! a single integer comparison, matching the lexicographic order naturally.
//!
//! # Computed at Parse Time
//!
//! Specificity is accumulated inline during parsing (no second pass). Each
//! simple selector calls `add_id()`, `add_class()`, or `add_type()` as it's
//! parsed. The result is stored in `Selector::specificity` and never changes.
//!
//! # Spec Reference
//!
//! <https://drafts.csswg.org/selectors-4/#specificity-rules>

/// Packed CSS specificity value. Higher numeric value = more specific.
///
/// Implements `Ord` via the packed `u32`, which naturally gives the correct
/// lexicographic (a, b, c) ordering because `a` occupies the highest bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Specificity(u32);

impl Specificity {
    pub const ZERO: Self = Self(0);

    const A_SHIFT: u32 = 24;
    const B_SHIFT: u32 = 12;

    /// Creates a specificity from (a, b, c) components.
    pub const fn new(a: u16, b: u16, c: u16) -> Self {
        Self(((a as u32) << Self::A_SHIFT) | ((b as u32) << Self::B_SHIFT) | (c as u32))
    }

    /// Returns the raw packed value for direct comparison.
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Creates a specificity from a raw packed value.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the (a, b, c) components.
    pub const fn components(self) -> (u16, u16, u16) {
        let a = (self.0 >> Self::A_SHIFT) as u16;
        let b = ((self.0 >> Self::B_SHIFT) & 0xFFF) as u16;
        let c = (self.0 & 0xFFF) as u16;
        (a, b, c)
    }

    /// Adds an ID selector (a += 1).
    pub fn add_id(&mut self) {
        self.0 += 1 << Self::A_SHIFT;
    }

    /// Adds a class, attribute, or pseudo-class (b += 1).
    pub fn add_class(&mut self) {
        self.0 += 1 << Self::B_SHIFT;
    }

    /// Adds a type or pseudo-element (c += 1).
    pub fn add_type(&mut self) {
        self.0 += 1;
    }

    /// Returns the maximum of two specificities.
    pub fn max(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }
}

impl std::fmt::Display for Specificity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (a, b, c) = self.components();
        write!(f, "({a},{b},{c})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero() {
        assert_eq!(Specificity::ZERO.components(), (0, 0, 0));
    }

    #[test]
    fn simple() {
        assert_eq!(Specificity::new(1, 0, 0).components(), (1, 0, 0));
        assert_eq!(Specificity::new(0, 3, 2).components(), (0, 3, 2));
        assert_eq!(Specificity::new(255, 4095, 4095).components(), (255, 4095, 4095));
    }

    #[test]
    fn ordering() {
        // ID beats any number of classes.
        assert!(Specificity::new(1, 0, 0) > Specificity::new(0, 100, 100));
        // More classes beats fewer.
        assert!(Specificity::new(0, 2, 0) > Specificity::new(0, 1, 10));
        // Equal specificity.
        assert_eq!(Specificity::new(0, 1, 1), Specificity::new(0, 1, 1));
    }

    #[test]
    fn add_methods() {
        let mut s = Specificity::ZERO;
        s.add_id();
        s.add_class();
        s.add_class();
        s.add_type();
        assert_eq!(s.components(), (1, 2, 1));
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", Specificity::new(0, 1, 3)), "(0,1,3)");
    }
}
