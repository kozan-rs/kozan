//! Arena — the high-level safe API combining allocator + storage.
//!
//! `Arena<T>` is the type users interact with. It combines [`IdAllocator`]
//! (slot lifecycle) with [`Storage<T>`] (data storage) into a single
//! type-safe container with O(1) operations and generational safety.
//!
//! # Performance
//!
//! | Operation   | Cost  | `HashMap` equivalent |
//! |-------------|-------|--------------------|
//! | `alloc()`   | O(1)  | `insert()` O(1)*   |
//! | `get()`     | O(1)  | `get()` O(1)*      |
//! | `free()`    | O(1)  | `remove()` O(1)*   |
//! | `is_alive()`| O(1)  | `contains()` O(1)* |
//!
//! \* `HashMap` is O(1) amortized but with ~10-25x higher constant factor
//! due to hashing. Arena uses direct array indexing.
//!
//! # Generational safety
//!
//! ```text
//! let id = arena.alloc(data);
//! arena.free(id);
//! let id2 = arena.alloc(new_data);  // reuses same slot
//! arena.get(id);   // → None (stale — generation mismatch)
//! arena.get(id2);  // → Some(&new_data)
//! ```

use super::allocator::IdAllocator;
use super::raw_id::RawId;
use super::storage::Storage;

/// A generational arena — O(1) alloc, get, free with stale-handle detection.
///
/// Combines [`IdAllocator`] + [`Storage<T>`] into one safe API.
/// No unsafe in user code — the arena manages all invariants internally.
///
/// # Usage
///
/// ```
/// use kozan_primitives::arena::Arena;
///
/// let mut arena = Arena::new();
/// let id = arena.alloc(String::from("hello"));
/// assert_eq!(arena.get(id), Some(&String::from("hello")));
///
/// arena.free(id);
/// assert_eq!(arena.get(id), None); // stale handle
/// ```
pub struct Arena<T> {
    allocator: IdAllocator,
    storage: Storage<T>,
}

impl<T> Arena<T> {
    /// Create a new empty arena.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            allocator: IdAllocator::new(),
            storage: Storage::new(),
        }
    }

    /// Allocate a new entry and store `value`. Returns a [`RawId`].
    ///
    /// O(1) — reuses freed slots, grows only when the free list is empty.
    pub fn alloc(&mut self, value: T) -> RawId {
        let id = self.allocator.alloc();
        self.storage.set(id.index(), value);
        id
    }

    /// Get a reference to the value at `id`.
    ///
    /// Returns `None` if:
    /// - The ID was freed (generation mismatch — stale handle).
    /// - The ID was never allocated.
    ///
    /// O(1) — array index + generation check.
    #[inline]
    #[must_use] 
    pub fn get(&self, id: RawId) -> Option<&T> {
        if !self.allocator.is_alive(id) {
            return None;
        }
        self.storage.get(id.index())
    }

    /// Get a mutable reference to the value at `id`.
    ///
    /// Returns `None` if the ID is stale or was never allocated.
    #[inline]
    pub fn get_mut(&mut self, id: RawId) -> Option<&mut T> {
        if !self.allocator.is_alive(id) {
            return None;
        }
        self.storage.get_mut(id.index())
    }

    /// Free a slot. Returns the removed value, or `None` if already dead.
    ///
    /// After freeing, all existing `RawId`s to this slot become stale.
    /// The slot will be reused by the next `alloc()` with a bumped generation.
    pub fn free(&mut self, id: RawId) -> Option<T> {
        if !self.allocator.free(id) {
            return None;
        }
        self.storage.take(id.index())
    }

    /// Check if an ID is still alive (not freed, correct generation).
    #[inline]
    #[must_use] 
    pub fn is_alive(&self, id: RawId) -> bool {
        self.allocator.is_alive(id)
    }

    /// Number of currently alive entries.
    #[inline]
    #[must_use] 
    pub fn count(&self) -> u32 {
        self.allocator.count()
    }

    /// Whether the arena has no alive entries.
    #[inline]
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.allocator.count() == 0
    }

    /// Total capacity (alive + freed slots, not including unallocated).
    #[inline]
    #[must_use] 
    pub fn capacity(&self) -> usize {
        self.allocator.capacity()
    }

    /// Read a value without generation check.
    ///
    /// # Safety
    ///
    /// The caller must ensure the ID is alive.
    #[inline]
    #[must_use] 
    pub unsafe fn get_unchecked(&self, id: RawId) -> &T {
        unsafe { self.storage.get_unchecked(id.index()) }
    }

    /// Write a value without generation check.
    ///
    /// # Safety
    ///
    /// The caller must ensure the ID is alive.
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, id: RawId) -> &mut T {
        unsafe { self.storage.get_unchecked_mut(id.index()) }
    }

    /// Call a closure on each alive entry (mutable access).
    ///
    /// Iterates all slots, skipping dead ones. O(capacity), but capacity
    /// is typically small (views per window, nodes per document).
    pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for i in 0..self.allocator.capacity() as u32 {
            if let Some(generation) = self.allocator.current_generation(i) {
                let id = RawId::new(i, generation);
                if self.allocator.is_alive(id) {
                    if let Some(val) = self.storage.get_mut(i) {
                        f(val);
                    }
                }
            }
        }
    }

    /// Remove and return all alive entries.
    ///
    /// After drain, the arena is empty. All slots are freed.
    /// Used for clean shutdown (e.g., shutting down all view threads).
    pub fn drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.count() as usize);
        for i in 0..self.allocator.capacity() as u32 {
            if let Some(generation) = self.allocator.current_generation(i) {
                let id = RawId::new(i, generation);
                if let Some(val) = self.free(id) {
                    result.push(val);
                }
            }
        }
        result
    }

    /// Access the underlying allocator (for advanced use cases).
    #[inline]
    #[must_use] 
    pub fn allocator(&self) -> &IdAllocator {
        &self.allocator
    }

    /// Access the underlying storage (for advanced use cases like
    /// parallel column access in kozan-core's `Document`).
    #[inline]
    #[must_use] 
    pub fn storage(&self) -> &Storage<T> {
        &self.storage
    }

    /// Mutable access to the underlying storage.
    #[inline]
    pub fn storage_mut(&mut self) -> &mut Storage<T> {
        &mut self.storage
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_get() {
        let mut arena = Arena::new();
        let id = arena.alloc(42u32);
        assert_eq!(arena.get(id), Some(&42));
        assert_eq!(arena.count(), 1);
    }

    #[test]
    fn get_mut_modifies() {
        let mut arena = Arena::new();
        let id = arena.alloc(10u32);
        *arena.get_mut(id).unwrap() = 20;
        assert_eq!(arena.get(id), Some(&20));
    }

    #[test]
    fn free_returns_value() {
        let mut arena = Arena::new();
        let id = arena.alloc(String::from("hello"));
        let val = arena.free(id);
        assert_eq!(val.as_deref(), Some("hello"));
        assert_eq!(arena.count(), 0);
    }

    #[test]
    fn stale_handle_returns_none() {
        let mut arena = Arena::new();
        let old = arena.alloc(1u32);
        arena.free(old);
        let _new = arena.alloc(2u32);
        // Old handle is stale — different generation.
        assert_eq!(arena.get(old), None);
    }

    #[test]
    fn slot_reuse_with_generation_bump() {
        let mut arena = Arena::new();
        let a = arena.alloc(String::from("first"));
        arena.free(a);
        let b = arena.alloc(String::from("second"));

        // Same index, different generation.
        assert_eq!(a.index(), b.index());
        assert_ne!(a.generation(), b.generation());

        // Old is dead, new is alive.
        assert!(!arena.is_alive(a));
        assert!(arena.is_alive(b));
        assert_eq!(arena.get(b), Some(&String::from("second")));
    }

    #[test]
    fn multiple_entries() {
        let mut arena = Arena::new();
        let a = arena.alloc("alpha");
        let b = arena.alloc("beta");
        let c = arena.alloc("gamma");

        assert_eq!(arena.count(), 3);
        assert_eq!(arena.get(a), Some(&"alpha"));
        assert_eq!(arena.get(b), Some(&"beta"));
        assert_eq!(arena.get(c), Some(&"gamma"));
    }

    #[test]
    fn double_free_returns_none() {
        let mut arena = Arena::new();
        let id = arena.alloc(42);
        assert!(arena.free(id).is_some());
        assert!(arena.free(id).is_none()); // already dead
    }

    #[test]
    fn is_empty() {
        let mut arena = Arena::<u32>::new();
        assert!(arena.is_empty());
        let id = arena.alloc(1);
        assert!(!arena.is_empty());
        arena.free(id);
        assert!(arena.is_empty());
    }

    #[test]
    fn capacity_grows() {
        let mut arena = Arena::new();
        assert_eq!(arena.capacity(), 0);
        arena.alloc(1u32);
        arena.alloc(2u32);
        assert_eq!(arena.capacity(), 2);
    }

    #[test]
    fn drops_on_free() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROPS: AtomicU32 = AtomicU32::new(0);

        struct Tracked;
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROPS.store(0, Ordering::Relaxed);
        let mut arena = Arena::new();
        let id = arena.alloc(Tracked);
        assert_eq!(DROPS.load(Ordering::Relaxed), 0);
        arena.free(id);
        assert_eq!(DROPS.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn drain_returns_all_alive() {
        let mut arena = Arena::new();
        let _a = arena.alloc("alpha");
        let b = arena.alloc("beta");
        let _c = arena.alloc("gamma");
        arena.free(b); // free one

        let drained = arena.drain();
        assert_eq!(drained.len(), 2);
        assert!(drained.contains(&"alpha"));
        assert!(drained.contains(&"gamma"));
        assert!(arena.is_empty());
        assert_eq!(arena.count(), 0);
    }

    #[test]
    fn drain_empty_arena() {
        let mut arena = Arena::<u32>::new();
        let drained = arena.drain();
        assert!(drained.is_empty());
    }

    #[test]
    fn for_each_mut_visits_alive() {
        let mut arena = Arena::new();
        let _a = arena.alloc(1u32);
        let b = arena.alloc(2u32);
        let _c = arena.alloc(3u32);
        arena.free(b);

        let mut sum = 0u32;
        arena.for_each_mut(|val| sum += *val);
        assert_eq!(sum, 4); // 1 + 3, skipping freed slot
    }

    #[test]
    fn drops_all_on_arena_drop() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROPS: AtomicU32 = AtomicU32::new(0);

        struct Tracked;
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROPS.store(0, Ordering::Relaxed);
        {
            let mut arena = Arena::new();
            arena.alloc(Tracked);
            arena.alloc(Tracked);
            arena.alloc(Tracked);
        }
        assert_eq!(DROPS.load(Ordering::Relaxed), 3);
    }
}
