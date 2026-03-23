//! Input event types — the engine's public input API.
//!
//! Like Chrome's `blink/public/common/input/` — these types define the
//! contract between the platform layer and the engine. The platform converts
//! OS events (winit) into these types, then passes them to `FrameWidget`.
//!
//! # Architecture
//!
//! ```text
//! Layer 0: winit events          (OS abstraction)
//! Layer 1: input::InputEvent     (THIS MODULE — engine's public API)
//!          Crosses the main→view thread boundary.
//!          Self-contained data, no pointers, no handles.
//! Layer 2: DOM Event             (events/ module — Event, MouseEvent, KeyboardEvent)
//!          Created by EventHandler from InputEvent.
//!          Spec-compliant, dispatched through the DOM tree.
//! ```
//!
//! # Why these types live in the engine (not the platform)
//!
//! Chrome defines `WebInputEvent` in blink (the engine), not in content (the
//! platform). This prevents circular dependencies: the engine defines the
//! types, the platform produces them, and the engine consumes them.

pub(crate) mod default_action;
pub mod keyboard;
pub mod modifiers;
pub mod mouse;
pub mod wheel;

// Re-export all types at the `input` level.
pub use keyboard::{KeyCode, KeyboardEvent};
pub use modifiers::Modifiers;
pub use mouse::{
    ButtonState, MouseButton, MouseButtonEvent, MouseEnterEvent, MouseLeaveEvent, MouseMoveEvent,
};
pub use wheel::{WheelDelta, WheelEvent};

/// An input event — the engine's entry point for user interaction.
///
/// Chrome equivalent: `WebInputEvent` with a `Type` enum.
/// In Rust, an enum of dedicated structs replaces a class hierarchy.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Mouse cursor moved within the view.
    MouseMove(MouseMoveEvent),
    /// Mouse button pressed or released.
    MouseButton(MouseButtonEvent),
    /// Mouse cursor entered the view.
    MouseEnter(MouseEnterEvent),
    /// Mouse cursor left the view.
    MouseLeave(MouseLeaveEvent),
    /// Mouse wheel or trackpad scroll.
    Wheel(WheelEvent),
    /// Keyboard key pressed or released.
    Keyboard(KeyboardEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_event_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<InputEvent>();
    }

    #[test]
    fn input_event_variants() {
        let evt = InputEvent::MouseMove(MouseMoveEvent {
            x: 10.0,
            y: 20.0,
            modifiers: Modifiers::EMPTY,
            timestamp: std::time::Instant::now(),
        });
        assert!(matches!(evt, InputEvent::MouseMove(_)));

        let evt = InputEvent::Keyboard(KeyboardEvent {
            key: KeyCode::Enter,
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY,
            text: None,
            timestamp: std::time::Instant::now(),
        });
        assert!(matches!(evt, InputEvent::Keyboard(_)));
    }
}
