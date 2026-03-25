// DocumentCell — The single point of unsafe in the entire system.
//
// ALL raw pointer operations on Document go through here. Handle and
// element types access Document through the `read()` and `write()` closures.
//
// Safety model:
//   1. NonNull is valid: Document is heap-allocated at a stable address
//   2. Single-threaded: Handle is Send+!Sync (see handle.rs for why Send is safe)
//   3. No aliasing: read()/write() create and drop borrows within the closure
//   4. Generational IDs: dead nodes return None, never UB on valid memory

use core::ptr::NonNull;

use crate::dom::document::Document;

/// Safe accessor for Document through a raw pointer.
///
/// Only four methods: `new`, `check_alive`, `read`, `write`.
/// All domain logic lives on `Document` itself.
/// This is the ONLY type in the system that dereferences raw pointers.
#[derive(Copy, Clone)]
pub(crate) struct DocumentCell(NonNull<Document>);

impl DocumentCell {
    #[inline]
    pub fn new(ptr: NonNull<Document>) -> Self {
        Self(ptr)
    }

    /// In debug builds, panic if the Document has been dropped.
    #[inline]
    pub fn check_alive(&self) {
        #[cfg(debug_assertions)]
        unsafe {
            debug_assert!(
                (*self.0.as_ptr()).is_alive_debug(),
                "Handle used after Document was dropped"
            );
        }
    }

    /// Safe read access. The reference cannot escape the closure.
    #[inline]
    pub(crate) fn read<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
        let doc = unsafe { &*self.0.as_ptr() };
        f(doc)
    }

    /// Safe mutable access. The reference cannot escape the closure.
    /// Single-threaded — no concurrent access.
    #[inline]
    pub(crate) fn write<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
        let doc = unsafe { &mut *self.0.as_ptr() };
        f(doc)
    }

    /// Raw pointer access for Stylo's `TElement::ensure_data()`.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// 1. The Document is alive and at a stable address.
    /// 2. No mutable reference to the same Document field exists concurrently.
    /// 3. The pointer is not used after the Document is dropped.
    ///
    /// This exists because Stylo's `ensure_data(&self) -> ElementDataMut<'_>`
    /// requires returning a borrow that outlives any closure. The closure-based
    /// `read()`/`write()` API cannot express this lifetime.
    #[inline]
    pub(crate) unsafe fn as_ptr(&self) -> *mut Document {
        self.0.as_ptr()
    }
}
