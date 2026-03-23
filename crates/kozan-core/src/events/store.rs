//! Per-document event listener storage.
//!
//! Uses `Storage<Option<Box<EventListenerMap>>>` — the same tested parallel
//! storage used by tree, meta, and element data. One data structure everywhere.

use core::any::TypeId;

use super::listener::RegisteredListener;
use super::listener_map::EventListenerMap;
use kozan_primitives::arena::Storage;

/// Per-document event listener storage.
///
/// One slot per node. Most slots are `None`.
/// Only nodes with listeners get a `Box<EventListenerMap>`.
pub(crate) struct EventStore {
    slots: Storage<Option<Box<EventListenerMap>>>,
}

impl EventStore {
    pub fn new() -> Self {
        Self {
            slots: Storage::new(),
        }
    }

    /// Ensure a slot exists and is initialized to `None`.
    pub fn ensure_slot(&mut self, index: u32) {
        if !self.slots.is_initialized(index) {
            self.slots.set(index, None);
        }
    }

    /// Get or create the listener map for a node (lazy allocation).
    pub fn ensure_listeners(&mut self, index: u32) -> &mut EventListenerMap {
        self.ensure_slot(index);
        let slot = unsafe { self.slots.get_unchecked_mut(index) };
        slot.get_or_insert_with(|| Box::new(EventListenerMap::new()))
    }

    /// Get the listener map for a node, if it exists.
    #[allow(dead_code)]
    pub fn get(&self, index: u32) -> Option<&EventListenerMap> {
        if !self.slots.is_initialized(index) {
            return None;
        }
        unsafe { self.slots.get_unchecked(index).as_deref() }
    }

    /// Get mutable access to a node's listener map, if it exists.
    pub fn get_mut(&mut self, index: u32) -> Option<&mut EventListenerMap> {
        if !self.slots.is_initialized(index) {
            return None;
        }
        unsafe { self.slots.get_unchecked_mut(index).as_deref_mut() }
    }

    /// Take listeners for dispatch (take-call-put pattern).
    pub fn take(&mut self, index: u32, type_id: TypeId) -> Option<Vec<RegisteredListener>> {
        self.get_mut(index)?.take(type_id)
    }

    /// Put listeners back after dispatch.
    pub fn put(&mut self, index: u32, type_id: TypeId, listeners: Vec<RegisteredListener>) {
        if let Some(map) = self.get_mut(index) {
            map.put(type_id, listeners);
        }
    }

    /// Check if a node has any event listeners.
    #[allow(dead_code)]
    pub fn has_listeners(&self, index: u32) -> bool {
        self.get(index).is_some_and(|m| !m.is_empty())
    }

    /// Remove all listeners for a node (called on node destroy).
    pub fn remove_node(&mut self, index: u32) {
        if self.slots.is_initialized(index) {
            self.slots.clear_slot(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::event::{Bubbles, Cancelable, Event};
    use crate::events::listener::ListenerOptions;

    struct TestEvent;
    impl Event for TestEvent {
        fn bubbles(&self) -> Bubbles {
            Bubbles::No
        }
        fn cancelable(&self) -> Cancelable {
            Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    #[test]
    fn new_store_is_empty() {
        let store = EventStore::new();
        assert!(!store.has_listeners(0));
        assert!(store.get(0).is_none());
    }

    #[test]
    fn ensure_slot_initializes() {
        let mut store = EventStore::new();
        store.ensure_slot(5);
        assert!(store.slots.is_initialized(5));
        assert!(store.get(5).is_none()); // None, not Some
    }

    #[test]
    fn ensure_listeners_creates_map() {
        let mut store = EventStore::new();
        let map = store.ensure_listeners(3);
        assert!(map.is_empty());
        assert!(store.get(3).is_some());
    }

    #[test]
    fn add_and_check_listener() {
        let mut store = EventStore::new();
        let map = store.ensure_listeners(0);
        let callback: crate::events::listener::EventCallback = Box::new(|_, _| {});
        map.add::<TestEvent>(callback, ListenerOptions::default());
        assert!(store.has_listeners(0));
    }

    #[test]
    fn take_and_put() {
        let mut store = EventStore::new();
        let map = store.ensure_listeners(0);
        let callback: crate::events::listener::EventCallback = Box::new(|_, _| {});
        map.add::<TestEvent>(callback, ListenerOptions::default());

        let type_id = TypeId::of::<TestEvent>();
        let taken = store.take(0, type_id);
        assert!(taken.is_some());
        assert_eq!(taken.as_ref().unwrap().len(), 1);

        assert!(store.take(0, type_id).is_none());

        store.put(0, type_id, taken.unwrap());
        assert!(store.has_listeners(0));
    }

    #[test]
    fn remove_node_clears_listeners() {
        let mut store = EventStore::new();
        let map = store.ensure_listeners(2);
        let callback: crate::events::listener::EventCallback = Box::new(|_, _| {});
        map.add::<TestEvent>(callback, ListenerOptions::default());

        store.remove_node(2);
        assert!(!store.has_listeners(2));
    }

    #[test]
    fn out_of_bounds_is_safe() {
        let store = EventStore::new();
        assert!(store.get(999).is_none());
        assert!(!store.has_listeners(999));
    }

    #[test]
    fn multiple_nodes_independent() {
        let mut store = EventStore::new();

        let map0 = store.ensure_listeners(0);
        map0.add::<TestEvent>(Box::new(|_, _| {}), ListenerOptions::default());

        let map1 = store.ensure_listeners(1);
        map1.add::<TestEvent>(Box::new(|_, _| {}), ListenerOptions::default());

        store.remove_node(0);
        assert!(!store.has_listeners(0));
        assert!(store.has_listeners(1));
    }
}
