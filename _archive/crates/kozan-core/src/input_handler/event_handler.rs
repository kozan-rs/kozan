//! Event handler — thin facade over Mouse/Keyboard/Wheel managers.
//!
//! Chrome: `EventHandler` (`blink/core/input/event_handler.h`) delegates
//! to `MouseEventManager`, `KeyboardEventManager`, `MouseWheelEventManager`.

use crate::input::InputEvent;

use super::keyboard_event_manager::KeyboardEventManager;
use super::mouse_event_manager::MouseEventManager;
use super::wheel_event_manager::WheelEventManager;
use super::{InputContext, InputResult};

pub(crate) struct EventHandler {
    mouse: MouseEventManager,
    keyboard: KeyboardEventManager,
    wheel: WheelEventManager,
}

impl EventHandler {
    pub fn new() -> Self {
        Self {
            mouse: MouseEventManager::new(),
            keyboard: KeyboardEventManager,
            wheel: WheelEventManager,
        }
    }

    pub fn handle_input(&mut self, event: InputEvent, ctx: &InputContext) -> InputResult {
        match event {
            InputEvent::MouseMove(me) => InputResult::state(self.mouse.on_mouse_move(ctx, me)),
            InputEvent::MouseButton(me) => {
                InputResult::state(self.mouse.on_mouse_button(ctx, me))
            }
            InputEvent::Keyboard(ke) => self.keyboard.on_key_event(ctx, ke),
            InputEvent::Wheel(we) => self.wheel.on_wheel(ctx, we),
            InputEvent::MouseEnter(_) => InputResult::state(false),
            InputEvent::MouseLeave(me) => InputResult::state(self.mouse.on_mouse_leave(ctx, me)),
        }
    }

    pub fn invalidate_hit_cache(&mut self) {
        self.mouse.invalidate_hit_cache();
    }

    #[cfg(test)]
    pub fn hovered_node(&self) -> Option<crate::id::RawId> {
        self.mouse.hovered_node()
    }
}
