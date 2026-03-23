//! Generational arena — O(1) alloc/get/free with stale-handle detection.
//!
//! Shared infrastructure used by every Kozan crate that needs fast,
//! type-safe, generational storage. Like Chrome's `base/` — foundational
//! infrastructure that all subsystems build on.
//!
//! # Types
//!
//! - [`RawId`] — 8-byte generational identifier (index + generation). `Copy`, `Send`, `Sync`.
//! - [`IdAllocator`] — Free-list allocator. Manages slot lifecycle.
//! - [`Storage<T>`] — Parallel typed storage. `MaybeUninit` + initialized bitmap.
//! - [`Arena<T>`] — High-level safe API combining allocator + storage.
//!
//! # Performance vs `HashMap`
//!
//! ```text
//! Arena:   array[index] + generation check  →  ~2ns per lookup
//! HashMap: hash(key) + probe + compare      →  ~20-50ns per lookup
//! ```
//!
//! # Usage
//!
//! ```
//! use kozan_primitives::arena::Arena;
//!
//! let mut arena = Arena::new();
//! let id = arena.alloc(String::from("hello"));
//! assert_eq!(arena.get(id), Some(&String::from("hello")));
//!
//! arena.free(id);
//! assert_eq!(arena.get(id), None); // stale handle
//! ```

pub mod allocator;
#[allow(clippy::module_inception)]
pub mod arena;
pub mod raw_id;
pub mod storage;

pub use allocator::IdAllocator;
pub use arena::Arena;
pub use raw_id::{INVALID, RawId};
pub use storage::Storage;
