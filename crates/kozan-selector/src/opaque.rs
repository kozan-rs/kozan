//! Type-safe element identity for caching and comparison.
//!
//! When the selector engine needs to store element references as cache keys
//! (NthIndexCache, HasCache) or compare identity (`:scope`), it can't borrow
//! the Element — it needs an owned, hashable, comparable identity token.
//!
//! **Stylo's approach**: `NonNull<()>` — requires unsafe, raw pointer casts,
//! and is tied to a specific memory model (pointer-based DOM).
//!
//! **Our approach**: `OpaqueElement(u64)` — a plain value type. The DOM
//! implementation chooses what identity to expose (pointer, index, ID, etc.)
//! via `Element::opaque()`. No unsafe, no raw pointers, works with any DOM
//! representation (arena-allocated, ECS, pointer-based, etc.).

/// Opaque, owned element identity — safe to store, compare, and hash.
///
/// Created via `Element::opaque()`. Two `OpaqueElement` values are equal
/// if and only if they refer to the same DOM element.
///
/// The inner `u64` is chosen because:
/// - Fits a pointer on both 32-bit and 64-bit platforms
/// - Fits an arena index + generation counter
/// - Fits an ECS entity ID
/// - Copy, no allocation, no lifetime
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpaqueElement(u64);

impl OpaqueElement {
    /// Create from a raw u64 identity value.
    ///
    /// DOM implementations should produce a value that is unique and stable
    /// for the lifetime of the element. Pointers (cast to u64), arena indices,
    /// or ECS entity IDs all work.
    #[inline]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Create from a pointer. Convenience for pointer-based DOM implementations.
    #[inline]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self(ptr as u64)
    }

    /// Returns the raw identity value.
    #[inline]
    pub fn raw(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality() {
        assert_eq!(OpaqueElement::new(42), OpaqueElement::new(42));
        assert_ne!(OpaqueElement::new(1), OpaqueElement::new(2));
    }

    #[test]
    fn from_pointer() {
        let x = 42u32;
        let a = OpaqueElement::from_ptr(&x as *const u32);
        let b = OpaqueElement::from_ptr(&x as *const u32);
        assert_eq!(a, b);
    }

    #[test]
    fn hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(OpaqueElement::new(1));
        set.insert(OpaqueElement::new(2));
        set.insert(OpaqueElement::new(1));
        assert_eq!(set.len(), 2);
    }
}
