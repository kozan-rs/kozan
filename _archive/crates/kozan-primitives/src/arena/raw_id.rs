//! Raw generational identifier — the foundation of arena identity.
//!
//! `RawId` is 8 bytes: `(index: u32, generation: u32)`.
//! It is `Copy + Send + Sync` — safe to pass anywhere.
//!
//! When a slot is freed, the generation is bumped. Any `RawId` holding
//! the old generation becomes **stale** — detected in O(1) by the arena.
//!
//! Chrome equivalent: slot indices with generation counters in arena-based
//! allocators.

use core::fmt;

/// Sentinel value meaning "no link" (empty free-list, no parent, etc.).
pub const INVALID: u32 = u32::MAX;

/// A raw generational identifier. 8 bytes, `Copy`, `Send + Sync`.
///
/// Carries no pointer and no methods beyond identity — just an index
/// and a generation. Type-safe wrappers (via [`Arena`](super::Arena))
/// prevent mixing IDs from different arenas.
///
/// # Usage
///
/// ```
/// use kozan_primitives::arena::RawId;
///
/// let id = RawId::new(0, 0);
/// assert_eq!(id.index(), 0);
/// assert_eq!(id.generation(), 0);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct RawId {
    index: u32,
    generation: u32,
}

impl RawId {
    /// Create a new `RawId` with the given index and generation.
    #[inline]
    #[must_use]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// The slot index in the arena.
    #[inline]
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// The generation counter (for stale-handle detection).
    #[inline]
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Debug for RawId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RawId({}v{})", self.index, self.generation)
    }
}

impl fmt::Display for RawId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_accessors() {
        let id = RawId::new(5, 3);
        assert_eq!(id.index(), 5);
        assert_eq!(id.generation(), 3);
    }

    #[test]
    fn copy_and_eq() {
        let a = RawId::new(1, 2);
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn different_generation_not_equal() {
        let a = RawId::new(1, 0);
        let b = RawId::new(1, 1);
        assert_ne!(a, b);
    }

    #[test]
    fn debug_format() {
        let id = RawId::new(3, 7);
        assert_eq!(format!("{:?}", id), "RawId(3v7)");
    }

    #[test]
    fn display_format() {
        let id = RawId::new(3, 7);
        assert_eq!(format!("{}", id), "3v7");
    }

    #[test]
    fn hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(RawId::new(0, 0));
        set.insert(RawId::new(0, 1));
        set.insert(RawId::new(1, 0));
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RawId>();
    }
}
