//! Input handling — converts platform events to DOM events.
//!
//! Chrome: `blink/core/input/` — `EventHandler`, `MouseEventManager`,
//! `KeyboardEventManager`, `MouseWheelEventManager`.

mod event_handler;
mod keyboard_event_manager;
mod mouse_event_manager;
mod wheel_event_manager;

pub(crate) use event_handler::EventHandler;

use std::sync::Arc;

use crate::dom::document::Document;
use crate::input::default_action::DefaultAction;
use crate::layout::fragment::Fragment;
use crate::layout::hit_test::HitTester;
use crate::scroll::ScrollTree;

/// Per-frame context needed by event handlers.
pub(crate) struct InputContext<'a> {
    pub doc: &'a Document,
    pub fragment: &'a Arc<Fragment>,
    pub hit_tester: &'a HitTester<'a>,
    pub viewport_height: f32,
    pub scroll_tree: &'a ScrollTree,
}

/// What happened after processing an input event.
pub(crate) struct InputResult {
    pub state_changed: bool,
    pub default_action: DefaultAction,
}

impl InputResult {
    pub(crate) fn state(changed: bool) -> Self {
        Self {
            state_changed: changed,
            default_action: DefaultAction::None,
        }
    }
}
