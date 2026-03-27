//! Event trait and dispatch properties.
//!
//! Unlike Chrome where `Event` is a class carrying dispatch state,
//! in Kozan the event data is separate from dispatch state (`EventContext`).
//! This lets event structs be simple data carriers — no mutable dispatch fields.

use core::any::Any;

/// Whether an event bubbles up through the tree.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Bubbles {
    Yes,
    No,
}

/// Whether an event can be cancelled via `prevent_default()`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cancelable {
    Yes,
    No,
}

/// Base trait for all events.
///
/// Every event type (`MouseEvent`, `KeyboardEvent`, custom events) implements this.
/// The trait is object-safe — events are dispatched as `&dyn Event`.
///
/// # Chrome equivalence
///
/// Chrome stores `bubbles_`, `cancelable_`, `type_` on the Event object.
/// In Kozan, `bubbles()` and `cancelable()` are trait methods (usually const),
/// and the type is identified via `TypeId` (no string matching).
pub trait Event: Any + 'static {
    /// Whether this event bubbles up through the tree after reaching the target.
    fn bubbles(&self) -> Bubbles;

    /// Whether this event can be cancelled via `prevent_default()`.
    fn cancelable(&self) -> Cancelable;

    /// Downcast to `&dyn Any` for type-safe casting in handlers.
    fn as_any(&self) -> &dyn Any;
}
