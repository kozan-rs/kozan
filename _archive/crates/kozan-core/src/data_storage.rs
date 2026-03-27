// Type-erased column storage for element-specific data.
//
// Each element type (Button, TextInput, custom elements) gets its own typed
// column. The columns are indexed by the same u32 slot index as all other
// storages.
//
// The type erasure uses `dyn Any` downcasting. The TypeId lookup in the HashMap
// is always with a compile-time constant key (monomorphized), so LLVM can often
// optimize it aggressively.

use core::any::{Any, TypeId};
use core::mem::MaybeUninit;
use std::collections::HashMap;

/// Type-erased column interface.
///
/// Each concrete column is a `TypedColumn<T>` that implements this trait.
trait AnyColumn: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Drop the value at the given index (if occupied).
    fn drop_at(&mut self, index: usize);

    /// Ensure the backing vec has at least `len` slots.
    #[allow(dead_code)]
    fn ensure_len(&mut self, len: usize);
}

/// A typed column storing one element data type.
struct TypedColumn<T: 'static> {
    slots: Vec<MaybeUninit<T>>,
    /// Track which slots are initialized (for safe drop).
    occupied: Vec<bool>,
}

impl<T: 'static> TypedColumn<T> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            occupied: Vec::new(),
        }
    }

    fn ensure_len(&mut self, len: usize) {
        if len > self.slots.len() {
            self.slots.reserve(len - self.slots.len());
            self.occupied.reserve(len - self.occupied.len());
            while self.slots.len() < len {
                self.slots.push(MaybeUninit::uninit());
                self.occupied.push(false);
            }
        }
    }

    fn set(&mut self, index: usize, value: T) {
        self.ensure_len(index + 1);
        // Drop old value if occupied.
        if self.occupied[index] {
            unsafe {
                self.slots[index].assume_init_drop();
            }
        }
        self.slots[index] = MaybeUninit::new(value);
        self.occupied[index] = true;
    }

    /// # Safety: slot must be occupied.
    #[inline]
    unsafe fn get(&self, index: usize) -> &T {
        debug_assert!(index < self.occupied.len() && self.occupied[index]);
        unsafe { self.slots.get_unchecked(index).assume_init_ref() }
    }

    /// # Safety: slot must be occupied.
    #[inline]
    unsafe fn get_mut(&mut self, index: usize) -> &mut T {
        debug_assert!(index < self.occupied.len() && self.occupied[index]);
        unsafe { self.slots.get_unchecked_mut(index).assume_init_mut() }
    }

    fn remove(&mut self, index: usize) {
        if index < self.occupied.len() && self.occupied[index] {
            unsafe {
                self.slots[index].assume_init_drop();
            }
            self.occupied[index] = false;
        }
    }
}

impl<T: 'static> AnyColumn for TypedColumn<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn drop_at(&mut self, index: usize) {
        self.remove(index);
    }

    fn ensure_len(&mut self, len: usize) {
        TypedColumn::ensure_len(self, len);
    }
}

impl<T: 'static> Drop for TypedColumn<T> {
    fn drop(&mut self) {
        // Drop all occupied slots.
        for i in 0..self.occupied.len() {
            if self.occupied[i] {
                unsafe {
                    self.slots[i].assume_init_drop();
                }
            }
        }
    }
}

/// Registry of type-erased columns, one per element data type.
///
/// Keyed by `TypeId` of the data type (e.g., `TypeId::of::<ButtonData>()`).
/// Typically holds 20–50 entries (one per element type in the app).
pub(crate) struct DataStorage {
    columns: HashMap<TypeId, Box<dyn AnyColumn>>,
}

impl DataStorage {
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
        }
    }

    /// Initialize a slot with the default value. Creates the column if needed.
    pub fn init<D: Default + 'static>(&mut self, index: u32) {
        let type_id = TypeId::of::<D>();
        let column = self
            .columns
            .entry(type_id)
            .or_insert_with(|| Box::new(TypedColumn::<D>::new()));
        let typed = column
            .as_any_mut()
            .downcast_mut::<TypedColumn<D>>()
            .expect("DataStorage type mismatch");
        typed.set(index as usize, D::default());
    }

    /// Read a reference to the data at the given index.
    ///
    /// # Safety
    /// - The slot at `index` must be alive and initialized for type `D`.
    /// - The caller must not hold a mutable reference to the same column.
    #[inline]
    pub unsafe fn get<D: 'static>(&self, index: u32) -> &D {
        unsafe {
            let column = self.columns.get(&TypeId::of::<D>()).unwrap_unchecked();
            let typed = column
                .as_any()
                .downcast_ref::<TypedColumn<D>>()
                .unwrap_unchecked();
            typed.get(index as usize)
        }
    }

    /// Get a mutable reference to the data at the given index.
    ///
    /// # Safety
    /// - The slot at `index` must be alive and initialized for type `D`.
    /// - The caller must have exclusive access.
    #[inline]
    pub unsafe fn get_mut<D: 'static>(&mut self, index: u32) -> &mut D {
        unsafe {
            let column = self.columns.get_mut(&TypeId::of::<D>()).unwrap_unchecked();
            let typed = column
                .as_any_mut()
                .downcast_mut::<TypedColumn<D>>()
                .unwrap_unchecked();
            typed.get_mut(index as usize)
        }
    }

    /// Remove and drop the data at the given index for the given type.
    pub fn remove(&mut self, type_id: TypeId, index: u32) {
        if let Some(column) = self.columns.get_mut(&type_id) {
            column.drop_at(index as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_init_and_read() {
        let mut ds = DataStorage::new();
        ds.init::<String>(0);
        ds.init::<String>(1);

        unsafe {
            assert_eq!(ds.get::<String>(0), "");
            assert_eq!(ds.get::<String>(1), "");
        }
    }

    #[test]
    fn write_and_read_back() {
        let mut ds = DataStorage::new();
        ds.init::<String>(0);

        unsafe {
            *ds.get_mut::<String>(0) = "hello".to_string();
            assert_eq!(ds.get::<String>(0), "hello");
        }
    }

    #[test]
    fn multiple_types() {
        let mut ds = DataStorage::new();
        ds.init::<String>(0);
        ds.init::<u32>(0);

        unsafe {
            *ds.get_mut::<String>(0) = "text".to_string();
            *ds.get_mut::<u32>(0) = 42;

            assert_eq!(ds.get::<String>(0), "text");
            assert_eq!(*ds.get::<u32>(0), 42);
        }
    }

    #[test]
    fn remove_drops_value() {
        use std::sync::atomic::{AtomicU32, Ordering};

        static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

        #[derive(Default)]
        struct Tracked(#[allow(dead_code)] String);
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }

        let mut ds = DataStorage::new();
        DROP_COUNT.store(0, Ordering::Relaxed);

        ds.init::<Tracked>(0);
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);

        ds.remove(TypeId::of::<Tracked>(), 0);
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 1);
    }
}
