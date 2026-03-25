//! Keyboard event manager — key dispatch and default keyboard actions.
//!
//! Chrome: `KeyboardEventManager` (`blink/core/input/keyboard_event_manager.h`).

use crate::dom::handle::Handle;
use crate::events::keyboard_event::{KeyDownEvent, KeyUpEvent};
use crate::input::default_action::DefaultAction;
use crate::input::{ButtonState, KeyCode};
use crate::page::FocusController;
use kozan_primitives::geometry::Offset;

use super::{InputContext, InputResult};

pub(crate) struct KeyboardEventManager;

impl KeyboardEventManager {
    pub fn on_key_event(&self, ctx: &InputContext, ke: crate::input::KeyboardEvent) -> InputResult {
        let handle = ctx
            .doc
            .focused_element()
            .and_then(|id| ctx.doc.resolve(id))
            .or_else(|| ctx.doc.root_handle());
        let Some(handle) = handle else {
            return InputResult::state(false);
        };

        let key = ke.key;
        let modifiers = ke.modifiers;

        let allow_default = match ke.state {
            ButtonState::Pressed => handle.dispatch_event(&KeyDownEvent {
                key,
                modifiers,
                text: ke.text,
            }),
            ButtonState::Released => {
                handle.dispatch_event(&KeyUpEvent { key, modifiers });
                return InputResult::state(true);
            }
        };

        let action = if allow_default {
            // Chrome: walk DefaultEventHandler chain from target up before
            // falling back to browser-level defaults (scroll, focus navigation).
            let handled = Self::run_default_event_handlers(&handle, key, modifiers);
            if handled {
                DefaultAction::None
            } else {
                self.default_keyboard_action(ctx, key, modifiers)
            }
        } else {
            DefaultAction::None
        };

        InputResult {
            state_changed: true,
            default_action: action,
        }
    }

    /// Chrome: `Node::DefaultEventHandler()` walk — target up to root.
    ///
    /// Each element gets a chance to handle the event (e.g. button converts
    /// Enter/Space to a synthetic click). Returns `true` if any element handled it.
    fn run_default_event_handlers(
        target: &Handle,
        key: KeyCode,
        modifiers: crate::input::Modifiers,
    ) -> bool {
        let event = KeyDownEvent {
            key,
            modifiers,
            text: None,
        };

        let mut current = *target;
        loop {
            if current.default_event_handler(&event) {
                return true;
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => return false,
            }
        }
    }

    fn default_keyboard_action(
        &self,
        ctx: &InputContext,
        key: KeyCode,
        modifiers: crate::input::Modifiers,
    ) -> DefaultAction {
        const LINE_PX: f32 = 40.0;
        let focused = ctx.doc.focused_element();

        let delta = match key {
            KeyCode::ArrowUp => Offset::new(0.0, -LINE_PX),
            KeyCode::ArrowDown => Offset::new(0.0, LINE_PX),
            KeyCode::ArrowLeft => Offset::new(-LINE_PX, 0.0),
            KeyCode::ArrowRight => Offset::new(LINE_PX, 0.0),
            KeyCode::PageUp => Offset::new(0.0, -(ctx.viewport_height - LINE_PX)),
            KeyCode::PageDown => Offset::new(0.0, ctx.viewport_height - LINE_PX),
            KeyCode::Home => Offset::new(0.0, -f32::MAX),
            KeyCode::End => Offset::new(0.0, f32::MAX),
            KeyCode::Space if modifiers.shift() => {
                Offset::new(0.0, -(ctx.viewport_height - LINE_PX))
            }
            KeyCode::Space if focused.is_none() => {
                Offset::new(0.0, ctx.viewport_height - LINE_PX)
            }
            KeyCode::Tab if modifiers.shift() => {
                return DefaultAction::FocusNavigate { forward: false };
            }
            KeyCode::Tab => return DefaultAction::FocusNavigate { forward: true },
            _ => return DefaultAction::None,
        };

        let target = FocusController::scroll_target(ctx.doc, ctx.scroll_tree)
            .or_else(|| ctx.scroll_tree.root_scroller())
            .unwrap_or(0);
        DefaultAction::Scroll { target, delta }
    }
}
