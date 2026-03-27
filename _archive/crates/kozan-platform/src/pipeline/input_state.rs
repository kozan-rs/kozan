//! Per-window input state — cursor position and modifier keys.
//!
//! Chrome: `RenderWidgetHostImpl` tracks per-widget input state.
//! The main thread updates this before routing events to threads.

use kozan_core::input::{ButtonState, Modifiers, MouseButton};

/// Per-window cursor and modifier tracking.
///
/// All coordinates are logical (CSS) pixels — physical-to-logical
/// conversion happens at the platform boundary (set_cursor_physical).
pub struct InputState {
    cursor_x: f64,
    cursor_y: f64,
    scale_factor: f64,
    modifiers: Modifiers,
}

impl InputState {
    #[must_use] 
    pub fn new(scale_factor: f64) -> Self {
        Self {
            cursor_x: 0.0,
            cursor_y: 0.0,
            scale_factor,
            modifiers: Modifiers::EMPTY,
        }
    }

    pub fn set_cursor_physical(&mut self, px: f64, py: f64) {
        self.cursor_x = px / self.scale_factor;
        self.cursor_y = py / self.scale_factor;
    }

    #[must_use] 
    pub fn cursor(&self) -> (f64, f64) {
        (self.cursor_x, self.cursor_y)
    }

    pub fn set_scale_factor(&mut self, factor: f64) {
        self.scale_factor = factor;
    }

    #[must_use] 
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    #[must_use] 
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    #[allow(clippy::fn_params_excessive_bools)]
    pub fn set_modifiers_from_keyboard(&mut self, shift: bool, ctrl: bool, alt: bool, meta: bool) {
        let mut m = Modifiers::EMPTY;
        if shift {
            m = m.with_shift();
        }
        if ctrl {
            m = m.with_ctrl();
        }
        if alt {
            m = m.with_alt();
        }
        if meta {
            m = m.with_meta();
        }
        let mouse_bits = self.modifiers.bits() & Self::MOUSE_BUTTON_MASK;
        self.modifiers = Modifiers::from_bits(m.bits() | mouse_bits);
    }

    pub fn update_button_modifier(&mut self, button: &MouseButton, state: ButtonState) {
        let flag = match button {
            MouseButton::Left => Modifiers::EMPTY.with_left_button(),
            MouseButton::Right => Modifiers::EMPTY.with_right_button(),
            MouseButton::Middle => Modifiers::EMPTY.with_middle_button(),
            _ => return,
        };
        match state {
            ButtonState::Pressed => self.modifiers |= flag,
            ButtonState::Released => {
                self.modifiers = Modifiers::from_bits(self.modifiers.bits() & !flag.bits());
            }
        }
    }

    const MOUSE_BUTTON_MASK: u16 = {
        let l = Modifiers::EMPTY.with_left_button().bits();
        let r = Modifiers::EMPTY.with_right_button().bits();
        let m = Modifiers::EMPTY.with_middle_button().bits();
        l | r | m
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_to_logical_conversion() {
        let mut state = InputState::new(2.0);
        state.set_cursor_physical(200.0, 400.0);
        assert_eq!(state.cursor(), (100.0, 200.0));
    }

    #[test]
    fn modifiers_keyboard_preserves_mouse() {
        let mut state = InputState::new(1.0);
        state.update_button_modifier(&MouseButton::Left, ButtonState::Pressed);
        state.set_modifiers_from_keyboard(true, false, false, false);
        assert!(state.modifiers().shift());
        assert!(state.modifiers().left_button());
    }

    #[test]
    fn scale_factor_change() {
        let mut state = InputState::new(1.0);
        state.set_cursor_physical(100.0, 100.0);
        assert_eq!(state.cursor(), (100.0, 100.0));
        state.set_scale_factor(2.0);
        state.set_cursor_physical(100.0, 100.0);
        assert_eq!(state.cursor(), (50.0, 50.0));
    }
}
