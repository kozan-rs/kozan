//! Keyboard event types.
//!
//! Like Chrome's `WebKeyboardEvent` — a dedicated struct carrying key code,
//! modifiers, text input, repeat state, and timestamp.
//!
//! Chrome equivalent: `third_party/blink/public/common/input/web_keyboard_event.h`

use std::time::Instant;

use super::modifiers::Modifiers;
use super::mouse::ButtonState;

/// Keyboard key press or release event.
///
/// Chrome equivalent: `WebKeyboardEvent`.
///
/// # Fields
///
/// - `key`: Physical key code (which key on the keyboard).
///   Chrome: `dom_code` (physical) + `dom_key` (logical).
///   We start with physical only — logical keys added when IME support lands.
///
/// - `state`: Pressed or Released.
///   Chrome: encoded in the event type (`kKeyDown` vs `kKeyUp`).
///
/// - `modifiers`: Includes `is_auto_repeat` flag for held keys.
///   Chrome: `modifiers_ & kIsAutoRepeat`.
///
/// - `text`: The character(s) produced by this key press, if any.
///   Chrome: `WebKeyboardEvent::text[4]` (UTF-16).
///   We use `Option<String>` (UTF-8) — Rust native.
///
/// - `timestamp`: When this event was received from the OS.
///   Chrome: `WebInputEvent::time_stamp_`.
#[derive(Debug, Clone)]
pub struct KeyboardEvent {
    /// Physical key code — which key was pressed.
    pub key: KeyCode,
    /// Whether the key was pressed or released.
    pub state: ButtonState,
    /// Modifier keys and auto-repeat flag.
    pub modifiers: Modifiers,
    /// Text input produced by this key press (if any).
    /// None for modifier keys, function keys, etc.
    pub text: Option<String>,
    /// When this event was received from the OS.
    pub timestamp: Instant,
}

/// Physical key code — identifies which key on the keyboard.
///
/// Chrome equivalent: `ui::DomCode` (physical key position) mapped from
/// `ui::KeyboardCode` (Windows virtual key code).
///
/// Starts minimal — extended incrementally as needed. Chrome has hundreds
/// of key codes; we add them as features require them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // ---- Letters ----
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,

    // ---- Digits ----
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // ---- Function keys ----
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // ---- Navigation ----
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,

    // ---- Editing ----
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Space,

    // ---- Modifiers (as physical keys) ----
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    SuperLeft,
    SuperRight,

    // ---- Not yet mapped ----
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_event_with_text() {
        let evt = KeyboardEvent {
            key: KeyCode::KeyA,
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY,
            text: Some("a".to_string()),
            timestamp: Instant::now(),
        };
        assert_eq!(evt.key, KeyCode::KeyA);
        assert_eq!(evt.text.as_deref(), Some("a"));
    }

    #[test]
    fn keyboard_event_auto_repeat() {
        let evt = KeyboardEvent {
            key: KeyCode::KeyA,
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY.with_auto_repeat(),
            text: Some("a".to_string()),
            timestamp: Instant::now(),
        };
        assert!(evt.modifiers.is_auto_repeat());
    }

    #[test]
    fn keyboard_event_with_modifiers() {
        let evt = KeyboardEvent {
            key: KeyCode::KeyC,
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY.with_ctrl(),
            text: None,
            timestamp: Instant::now(),
        };
        assert!(evt.modifiers.ctrl());
        assert!(evt.text.is_none());
    }

    #[test]
    fn modifier_key_has_no_text() {
        let evt = KeyboardEvent {
            key: KeyCode::ShiftLeft,
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY.with_shift(),
            text: None,
            timestamp: Instant::now(),
        };
        assert_eq!(evt.key, KeyCode::ShiftLeft);
        assert!(evt.text.is_none());
    }
}
