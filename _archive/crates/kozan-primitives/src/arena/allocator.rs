//! Generational free-list allocator.
//!
//! Manages slot lifecycle with generational indices.
//! Every slot gets a `(index, generation)` pair. When freed, the generation
//! is bumped so stale handles become detectable in O(1).
//!
//! # Performance
//!
//! - `alloc()`: O(1) — pops from free list, or grows by 1.
//! - `free()`: O(1) — pushes to free list, bumps generation.
//! - `is_alive()`: O(1) — array index + generation compare.
//! - Memory: 9 bytes per slot (4 gen + 1 alive + 4 `next_free`).

use super::raw_id::{INVALID, RawId};

/// Generational free-list allocator.
///
/// Manages which slots are alive, tracks generations, and recycles freed slots.
/// All parallel storages ([`Storage<T>`](super::Storage)) are indexed by the
/// same `u32` index that this allocator hands out.
///
/// # Invariants
///
/// - A slot's generation is bumped on every `free()`.
/// - `is_alive(index, gen)` returns `true` ONLY if the slot is alive AND
///   the generation matches. Stale handles always return `false`.
/// - Freed slots form a LIFO linked list via `next_free`. This keeps
///   recently-freed slots hot in cache.
pub struct IdAllocator {
    /// Generation counter per slot. Bumped on free.
    generations: Vec<u32>,
    /// Whether each slot is currently occupied.
    alive: Vec<bool>,
    /// Intrusive free list: `next_free[i]` = next free slot after `i`.
    next_free: Vec<u32>,
    /// Head of the free list. `INVALID` = empty.
    free_head: u32,
    /// Number of currently alive entries.
    count: u32,
}

impl IdAllocator {
    /// Create a new empty allocator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            alive: Vec::new(),
            next_free: Vec::new(),
            free_head: INVALID,
            count: 0,
        }
    }

    /// Allocate a new slot. Returns a [`RawId`] with index and generation.
    ///
    /// Reuses freed slots (LIFO) to keep indices dense.
    /// Grows the backing storage only when the free list is empty.
    pub fn alloc(&mut self) -> RawId {
        if self.free_head != INVALID {
            // Reuse a freed slot.
            let index = self.free_head;
            self.free_head = self.next_free[index as usize];
            self.alive[index as usize] = true;
            self.count += 1;
            RawId::new(index, self.generations[index as usize])
        } else {
            // Grow.
            let index = self.generations.len() as u32;
            self.generations.push(0);
            self.alive.push(true);
            self.next_free.push(INVALID);
            self.count += 1;
            RawId::new(index, 0)
        }
    }

    /// Free a slot. Returns `true` if the slot was alive and is now freed.
    ///
    /// Bumps the generation so all existing `RawId`s to this slot become stale.
    pub fn free(&mut self, id: RawId) -> bool {
        if !self.is_alive(id) {
            return false;
        }
        let index = id.index();
        self.alive[index as usize] = false;
        self.generations[index as usize] = id.generation().wrapping_add(1);
        self.next_free[index as usize] = self.free_head;
        self.free_head = index;
        self.count -= 1;
        true
    }

    /// Check if a slot is alive with the given generation.
    #[inline]
    #[must_use]
    pub fn is_alive(&self, id: RawId) -> bool {
        let i = id.index() as usize;
        i < self.generations.len() && self.generations[i] == id.generation() && self.alive[i]
    }

    /// Number of currently alive entries.
    #[inline]
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Total number of slots (alive + dead, not including unallocated).
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.generations.len()
    }

    /// Get the current generation for a slot index.
    ///
    /// Returns `None` if the index is out of bounds.
    #[inline]
    #[must_use]
    pub fn current_generation(&self, index: u32) -> Option<u32> {
        self.generations.get(index as usize).copied()
    }

    /// Get the current generation for a live slot, or `None` if dead/out-of-bounds.
    ///
    /// Used by hit testing: the fragment tree stores only the node index,
    /// this recovers the full `RawId` needed to create a `Handle`.
    #[inline]
    #[must_use]
    pub fn live_generation(&self, index: u32) -> Option<u32> {
        let i = index as usize;
        if i < self.alive.len() && self.alive[i] {
            Some(self.generations[i])
        } else {
            None
        }
    }

    /// Get the current generation for a slot without validation.
    ///
    /// # Safety
    /// The index must be within bounds.
    #[inline]
    #[must_use]
    pub unsafe fn generation_unchecked(&self, index: u32) -> u32 {
        unsafe { *self.generations.get_unchecked(index as usize) }
    }

    /// Check if a slot is alive with the given index and generation values.
    ///
    /// Convenience for callers that have index and generation as separate values.
    #[inline]
    #[must_use]
    pub fn is_alive_raw(&self, index: u32, generation: u32) -> bool {
        self.is_alive(RawId::new(index, generation))
    }
}

impl Default for IdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_returns_sequential_indices() {
        let mut ids = IdAllocator::new();
        let a = ids.alloc();
        let b = ids.alloc();
        let c = ids.alloc();
        assert_eq!(a, RawId::new(0, 0));
        assert_eq!(b, RawId::new(1, 0));
        assert_eq!(c, RawId::new(2, 0));
        assert_eq!(ids.count(), 3);
    }

    #[test]
    fn free_and_reuse() {
        let mut ids = IdAllocator::new();
        let a = ids.alloc();
        let b = ids.alloc();

        assert!(ids.free(a));
        assert!(!ids.is_alive(a));
        assert_eq!(ids.count(), 1);

        // Reallocate — should reuse slot 0 with bumped generation.
        let c = ids.alloc();
        assert_eq!(c.index(), a.index());
        assert_eq!(c.generation(), a.generation() + 1);

        // Old handle is stale.
        assert!(!ids.is_alive(a));
        // New handle is alive.
        assert!(ids.is_alive(c));
        // Slot 1 is still alive.
        assert!(ids.is_alive(b));
    }

    #[test]
    fn double_free_returns_false() {
        let mut ids = IdAllocator::new();
        let a = ids.alloc();
        assert!(ids.free(a));
        assert!(!ids.free(a)); // already dead
    }

    #[test]
    fn stale_handle_after_reuse() {
        let mut ids = IdAllocator::new();
        let old = ids.alloc();
        ids.free(old);
        let new = ids.alloc(); // reuses slot
        assert!(!ids.is_alive(old));
        assert!(ids.is_alive(new));
    }

    #[test]
    fn current_generation() {
        let mut ids = IdAllocator::new();
        let a = ids.alloc();
        assert_eq!(ids.current_generation(a.index()), Some(0));
        ids.free(a);
        assert_eq!(ids.current_generation(a.index()), Some(1)); // bumped
        assert!(ids.current_generation(999).is_none());
    }

    #[test]
    fn capacity_grows() {
        let mut ids = IdAllocator::new();
        assert_eq!(ids.capacity(), 0);
        ids.alloc();
        ids.alloc();
        assert_eq!(ids.capacity(), 2);
    }
}
