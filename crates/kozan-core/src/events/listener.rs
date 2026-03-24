//! Event listeners — registration, storage, and options.
//!
//! Chrome's `RegisteredEventListener`: callback + packed bitfield options.
//! Kozan: `RegisteredListener` with `Box<dyn FnMut>` callback + options.

use core::any::TypeId;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::EventContext;
use super::event::Event;

/// Type alias for event listener callbacks.
///
/// Chrome: `EventListener::handleEvent()`.
/// Type-erased: receives `&dyn Event` (the concrete event is downcast by the caller).
pub type EventCallback = Box<dyn FnMut(&dyn Event, &EventContext)>;

/// Opaque handle for removing a listener.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(pub(crate) u64);

/// Global listener ID counter.
static NEXT_LISTENER_ID: AtomicU64 = AtomicU64::new(1);

impl ListenerId {
    pub(crate) fn next() -> Self {
        Self(NEXT_LISTENER_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Options for `addEventListener`.
///
/// Matches Chrome's `AddEventListenerOptionsResolved`.
#[derive(Copy, Clone, Debug, Default)]
pub struct ListenerOptions {
    /// If true, fires during the capture phase instead of bubble phase.
    pub capture: bool,
    /// If true, `prevent_default()` has no effect (enables compositor optimizations).
    pub passive: bool,
    /// If true, automatically removed after first invocation.
    pub once: bool,
}

impl ListenerOptions {
    #[must_use]
    pub fn capture() -> Self {
        Self {
            capture: true,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn passive() -> Self {
        Self {
            passive: true,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn once() -> Self {
        Self {
            once: true,
            ..Default::default()
        }
    }
}

/// A registered event listener.
///
/// Chrome's `RegisteredEventListener`: callback + bitfield flags.
/// In Kozan: type-erased `FnMut(&dyn Event, &EventContext)` + options.
pub struct RegisteredListener {
    pub(crate) id: ListenerId,
    #[allow(dead_code)]
    pub(crate) event_type: TypeId,
    pub(crate) callback: EventCallback,
    pub(crate) options: ListenerOptions,
    /// Tombstone flag (Chrome pattern). Marked removed during iteration,
    /// actually cleaned up later. Prevents iterator invalidation.
    pub(crate) removed: bool,
}
