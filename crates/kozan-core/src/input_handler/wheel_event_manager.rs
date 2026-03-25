//! Wheel event manager — DOM WheelEvent dispatch.
//!
//! Chrome: `MouseWheelEventManager` (`blink/core/input/mouse_wheel_event_manager.h`).
//! Scroll state is owned by the compositor — this only dispatches the DOM event.

use crate::events::wheel_event::WheelEvent;
use crate::input::default_action::DefaultAction;
use kozan_primitives::geometry::Point;

use super::{InputContext, InputResult};

pub(crate) struct WheelEventManager;

impl WheelEventManager {
    pub fn on_wheel(&self, ctx: &InputContext, we: crate::input::WheelEvent) -> InputResult {
        let point = Point::new(we.x as f32, we.y as f32);
        let hit = ctx.hit_tester.test(ctx.fragment, point);

        let Some(target) = hit.node_index else {
            return InputResult::state(false);
        };
        let Some(handle) = ctx.doc.handle_for_index(target) else {
            return InputResult::state(false);
        };

        let allow_default = handle.dispatch_event(&WheelEvent {
            x: point.x,
            y: point.y,
            delta_x: -(we.delta.dx() as f32),
            delta_y: -(we.delta.dy() as f32),
            modifiers: we.modifiers,
        });

        let action = if allow_default {
            DefaultAction::None
        } else {
            DefaultAction::ScrollPrevented
        };

        InputResult {
            state_changed: false,
            default_action: action,
        }
    }
}
