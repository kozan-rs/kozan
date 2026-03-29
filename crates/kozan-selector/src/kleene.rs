//! Three-valued logic for CSS selector matching.
//!
//! Standard boolean matching (true/false) can't express "unknown" — which is
//! needed for:
//! - `:visited` privacy: browsers must not leak whether a link is visited via
//!   timing or computed style differences. During matching, `:visited` may
//!   return `Unknown` to prevent information leakage.
//! - Invalidation: when computing which selectors *might* match after a DOM
//!   mutation, some conditions are not yet evaluable.
//!
//! Stylo uses this same concept but buries it inside matching.rs. We make it
//! a first-class type with full operator support so any matching consumer
//! (invalidation, style resolution, querySelector) can use it uniformly.

/// Three-valued logic: True, False, or Unknown.
///
/// Follows Kleene's strong logic of indeterminacy:
/// - `Unknown AND False = False` (False dominates AND)
/// - `Unknown OR True = True` (True dominates OR)
/// - `NOT Unknown = Unknown`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KleeneValue {
    True,
    False,
    Unknown,
}

impl KleeneValue {
    /// Returns `true` only if the value is definitely `True`.
    #[inline]
    pub fn is_true(self) -> bool {
        self == Self::True
    }

    /// Returns `true` only if the value is definitely `False`.
    #[inline]
    pub fn is_false(self) -> bool {
        self == Self::False
    }

    /// Returns `true` if the value is `Unknown`.
    #[inline]
    pub fn is_unknown(self) -> bool {
        self == Self::Unknown
    }

    /// Kleene OR over an iterator: returns `True` if any element is `True`,
    /// `False` if all are `False`, `Unknown` otherwise.
    #[inline]
    pub fn any<I, F>(iter: I, mut f: F) -> Self
    where
        I: IntoIterator,
        F: FnMut(I::Item) -> Self,
    {
        let mut result = Self::False;
        for item in iter {
            match f(item) {
                Self::True => return Self::True,
                Self::Unknown => result = Self::Unknown,
                Self::False => {}
            }
        }
        result
    }

    /// Kleene AND over an iterator: returns `False` if any element is `False`,
    /// `True` if all are `True`, `Unknown` otherwise.
    #[inline]
    pub fn all<I, F>(iter: I, mut f: F) -> Self
    where
        I: IntoIterator,
        F: FnMut(I::Item) -> Self,
    {
        let mut result = Self::True;
        for item in iter {
            match f(item) {
                Self::False => return Self::False,
                Self::Unknown => result = Self::Unknown,
                Self::True => {}
            }
        }
        result
    }
}

impl From<bool> for KleeneValue {
    #[inline]
    fn from(b: bool) -> Self {
        if b { Self::True } else { Self::False }
    }
}

impl std::ops::Not for KleeneValue {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
}

impl std::ops::BitAnd for KleeneValue {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }
}

impl std::ops::BitOr for KleeneValue {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_conversion() {
        assert_eq!(KleeneValue::from(true), KleeneValue::True);
        assert_eq!(KleeneValue::from(false), KleeneValue::False);
    }

    #[test]
    fn not_operator() {
        assert_eq!(!KleeneValue::True, KleeneValue::False);
        assert_eq!(!KleeneValue::False, KleeneValue::True);
        assert_eq!(!KleeneValue::Unknown, KleeneValue::Unknown);
    }

    #[test]
    fn and_truth_table() {
        assert_eq!(KleeneValue::True & KleeneValue::True, KleeneValue::True);
        assert_eq!(KleeneValue::True & KleeneValue::False, KleeneValue::False);
        assert_eq!(KleeneValue::True & KleeneValue::Unknown, KleeneValue::Unknown);
        assert_eq!(KleeneValue::False & KleeneValue::Unknown, KleeneValue::False);
        assert_eq!(KleeneValue::Unknown & KleeneValue::Unknown, KleeneValue::Unknown);
    }

    #[test]
    fn or_truth_table() {
        assert_eq!(KleeneValue::False | KleeneValue::False, KleeneValue::False);
        assert_eq!(KleeneValue::False | KleeneValue::True, KleeneValue::True);
        assert_eq!(KleeneValue::False | KleeneValue::Unknown, KleeneValue::Unknown);
        assert_eq!(KleeneValue::True | KleeneValue::Unknown, KleeneValue::True);
    }

    #[test]
    fn any_iterator() {
        let vals = [1, 2, 3];
        assert_eq!(KleeneValue::any(&vals, |&v| (v == 2).into()), KleeneValue::True);
        assert_eq!(KleeneValue::any(&vals, |&v| (v == 5).into()), KleeneValue::False);
        assert_eq!(
            KleeneValue::any(&vals, |&v| if v == 2 { KleeneValue::Unknown } else { KleeneValue::False }),
            KleeneValue::Unknown
        );
    }

    #[test]
    fn all_iterator() {
        let vals = [2, 4, 6];
        assert_eq!(KleeneValue::all(&vals, |&v| (v % 2 == 0).into()), KleeneValue::True);
        assert_eq!(KleeneValue::all(&vals, |&v| (v > 3).into()), KleeneValue::False);
    }
}
