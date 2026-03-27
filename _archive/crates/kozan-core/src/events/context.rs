//! Event dispatch context — mutable state during dispatch.
//!
//! Chrome puts phase, propagation flags, `current_target` all on the Event object.
//! In Kozan, this state lives in `EventContext` (passed to handlers alongside the event).
//! This keeps Event structs immutable and simple.

use core::cell::Cell;

/// The current phase of event dispatch.
///
/// Matches Chrome's `Event::PhaseType` and DOM spec values.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    None = 0,
    Capturing = 1,
    AtTarget = 2,
    Bubbling = 3,
}

/// Mutable dispatch state passed to event handlers.
///
/// Uses `Cell` for interior mutability — handlers can call `stop_propagation()`
/// etc. through a shared reference. Safe because dispatch is single-threaded.
///
/// # Chrome equivalence
///
/// | Chrome field                       | Kozan field                |
/// |------------------------------------|----------------------------|
/// | `event_phase_`                     | `phase`                    |
/// | `propagation_stopped_`             | `propagation_stopped`      |
/// | `immediate_propagation_stopped_`   | `immediate_stopped`        |
/// | `default_prevented_`               | `default_prevented`        |
/// | `current_target_`                  | `current_target`           |
/// | `target_`                          | `target`                   |
pub struct EventContext {
    phase: Cell<Phase>,
    propagation_stopped: Cell<bool>,
    immediate_stopped: Cell<bool>,
    default_prevented: Cell<bool>,
    target: u32,
    current_target: Cell<u32>,
}

impl EventContext {
    pub(crate) fn new(target: u32) -> Self {
        Self {
            phase: Cell::new(Phase::None),
            propagation_stopped: Cell::new(false),
            immediate_stopped: Cell::new(false),
            default_prevented: Cell::new(false),
            target,
            current_target: Cell::new(target),
        }
    }

    /// The target node (where the event was originally dispatched).
    #[inline]
    pub fn target(&self) -> u32 {
        self.target
    }

    /// The node currently being processed in the dispatch path.
    #[inline]
    pub fn current_target(&self) -> u32 {
        self.current_target.get()
    }

    /// The current dispatch phase.
    #[inline]
    pub fn phase(&self) -> Phase {
        self.phase.get()
    }

    /// Stop propagation to subsequent nodes.
    /// Remaining listeners on the current node still fire.
    /// (Chrome: `stopPropagation()`)
    pub fn stop_propagation(&self) {
        self.propagation_stopped.set(true);
    }

    /// Stop propagation AND prevent remaining listeners on the current node.
    /// (Chrome: `stopImmediatePropagation()`)
    pub fn stop_immediate_propagation(&self) {
        self.propagation_stopped.set(true);
        self.immediate_stopped.set(true);
    }

    /// Prevent the default action for this event.
    /// Does NOT stop propagation — all listeners still fire.
    /// (Chrome: `preventDefault()`)
    pub fn prevent_default(&self) {
        self.default_prevented.set(true);
    }

    /// Was `stop_propagation()` or `stop_immediate_propagation()` called?
    #[inline]
    pub fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped.get()
    }

    /// Was `stop_immediate_propagation()` called?
    #[inline]
    pub fn is_immediate_stopped(&self) -> bool {
        self.immediate_stopped.get()
    }

    /// Was `prevent_default()` called?
    #[inline]
    pub fn is_default_prevented(&self) -> bool {
        self.default_prevented.get()
    }

    // Internal — used by dispatcher.
    pub(crate) fn set_phase(&self, phase: Phase) {
        self.phase.set(phase);
    }

    pub(crate) fn set_current_target(&self, index: u32) {
        self.current_target.set(index);
    }
}
