//! Per-node event listener storage.
//!
//! Chrome's `EventListenerMap`: `Vec<(AtomicString, Vec<Listener>)>` with
//! inline capacity of 2. Linear scan — most nodes have 0-3 event types.
//!
//! Kozan uses `TypeId` instead of strings for type-safe event matching.
//! Same linear scan, same performance characteristics.

use core::any::TypeId;

use super::event::Event;
use super::listener::{ListenerId, ListenerOptions, RegisteredListener};

/// Per-node event listener map.
///
/// Stores listeners grouped by event type. Linear scan — optimized for
/// the typical case of 0-3 event types per node.
///
/// # Chrome equivalence
///
/// Chrome: `HeapVector<pair<AtomicString, Vec<RegisteredEventListener>>, 2>`
/// Kozan: `Vec<(TypeId, Vec<RegisteredListener>)>` (same structure, typed)
#[derive(Default)]
pub struct EventListenerMap {
    entries: Vec<(TypeId, Vec<RegisteredListener>)>,
}

impl EventListenerMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a listener for a specific event type. Returns its ID.
    pub fn add<E: Event>(
        &mut self,
        callback: super::listener::EventCallback,
        options: ListenerOptions,
    ) -> ListenerId {
        let type_id = TypeId::of::<E>();
        let id = ListenerId::next();

        let listeners = match self.entries.iter_mut().find(|(t, _)| *t == type_id) {
            Some((_, listeners)) => listeners,
            None => {
                self.entries.push((type_id, Vec::new()));
                &mut self.entries.last_mut().expect("just pushed").1
            }
        };

        listeners.push(RegisteredListener {
            id,
            event_type: type_id,
            callback,
            options,
            removed: false,
        });

        id
    }

    /// Remove a listener by its ID. Returns true if found.
    ///
    /// During dispatch, this sets the tombstone flag instead of removing.
    /// Chrome does the same to prevent iterator invalidation.
    pub fn remove(&mut self, id: ListenerId) -> bool {
        for (_, listeners) in &mut self.entries {
            if let Some(listener) = listeners.iter_mut().find(|l| l.id == id) {
                listener.removed = true;
                return true;
            }
        }
        false
    }

    /// Remove all listeners for a node (used when destroying a node).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get listeners for a specific event type.
    ///
    /// Returns None if no listeners are registered for this type.
    #[must_use]
    pub fn get(&self, type_id: TypeId) -> Option<&Vec<RegisteredListener>> {
        self.entries
            .iter()
            .find(|(t, _)| *t == type_id)
            .map(|(_, listeners)| listeners)
    }

    /// Take listeners for dispatch (take-call-put pattern).
    ///
    /// Removes the listener vec from the map temporarily.
    /// Call `put()` to return them after dispatch.
    pub fn take(&mut self, type_id: TypeId) -> Option<Vec<RegisteredListener>> {
        if let Some(pos) = self.entries.iter().position(|(t, _)| *t == type_id) {
            let (_, listeners) = &mut self.entries[pos];
            if listeners.is_empty() {
                return None;
            }
            Some(core::mem::take(listeners))
        } else {
            None
        }
    }

    /// Put listeners back after dispatch.
    ///
    /// Cleans up tombstoned (removed) listeners and `once` listeners.
    pub fn put(&mut self, type_id: TypeId, mut listeners: Vec<RegisteredListener>) {
        // Clean up tombstones and once-fired listeners.
        listeners.retain(|l| !l.removed);

        if let Some((_, existing)) = self.entries.iter_mut().find(|(t, _)| *t == type_id) {
            // Merge back: existing may have new listeners added during dispatch.
            let new_during_dispatch = core::mem::take(existing);
            *existing = listeners;
            existing.extend(new_during_dispatch);
        } else if !listeners.is_empty() {
            self.entries.push((type_id, listeners));
        }
    }

    /// Does this node have any listeners at all?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|(_, l)| l.is_empty())
    }

    /// Does this node have listeners for a specific event type?
    #[must_use]
    pub fn has_listeners(&self, type_id: TypeId) -> bool {
        self.entries
            .iter()
            .any(|(t, l)| *t == type_id && !l.is_empty())
    }

    /// Does this node have any capturing listeners for a specific event type?
    #[must_use]
    pub fn has_capture_listeners(&self, type_id: TypeId) -> bool {
        self.entries
            .iter()
            .any(|(t, l)| *t == type_id && l.iter().any(|r| r.options.capture && !r.removed))
    }

    /// Total number of registered listeners (across all types).
    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.iter().map(|(_, l)| l.len()).sum()
    }
}
