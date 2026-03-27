//! `EventTarget` — base trait for anything that can receive events.
//!
//! Like Chrome's `EventTarget` (`core/dom/events/event_target.h`).
//! Both DOM nodes and `Window` implement this.
//!
//! All methods have default implementations.
//! Derives generate `HasHandle` — `EventTarget` is an empty impl.

use super::dispatcher::EventStoreAccess;
use super::{Event, EventContext, ListenerId, ListenerOptions};
use crate::dom::traits::HasHandle;

/// Base trait for anything that can receive events.
///
/// Like Chrome's `EventTarget` class. Both DOM nodes and `Window` implement this.
///
/// # Methods
///
/// | Method              | Description                                      |
/// |---------------------|--------------------------------------------------|
/// | [`on()`](Self::on)            | Add a typed event listener                       |
/// | [`on_capture()`](Self::on_capture)    | Add a capture-phase listener                     |
/// | [`on_once()`](Self::on_once)       | Add a one-shot listener (auto-removed)           |
/// | [`on_with_options()`](Self::on_with_options)| Add with explicit options                       |
/// | [`off()`](Self::off)           | Remove a listener by ID                          |
/// | [`dispatch_event()`](Self::dispatch_event)| Dispatch an event (capture → target → bubble)    |
///
/// # Example
///
/// ```ignore
/// // Add a click listener to a button.
/// btn.on::<ClickEvent>(|event, ctx| {
///     println!("Button clicked!");
/// });
///
/// // Dispatch an event.
/// btn.dispatch_event(&ClickEvent);
/// ```
pub trait EventTarget: HasHandle {
    /// Add a typed event listener. Returns an ID for removal.
    ///
    /// The callback receives `&E` (the concrete event) and `&EventContext`
    /// (dispatch state: phase, target, propagation control).
    fn on<E: Event>(&self, callback: impl FnMut(&E, &EventContext) + 'static) -> ListenerId {
        self.on_with_options::<E>(callback, ListenerOptions::default())
    }

    /// Add a listener with explicit options.
    ///
    /// Options: `capture` (fire during capture phase), `passive` (no preventDefault),
    /// `once` (auto-remove after first call).
    fn on_with_options<E: Event>(
        &self,
        mut callback: impl FnMut(&E, &EventContext) + 'static,
        options: ListenerOptions,
    ) -> ListenerId {
        let h = self.handle();
        let erased: super::listener::EventCallback =
            Box::new(move |event: &dyn Event, ctx: &EventContext| {
                if let Some(typed) = event.as_any().downcast_ref::<E>() {
                    callback(typed, ctx);
                }
            });
        h.cell.write(|doc| {
            let map = doc.ensure_event_listeners(h.id.index());
            map.add::<E>(erased, options)
        })
    }

    /// Add a capture-phase listener.
    ///
    /// Fires during the capture phase (root → target) instead of bubble phase.
    fn on_capture<E: Event>(
        &self,
        callback: impl FnMut(&E, &EventContext) + 'static,
    ) -> ListenerId {
        self.on_with_options::<E>(callback, ListenerOptions::capture())
    }

    /// Add a one-shot listener that auto-removes after first call.
    fn on_once<E: Event>(&self, callback: impl FnMut(&E, &EventContext) + 'static) -> ListenerId {
        self.on_with_options::<E>(callback, ListenerOptions::once())
    }

    /// Remove a listener by its ID.
    fn off(&self, id: ListenerId) {
        let h = self.handle();
        h.cell.write(|doc| {
            if let Some(map) = doc.event_listeners_mut(h.id.index()) {
                map.remove(id);
            }
        });
    }

    /// Dispatch an event to this target.
    ///
    /// Runs the full capture → target → bubble pipeline.
    /// Returns `true` if the default action was NOT prevented.
    fn dispatch_event(&self, event: &dyn Event) -> bool {
        let h = self.handle();
        if !h.is_alive() {
            return false;
        }
        let mut store = EventStoreAccess::new(h.cell);
        super::dispatch(h.cell, h.id, event, &mut store)
    }
}
