//! Mouse event types — dedicated structs per event kind.
//!
//! Like Chrome's `WebMouseEvent` — each mouse event is a standalone struct
//! carrying all the context needed for dispatch (position, button, modifiers,
//! timestamp).
//!
//! Chrome equivalent files:
//! - `third_party/blink/public/common/input/web_mouse_event.h`
//! - `ui/events/event.h` (`ui::MouseEvent`)

use std::time::Instant;

use super::modifiers::Modifiers;

/// Mouse cursor moved within the view.
///
/// Chrome equivalent: `WebMouseEvent` with type `kMouseMove`.
///
/// Position is in physical pixels (f64) relative to the view's top-left corner.
/// Use `ViewContext::scale_factor()` to convert to logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct MouseMoveEvent {
    /// Cursor X position in physical pixels, relative to view origin.
    pub x: f64,
    /// Cursor Y position in physical pixels, relative to view origin.
    pub y: f64,
    /// Modifier keys and mouse button state at the time of this event.
    pub modifiers: Modifiers,
    /// When this event was received from the OS.
    /// Chrome equivalent: `WebInputEvent::time_stamp_`.
    pub timestamp: Instant,
}

/// Mouse button pressed or released.
///
/// Chrome equivalent: `WebMouseEvent` with type `kMouseDown` / `kMouseUp`.
///
/// Always carries the correct cursor position — the `AppHandler` tracks
/// cursor position and attaches it (like Chrome's `InputRouterImpl`).
#[derive(Debug, Clone, Copy)]
pub struct MouseButtonEvent {
    /// Cursor X position in physical pixels.
    pub x: f64,
    /// Cursor Y position in physical pixels.
    pub y: f64,
    /// Which button was pressed or released.
    pub button: MouseButton,
    /// Whether the button was pressed or released.
    pub state: ButtonState,
    /// Modifier keys and mouse button state at the time of this event.
    pub modifiers: Modifiers,
    /// Number of rapid clicks (1 = single click, 2 = double click, etc.).
    /// Chrome tracks this in `WebMouseEvent::click_count`.
    pub click_count: u8,
    /// When this event was received from the OS.
    pub timestamp: Instant,
}

/// Mouse cursor entered the view area.
///
/// Chrome equivalent: `WebMouseEvent` with type `kMouseEnter`.
#[derive(Debug, Clone, Copy)]
pub struct MouseEnterEvent {
    pub x: f64,
    pub y: f64,
    pub modifiers: Modifiers,
    pub timestamp: Instant,
}

/// Mouse cursor left the view area.
///
/// Chrome equivalent: `WebMouseEvent` with type `kMouseLeave`.
#[derive(Debug, Clone, Copy)]
pub struct MouseLeaveEvent {
    pub modifiers: Modifiers,
    pub timestamp: Instant,
}

/// Mouse button identifier.
///
/// Chrome equivalent: `WebPointerProperties::Button`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

/// Button press/release state.
///
/// Used by both mouse and keyboard events.
/// Chrome equivalent: part of the event type (`kMouseDown` vs `kMouseUp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonState {
    Pressed,
    Released,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_move_event_carries_modifiers_and_timestamp() {
        let evt = MouseMoveEvent {
            x: 100.0,
            y: 200.0,
            modifiers: Modifiers::EMPTY.with_ctrl(),
            timestamp: Instant::now(),
        };
        assert!(evt.modifiers.ctrl());
        assert!(!evt.modifiers.shift());
    }

    #[test]
    fn mouse_button_event_has_click_count() {
        let evt = MouseButtonEvent {
            x: 50.0,
            y: 75.0,
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY,
            click_count: 2,
            timestamp: Instant::now(),
        };
        assert_eq!(evt.click_count, 2);
        assert_eq!(evt.button, MouseButton::Left);
    }

    #[test]
    fn mouse_button_equality() {
        assert_eq!(MouseButton::Left, MouseButton::Left);
        assert_ne!(MouseButton::Left, MouseButton::Right);
        assert_eq!(MouseButton::Other(4), MouseButton::Other(4));
        assert_ne!(MouseButton::Other(4), MouseButton::Other(5));
    }

    #[test]
    fn positions_are_f64() {
        let evt = MouseMoveEvent {
            x: 1920.123456789,
            y: 1080.987654321,
            modifiers: Modifiers::EMPTY,
            timestamp: Instant::now(),
        };
        // f64 preserves this precision, f32 would not.
        assert!((evt.x - 1920.123456789).abs() < 1e-9);
        assert!((evt.y - 1080.987654321).abs() < 1e-9);
    }
}
