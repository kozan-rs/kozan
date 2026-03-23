//! winit → kozan type conversions.
//!
//! Like Chrome's `ui/events/blink/web_input_event.cc` — converts OS events
//! into the engine's input types. This is the ONLY place where winit event
//! types are mapped to kozan types.

use kozan_core::input::*;

pub(crate) fn convert_mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Back,
        winit::event::MouseButton::Forward => MouseButton::Forward,
        winit::event::MouseButton::Other(id) => MouseButton::Other(id),
    }
}

pub(crate) fn convert_button_state(state: winit::event::ElementState) -> ButtonState {
    match state {
        winit::event::ElementState::Pressed => ButtonState::Pressed,
        winit::event::ElementState::Released => ButtonState::Released,
    }
}

pub(crate) fn convert_key_code(key: &winit::keyboard::PhysicalKey) -> KeyCode {
    use winit::keyboard::KeyCode as WKey;
    use winit::keyboard::PhysicalKey;

    match key {
        PhysicalKey::Code(code) => match code {
            WKey::KeyA => KeyCode::KeyA,
            WKey::KeyB => KeyCode::KeyB,
            WKey::KeyC => KeyCode::KeyC,
            WKey::KeyD => KeyCode::KeyD,
            WKey::KeyE => KeyCode::KeyE,
            WKey::KeyF => KeyCode::KeyF,
            WKey::KeyG => KeyCode::KeyG,
            WKey::KeyH => KeyCode::KeyH,
            WKey::KeyI => KeyCode::KeyI,
            WKey::KeyJ => KeyCode::KeyJ,
            WKey::KeyK => KeyCode::KeyK,
            WKey::KeyL => KeyCode::KeyL,
            WKey::KeyM => KeyCode::KeyM,
            WKey::KeyN => KeyCode::KeyN,
            WKey::KeyO => KeyCode::KeyO,
            WKey::KeyP => KeyCode::KeyP,
            WKey::KeyQ => KeyCode::KeyQ,
            WKey::KeyR => KeyCode::KeyR,
            WKey::KeyS => KeyCode::KeyS,
            WKey::KeyT => KeyCode::KeyT,
            WKey::KeyU => KeyCode::KeyU,
            WKey::KeyV => KeyCode::KeyV,
            WKey::KeyW => KeyCode::KeyW,
            WKey::KeyX => KeyCode::KeyX,
            WKey::KeyY => KeyCode::KeyY,
            WKey::KeyZ => KeyCode::KeyZ,
            WKey::Digit0 => KeyCode::Digit0,
            WKey::Digit1 => KeyCode::Digit1,
            WKey::Digit2 => KeyCode::Digit2,
            WKey::Digit3 => KeyCode::Digit3,
            WKey::Digit4 => KeyCode::Digit4,
            WKey::Digit5 => KeyCode::Digit5,
            WKey::Digit6 => KeyCode::Digit6,
            WKey::Digit7 => KeyCode::Digit7,
            WKey::Digit8 => KeyCode::Digit8,
            WKey::Digit9 => KeyCode::Digit9,
            WKey::F1 => KeyCode::F1,
            WKey::F2 => KeyCode::F2,
            WKey::F3 => KeyCode::F3,
            WKey::F4 => KeyCode::F4,
            WKey::F5 => KeyCode::F5,
            WKey::F6 => KeyCode::F6,
            WKey::F7 => KeyCode::F7,
            WKey::F8 => KeyCode::F8,
            WKey::F9 => KeyCode::F9,
            WKey::F10 => KeyCode::F10,
            WKey::F11 => KeyCode::F11,
            WKey::F12 => KeyCode::F12,
            WKey::ArrowUp => KeyCode::ArrowUp,
            WKey::ArrowDown => KeyCode::ArrowDown,
            WKey::ArrowLeft => KeyCode::ArrowLeft,
            WKey::ArrowRight => KeyCode::ArrowRight,
            WKey::Home => KeyCode::Home,
            WKey::End => KeyCode::End,
            WKey::PageUp => KeyCode::PageUp,
            WKey::PageDown => KeyCode::PageDown,
            WKey::Backspace => KeyCode::Backspace,
            WKey::Delete => KeyCode::Delete,
            WKey::Enter => KeyCode::Enter,
            WKey::Tab => KeyCode::Tab,
            WKey::Escape => KeyCode::Escape,
            WKey::Space => KeyCode::Space,
            WKey::ShiftLeft => KeyCode::ShiftLeft,
            WKey::ShiftRight => KeyCode::ShiftRight,
            WKey::ControlLeft => KeyCode::ControlLeft,
            WKey::ControlRight => KeyCode::ControlRight,
            WKey::AltLeft => KeyCode::AltLeft,
            WKey::AltRight => KeyCode::AltRight,
            WKey::SuperLeft => KeyCode::SuperLeft,
            WKey::SuperRight => KeyCode::SuperRight,
            _ => KeyCode::Unknown,
        },
        PhysicalKey::Unidentified(_) => KeyCode::Unknown,
    }
}
