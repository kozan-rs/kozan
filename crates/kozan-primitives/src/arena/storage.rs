//! Parallel typed storage — one Vec per data kind, indexed by slot.
//!
//! This is the "column" in the `SoA` (Struct of Arrays) layout.
//! Each `Storage<T>` holds one aspect of all entries: data, styles, etc.
//!
//! # Safety model
//!
//! Uses `MaybeUninit<T>` for performance (no default initialization of gaps).
//! Tracks which slots are initialized via a `Vec<bool>` bitmap.
//! - `get()`/`get_mut()` check initialization and return `Option`.
//! - `get_unchecked()`/`get_unchecked_mut()` are unsafe — caller ensures initialized.
//! - `set()` properly drops old values on overwrite.
//! - `Drop` drops all initialized values.

use core::mem::MaybeUninit;

/// A typed parallel storage indexed by `u32` slot indices.
///
/// Managed by [`IdAllocator`](super::IdAllocator) — this storage does not
/// decide which slots are alive. It only tracks which slots have been initialized.
///
/// # Performance
///
/// - `set()`: O(1) amortized (may grow the Vec).
/// - `get()`: O(1) — array index + initialized check.
/// - Cache-friendly: contiguous memory, linear access patterns.
pub struct Storage<T> {
    slots: Vec<MaybeUninit<T>>,
    initialized: Vec<bool>,
}

impl<T> Storage<T> {
    /// Create a new empty storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            initialized: Vec::new(),
        }
    }

    /// Number of slots (including uninitialized gaps).
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the storage has zero slots.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Grow to fit `index + 1` slots. New slots are uninitialized.
    pub fn grow(&mut self, index: u32) {
        let needed = index as usize + 1;
        if needed > self.slots.len() {
            self.slots.reserve(needed - self.slots.len());
            self.initialized.reserve(needed - self.initialized.len());
            while self.slots.len() < needed {
                self.slots.push(MaybeUninit::uninit());
                self.initialized.push(false);
            }
        }
    }

    /// Write a value at the given index. Grows if needed.
    /// Drops the old value if the slot was already initialized.
    #[inline]
    pub fn set(&mut self, index: u32, value: T) {
        self.grow(index);
        let i = index as usize;
        if self.initialized[i] {
            unsafe {
                self.slots[i].assume_init_drop();
            }
        }
        self.slots[i] = MaybeUninit::new(value);
        self.initialized[i] = true;
    }

    /// Check if a slot has been initialized.
    #[inline]
    #[must_use]
    pub fn is_initialized(&self, index: u32) -> bool {
        let i = index as usize;
        i < self.initialized.len() && self.initialized[i]
    }

    /// Safe read — returns `None` if not initialized or out of bounds.
    #[inline]
    #[must_use]
    pub fn get(&self, index: u32) -> Option<&T> {
        if !self.is_initialized(index) {
            return None;
        }
        Some(unsafe { self.slots.get_unchecked(index as usize).assume_init_ref() })
    }

    /// Safe mutable read — returns `None` if not initialized or out of bounds.
    #[inline]
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        if !self.is_initialized(index) {
            return None;
        }
        Some(unsafe {
            self.slots
                .get_unchecked_mut(index as usize)
                .assume_init_mut()
        })
    }

    /// Unchecked read — caller must ensure the slot is initialized.
    ///
    /// # Safety
    ///
    /// The slot at `index` must have been initialized via `set()`.
    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked(&self, index: u32) -> &T {
        debug_assert!(
            self.is_initialized(index),
            "Storage::get_unchecked on uninitialized slot {index}"
        );
        unsafe { self.slots.get_unchecked(index as usize).assume_init_ref() }
    }

    /// Unchecked mutable read — caller must ensure the slot is initialized.
    ///
    /// # Safety
    ///
    /// The slot at `index` must have been initialized via `set()`.
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, index: u32) -> &mut T {
        debug_assert!(
            self.is_initialized(index),
            "Storage::get_unchecked_mut on uninitialized slot {index}"
        );
        unsafe {
            self.slots
                .get_unchecked_mut(index as usize)
                .assume_init_mut()
        }
    }

    /// Remove the value at the given index and return it.
    /// Returns `None` if not initialized.
    pub fn take(&mut self, index: u32) -> Option<T> {
        if !self.is_initialized(index) {
            return None;
        }
        let i = index as usize;
        self.initialized[i] = false;
        Some(unsafe { self.slots.get_unchecked(i).assume_init_read() })
    }

    /// Drop all initialized values and reset to empty.
    pub fn clear(&mut self) {
        for i in 0..self.initialized.len() {
            if self.initialized[i] {
                unsafe {
                    self.slots[i].assume_init_drop();
                }
                self.initialized[i] = false;
            }
        }
    }

    /// Iterate over all initialized `(index, &value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
        self.initialized
            .iter()
            .enumerate()
            .filter_map(move |(i, &init)| {
                if init {
                    // SAFETY: initialized[i] is true, so the slot was written via set().
                    Some((i as u32, unsafe { self.slots[i].assume_init_ref() }))
                } else {
                    None
                }
            })
    }

    /// Mark a slot as uninitialized and drop its value.
    /// Safe to call on already-uninitialized slots (no-op).
    pub fn clear_slot(&mut self, index: u32) {
        let i = index as usize;
        if i < self.initialized.len() && self.initialized[i] {
            unsafe {
                self.slots[i].assume_init_drop();
            }
            self.initialized[i] = false;
        }
    }
}

impl<T> Default for Storage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for Storage<T> {
    fn clone(&self) -> Self {
        let mut new = Self::new();
        for (id, val) in self.iter() {
            new.set(id, val.clone());
        }
        new
    }
}

impl<T> Drop for Storage<T> {
    fn drop(&mut self) {
        for i in 0..self.initialized.len() {
            if self.initialized[i] {
                unsafe {
                    self.slots[i].assume_init_drop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let s = Storage::<u32>::new();
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert!(!s.is_initialized(0));
    }

    #[test]
    fn set_and_get() {
        let mut s = Storage::new();
        s.set(0, 42u32);
        assert!(s.is_initialized(0));
        assert_eq!(s.get(0), Some(&42));
    }

    #[test]
    fn get_uninitialized_returns_none() {
        let mut s = Storage::<u32>::new();
        s.grow(5);
        assert_eq!(s.get(2), None);
        assert_eq!(s.get_mut(2), None);
    }

    #[test]
    fn set_grows_storage() {
        let mut s = Storage::new();
        s.set(5, 99u32);
        assert_eq!(s.len(), 6);
        assert!(!s.is_initialized(0));
        assert!(!s.is_initialized(4));
        assert_eq!(s.get(5), Some(&99));
    }

    #[test]
    fn set_overwrites() {
        let mut s = Storage::new();
        s.set(0, String::from("old"));
        s.set(0, String::from("new"));
        assert_eq!(s.get(0).map(|s| s.as_str()), Some("new"));
    }

    #[test]
    fn get_mut_modifies() {
        let mut s = Storage::new();
        s.set(0, 10u32);
        *s.get_mut(0).unwrap() = 20;
        assert_eq!(s.get(0), Some(&20));
    }

    #[test]
    fn take_removes() {
        let mut s = Storage::new();
        s.set(0, String::from("hello"));
        let val = s.take(0);
        assert_eq!(val.as_deref(), Some("hello"));
        assert!(!s.is_initialized(0));
    }

    #[test]
    fn take_uninitialized_returns_none() {
        let mut s = Storage::<u32>::new();
        s.grow(5);
        assert_eq!(s.take(2), None);
    }

    #[test]
    fn clear_slot_drops_value() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROPS: AtomicU32 = AtomicU32::new(0);

        struct Tracked;
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROPS.store(0, Ordering::Relaxed);
        let mut s = Storage::new();
        s.set(0, Tracked);
        assert_eq!(DROPS.load(Ordering::Relaxed), 0);

        s.clear_slot(0);
        assert_eq!(DROPS.load(Ordering::Relaxed), 1);
        assert!(!s.is_initialized(0));
    }

    #[test]
    fn clear_uninitialized_is_noop() {
        let mut s = Storage::<u32>::new();
        s.grow(5);
        s.clear_slot(3);
    }

    #[test]
    fn drop_cleans_up_all() {
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
            let mut s = Storage::new();
            s.set(0, Tracked);
            s.set(2, Tracked);
            s.set(4, Tracked);
        }
        assert_eq!(DROPS.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn set_overwrite_drops_old() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DROPS: AtomicU32 = AtomicU32::new(0);

        struct Tracked;
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROPS.store(0, Ordering::Relaxed);
        let mut s = Storage::new();
        s.set(0, Tracked);
        assert_eq!(DROPS.load(Ordering::Relaxed), 0);
        s.set(0, Tracked);
        assert_eq!(DROPS.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn iter_yields_initialized_slots() {
        let mut s = Storage::new();
        s.set(0, 10u32);
        s.set(2, 20);
        s.set(4, 30);
        let pairs: Vec<(u32, u32)> = s.iter().map(|(i, &v)| (i, v)).collect();
        assert_eq!(pairs, vec![(0, 10), (2, 20), (4, 30)]);
    }

    #[test]
    fn iter_empty_storage() {
        let s = Storage::<u32>::new();
        assert_eq!(s.iter().count(), 0);
    }

    #[test]
    fn clone_preserves_values() {
        let mut s = Storage::new();
        s.set(0, 10u32);
        s.set(3, 30);
        s.set(5, 50);

        let c = s.clone();
        assert_eq!(c.get(0), Some(&10));
        assert_eq!(c.get(1), None);
        assert_eq!(c.get(3), Some(&30));
        assert_eq!(c.get(5), Some(&50));
    }

    #[test]
    fn clone_is_independent() {
        let mut s = Storage::new();
        s.set(0, String::from("hello"));
        let mut c = s.clone();
        c.set(0, String::from("world"));
        assert_eq!(s.get(0).map(String::as_str), Some("hello"));
        assert_eq!(c.get(0).map(String::as_str), Some("world"));
    }

    #[test]
    #[should_panic(expected = "uninitialized slot")]
    #[cfg(debug_assertions)]
    fn unchecked_get_panics_in_debug() {
        let mut s = Storage::<u32>::new();
        s.grow(5);
        unsafe {
            let _ = s.get_unchecked(2);
        }
    }
}
