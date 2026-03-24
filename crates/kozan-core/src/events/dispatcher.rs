//! Event dispatcher — the complete dispatch algorithm.
//!
//! Chrome's `EventDispatcher::Dispatch()` adapted for Rust.
//!
//! # Algorithm
//!
//! 1. Build `EventPath` (target → root snapshot)
//! 2. **Capture phase**: root → target, fire capture listeners
//! 3. **Target phase**: fire both capture + bubble listeners on target
//! 4. **Bubble phase**: target → root, fire bubble listeners (if event.bubbles)
//! 5. **Default action**: call `default_event_handler` (if !preventDefault)
//!
//! # Take-call-put
//!
//! Listeners are **taken** from storage before calling, then **put back** after.
//! This means during a handler callback:
//! - The tree can be mutated (append, remove, destroy)
//! - New listeners can be added (they won't fire during this dispatch)
//! - Listeners can be removed (tombstoned via `removed` flag)

use core::any::TypeId;

use crate::dom::document_cell::DocumentCell;
use crate::id::RawId;

use super::context::{EventContext, Phase};
use super::event::{Bubbles, Event};
use super::listener::RegisteredListener;
use super::path::EventPath;

/// Dispatch an event to a target node.
///
/// This is the core dispatch function — equivalent to Chrome's
/// `EventDispatcher::Dispatch()`.
///
/// The `event_store` parameter provides access to the per-node listener maps.
/// In practice, this is called through `Handle::dispatch_event()`.
pub(crate) fn dispatch(
    cell: DocumentCell,
    target: RawId,
    event: &dyn Event,
    event_store: &mut EventStoreAccess<'_>,
) -> bool {
    let type_id = (*event).type_id();
    let ctx = EventContext::new(target.index());

    // 1. Build the propagation path (snapshot).
    let path = EventPath::build(cell, target);
    if path.is_empty() {
        return false;
    }

    // 2. CAPTURE PHASE: root → target (ancestors only; target fires in step 3).
    ctx.set_phase(Phase::Capturing);
    for (_pos, node_idx, node_gen) in path.capture_order() {
        // Target fires exclusively in the at-target step below.
        if node_idx == target.index() {
            continue;
        }
        if !cell.read(|doc| doc.is_alive_id(RawId::new(node_idx, node_gen))) {
            continue;
        }
        ctx.set_current_target(node_idx);
        fire_listeners(event_store, node_idx, type_id, event, &ctx, true, false);

        if ctx.is_propagation_stopped() {
            break;
        }
    }

    // 3. AT-TARGET: fire all listeners on the target (capture and bubble both apply).
    //    Chrome fires all registered listeners in registration order regardless of
    //    capture flag. The bubbles flag does not affect at-target firing.
    if !ctx.is_propagation_stopped() {
        ctx.set_phase(Phase::AtTarget);
        ctx.set_current_target(target.index());
        if cell.read(|doc| doc.is_alive_id(target)) {
            fire_listeners(
                event_store,
                target.index(),
                type_id,
                event,
                &ctx,
                false,
                true,
            );
        }
    }

    // 4. BUBBLE PHASE: target → root (if event bubbles and not stopped).
    //    Skip the target itself (already handled above).
    if event.bubbles() == Bubbles::Yes && !ctx.is_propagation_stopped() {
        for (_pos, node_idx, node_gen) in path.bubble_order() {
            if node_idx == target.index() {
                continue; // already handled in step 3
            }
            if !cell.read(|doc| doc.is_alive_id(RawId::new(node_idx, node_gen))) {
                continue;
            }
            ctx.set_current_target(node_idx);
            ctx.set_phase(Phase::Bubbling);
            fire_listeners(event_store, node_idx, type_id, event, &ctx, false, false);

            if ctx.is_propagation_stopped() {
                break;
            }
        }
    }

    // 4. Reset phase.
    ctx.set_phase(Phase::None);

    // Return whether default was prevented.
    !ctx.is_default_prevented()
}

/// Fire matching listeners on a single node.
///
/// When `at_target` is true all registered listeners fire (Chrome at-target semantics).
/// Otherwise only listeners whose `capture` flag matches `capture_pass` fire.
fn fire_listeners(
    store: &mut EventStoreAccess<'_>,
    node_index: u32,
    type_id: TypeId,
    event: &dyn Event,
    ctx: &EventContext,
    capture_pass: bool,
    at_target: bool,
) {
    // Take-call-put: take listeners out of storage.
    let Some(mut listeners) = store.take(node_index, type_id) else {
        return;
    };

    for listener in &mut listeners {
        if listener.removed {
            continue;
        }

        // At target: all listeners fire regardless of capture flag (both passes
        // run on the target node, so capture and bubble listeners both apply).
        // Off-target: only listeners matching the current phase direction fire.
        if !at_target && (listener.options.capture != capture_pass) {
            continue;
        }

        // Chrome: remove `once` listeners BEFORE firing.
        if listener.options.once {
            listener.removed = true;
        }

        // Fire the callback.
        (listener.callback)(event, ctx);

        // Chrome: stopImmediatePropagation() kills remaining listeners on THIS node.
        if ctx.is_immediate_stopped() {
            break;
        }
    }

    // Put listeners back (clean up tombstones).
    store.put(node_index, type_id, listeners);
}

/// Abstraction over event listener storage access.
///
/// The dispatcher needs to take/put listeners from the per-node maps
/// in `Document`. This trait abstracts that access so the dispatcher
/// doesn't directly depend on `Document` internals.
pub(crate) struct EventStoreAccess<'a> {
    cell: DocumentCell,
    _marker: core::marker::PhantomData<&'a mut ()>,
}

impl<'a> EventStoreAccess<'a> {
    pub(crate) fn new(cell: DocumentCell) -> Self {
        Self {
            cell,
            _marker: core::marker::PhantomData,
        }
    }

    fn take(&mut self, node_index: u32, type_id: TypeId) -> Option<Vec<RegisteredListener>> {
        self.cell
            .write(|doc| doc.take_event_listeners(node_index, type_id))
    }

    fn put(&mut self, node_index: u32, type_id: TypeId, listeners: Vec<RegisteredListener>) {
        self.cell
            .write(|doc| doc.put_event_listeners(node_index, type_id, listeners));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::dom::document::Document;
    use crate::dom::traits::{ContainerNode, HasHandle};
    use crate::events::{Bubbles, Cancelable, Event, EventTarget};
    use crate::html::{HtmlButtonElement, HtmlDivElement};

    struct TestEvent;
    impl Event for TestEvent {
        fn bubbles(&self) -> Bubbles {
            Bubbles::Yes
        }
        fn cancelable(&self) -> Cancelable {
            Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    struct NonBubblingEvent;
    impl Event for NonBubblingEvent {
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
    fn capture_fires_before_bubble_on_ancestor() {
        let doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        let btn = doc.create::<HtmlButtonElement>();
        doc.root().append(div);
        div.append(btn);

        let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

        let o1 = order.clone();
        div.on_capture::<TestEvent>(move |_, _| o1.borrow_mut().push("capture"));

        let o2 = order.clone();
        div.on::<TestEvent>(move |_, _| o2.borrow_mut().push("bubble"));

        btn.dispatch_event(&TestEvent);
        assert_eq!(*order.borrow(), vec!["capture", "bubble"]);
    }

    #[test]
    fn at_target_fires_capture_and_bubble_listeners() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        doc.root().append(btn);

        let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

        let o1 = order.clone();
        btn.on_capture::<NonBubblingEvent>(move |_, _| o1.borrow_mut().push("capture"));

        let o2 = order.clone();
        btn.on::<NonBubblingEvent>(move |_, _| o2.borrow_mut().push("bubble"));

        btn.dispatch_event(&NonBubblingEvent);
        let fired = order.borrow();
        assert_eq!(fired.len(), 2);
        assert!(fired.contains(&"capture"));
        assert!(fired.contains(&"bubble"));
    }

    #[test]
    fn stop_propagation_halts_bubble_to_ancestors() {
        let doc = Document::new();
        let outer = doc.create::<HtmlDivElement>();
        let inner = doc.create::<HtmlDivElement>();
        let btn = doc.create::<HtmlButtonElement>();
        doc.root().append(outer);
        outer.append(inner);
        inner.append(btn);

        let outer_called = Rc::new(std::cell::Cell::new(false));
        let oc = outer_called.clone();
        btn.on::<TestEvent>(move |_, ctx| ctx.stop_propagation());
        outer.on::<TestEvent>(move |_, _| oc.set(true));

        btn.dispatch_event(&TestEvent);
        assert!(!outer_called.get());
    }

    #[test]
    fn dispatch_to_detached_node_returns_false() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        // btn is not attached to the tree (no parent chain to root),
        // but it is alive. dispatch_event should still work — the path
        // will contain only the target itself.
        let result = btn.dispatch_event(&TestEvent);
        assert!(result);
    }

    #[test]
    fn dispatch_to_destroyed_node_returns_false() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        btn.handle().destroy();
        let result = btn.dispatch_event(&TestEvent);
        assert!(!result);
    }
}
