//! Keyboard DOM events — Chrome: `blink/core/dom/events/keyboard_event.h`.
//!
//! DOM-level keyboard events dispatched through the tree.
//! The `EventHandler` converts `input::KeyboardEvent` → `KeyDownEvent` / `KeyUpEvent`.

use crate::input::{KeyCode, Modifiers};
use kozan_macros::Event;

/// DOM `keydown` event — fired when a key is pressed.
///
/// Chrome: `KeyboardEvent` with type `"keydown"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles, cancelable)]
#[non_exhaustive]
pub struct KeyDownEvent {
    /// Physical key code — which key was pressed.
    pub key: KeyCode,
    /// Modifier keys held during the key press.
    pub modifiers: Modifiers,
    /// Text input produced by this key press (if any).
    pub text: Option<String>,
}

/// DOM `keyup` event — fired when a key is released.
///
/// Chrome: `KeyboardEvent` with type `"keyup"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles)]
#[non_exhaustive]
pub struct KeyUpEvent {
    /// Physical key code — which key was released.
    pub key: KeyCode,
    /// Modifier keys held during the key release.
    pub modifiers: Modifiers,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Bubbles, Cancelable, Event};

    #[test]
    fn keydown_bubbles_and_cancelable() {
        let evt = KeyDownEvent {
            key: KeyCode::Enter,
            modifiers: Modifiers::EMPTY,
            text: Some("\n".to_string()),
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::Yes);
    }

    #[test]
    fn keyup_bubbles_not_cancelable() {
        let evt = KeyUpEvent {
            key: KeyCode::KeyA,
            modifiers: Modifiers::EMPTY,
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }

    #[test]
    fn keydown_as_any_downcast() {
        let evt = KeyDownEvent {
            key: KeyCode::KeyC,
            modifiers: Modifiers::EMPTY.with_ctrl(),
            text: None,
        };
        let any = evt.as_any();
        let downcasted = any.downcast_ref::<KeyDownEvent>().unwrap();
        assert_eq!(downcasted.key, KeyCode::KeyC);
        assert!(downcasted.modifiers.ctrl());
    }
}
