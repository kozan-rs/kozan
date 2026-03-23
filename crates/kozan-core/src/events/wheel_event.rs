//! Wheel DOM event — Chrome: `blink/core/dom/events/wheel_event.h`.
//!
//! DOM-level wheel event dispatched through the tree.
//! The `EventHandler` converts `input::WheelEvent` -> `events::WheelEvent`.
//!
//! Note: shares its name with `input::WheelEvent` (the platform-level type).
//! Use the module path to disambiguate when both are in scope.

use crate::input::Modifiers;
use kozan_macros::Event;

/// DOM `wheel` event — fired on mouse wheel or trackpad scroll.
///
/// Chrome: `WheelEvent` with type `"wheel"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct WheelEvent {
    /// Cursor X position in CSS pixels.
    pub x: f32,
    /// Cursor Y position in CSS pixels.
    pub y: f32,
    /// Horizontal scroll delta in CSS pixels.
    pub delta_x: f32,
    /// Vertical scroll delta in CSS pixels.
    pub delta_y: f32,
    /// Modifier keys held during the scroll.
    pub modifiers: Modifiers,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Bubbles, Cancelable, Event};

    #[test]
    fn wheel_event_bubbles_and_cancelable() {
        let evt = WheelEvent {
            x: 100.0,
            y: 200.0,
            delta_x: 0.0,
            delta_y: -120.0,
            modifiers: Modifiers::EMPTY,
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::Yes);
    }

    #[test]
    fn wheel_event_as_any_downcast() {
        let evt = WheelEvent {
            x: 50.0,
            y: 75.0,
            delta_x: 10.0,
            delta_y: -20.0,
            modifiers: Modifiers::EMPTY.with_shift(),
        };
        let any = evt.as_any();
        let downcasted = any.downcast_ref::<WheelEvent>().unwrap();
        assert_eq!(downcasted.delta_y, -20.0);
        assert!(downcasted.modifiers.shift());
    }
}
