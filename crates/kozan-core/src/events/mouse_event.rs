//! Mouse DOM events — Chrome: `blink/core/dom/events/mouse_event.h`.
//!
//! These are DOM-level events dispatched through the tree (capture -> target -> bubble).
//! NOT the platform-level input structs from `input::mouse` (which carry `Instant`,
//! physical coords, and are produced by the OS).
//!
//! The `EventHandler` converts `input::MouseButtonEvent` -> `MouseDownEvent` etc.
//!
//! # Naming convention
//!
//! All DOM events use clean W3C-standard names. Where a name collides with a
//! platform-level type in `input::` (e.g., `MouseMoveEvent`), the module path
//! disambiguates: `events::MouseMoveEvent` vs `input::MouseMoveEvent`.

use crate::input::{Modifiers, MouseButton};
use kozan_macros::Event;

/// DOM `click` event — fired after mousedown + mouseup on the same target.
///
/// Chrome: `MouseEvent` with type `"click"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct ClickEvent {
    /// X position in CSS pixels, relative to viewport origin.
    pub x: f32,
    /// Y position in CSS pixels, relative to viewport origin.
    pub y: f32,
    /// Which button triggered this click.
    pub button: MouseButton,
    /// Modifier keys held during the click.
    pub modifiers: Modifiers,
}

/// DOM `dblclick` event — fired on double-click.
///
/// Chrome: `MouseEvent` with type `"dblclick"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct DblClickEvent {
    pub x: f32,
    pub y: f32,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}

/// DOM `mousedown` event — fired when a button is pressed.
///
/// Chrome: `MouseEvent` with type `"mousedown"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct MouseDownEvent {
    pub x: f32,
    pub y: f32,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}

/// DOM `mouseup` event — fired when a button is released.
///
/// Chrome: `MouseEvent` with type `"mouseup"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct MouseUpEvent {
    pub x: f32,
    pub y: f32,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}

/// DOM `mousemove` event — fired when the cursor moves over an element.
///
/// Chrome: `MouseEvent` with type `"mousemove"`.
///
/// Note: shares its name with `input::MouseMoveEvent` (the platform-level type).
/// Use the module path to disambiguate when both are in scope.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct MouseMoveEvent {
    pub x: f32,
    pub y: f32,
    pub modifiers: Modifiers,
}

/// DOM `mouseenter` event — fired when the cursor enters an element.
/// Does NOT bubble (unlike `MouseOverEvent`).
///
/// Chrome: `MouseEvent` with type `"mouseenter"`.
#[derive(Debug, Clone, Event)]
#[event()]
#[non_exhaustive]
pub struct MouseEnterEvent {
    pub x: f32,
    pub y: f32,
    pub modifiers: Modifiers,
}

/// DOM `mouseleave` event — fired when the cursor leaves an element.
/// Does NOT bubble (unlike `MouseOutEvent`).
///
/// Chrome: `MouseEvent` with type `"mouseleave"`.
/// Always carries the last known cursor position (like Chrome's `clientX`/`clientY`).
#[derive(Debug, Clone, Event)]
#[event()]
#[non_exhaustive]
pub struct MouseLeaveEvent {
    /// Last cursor X position when leaving.
    pub x: f32,
    /// Last cursor Y position when leaving.
    pub y: f32,
    pub modifiers: Modifiers,
}

/// DOM `mouseover` event — fired when the cursor enters an element or its children.
/// Bubbles (unlike `MouseEnterEvent`).
///
/// Chrome: `MouseEvent` with type `"mouseover"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles)]
#[non_exhaustive]
pub struct MouseOverEvent {
    pub x: f32,
    pub y: f32,
    pub modifiers: Modifiers,
}

/// DOM `mouseout` event — fired when the cursor leaves an element or enters a child.
/// Bubbles (unlike `MouseLeaveEvent`).
///
/// Chrome: `MouseEvent` with type `"mouseout"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles)]
#[non_exhaustive]
pub struct MouseOutEvent {
    pub x: f32,
    pub y: f32,
    pub modifiers: Modifiers,
}

/// DOM `contextmenu` event — fired on right-click.
///
/// Chrome: `MouseEvent` with type `"contextmenu"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct ContextMenuEvent {
    pub x: f32,
    pub y: f32,
    pub modifiers: Modifiers,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Bubbles, Cancelable, Event};

    #[test]
    fn click_event_properties() {
        let evt = ClickEvent {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            modifiers: Modifiers::EMPTY,
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::Yes);
        assert_eq!(evt.x, 10.0);
    }

    #[test]
    fn mouseenter_does_not_bubble() {
        let evt = MouseEnterEvent {
            x: 0.0,
            y: 0.0,
            modifiers: Modifiers::EMPTY,
        };
        assert_eq!(evt.bubbles(), Bubbles::No);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }

    #[test]
    fn mouseleave_carries_position() {
        let evt = MouseLeaveEvent {
            x: 150.0,
            y: 200.0,
            modifiers: Modifiers::EMPTY,
        };
        assert_eq!(evt.bubbles(), Bubbles::No);
        assert_eq!(evt.x, 150.0);
        assert_eq!(evt.y, 200.0);
    }

    #[test]
    fn mouseover_bubbles() {
        let evt = MouseOverEvent {
            x: 0.0,
            y: 0.0,
            modifiers: Modifiers::EMPTY,
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }

    #[test]
    fn context_menu_bubbles_and_cancelable() {
        let evt = ContextMenuEvent {
            x: 50.0,
            y: 75.0,
            modifiers: Modifiers::EMPTY.with_ctrl(),
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::Yes);
    }

    #[test]
    fn event_as_any_downcast() {
        let evt = ClickEvent {
            x: 1.0,
            y: 2.0,
            button: MouseButton::Left,
            modifiers: Modifiers::EMPTY,
        };
        let any = evt.as_any();
        let downcasted = any.downcast_ref::<ClickEvent>().unwrap();
        assert_eq!(downcasted.x, 1.0);
        assert_eq!(downcasted.y, 2.0);
    }
}
