//! Mouse event manager — hit testing, hover, mousedown, click dispatch.
//!
//! Chrome: `MouseEventManager` (`blink/core/input/mouse_event_manager.h`).

use crate::dom::handle::Handle;
use crate::events::mouse_event::{
    ClickEvent, ContextMenuEvent, DblClickEvent, MouseDownEvent, MouseEnterEvent, MouseLeaveEvent,
    MouseMoveEvent, MouseOutEvent, MouseOverEvent, MouseUpEvent,
};
use crate::id::RawId;
use crate::input::{ButtonState, MouseButton};
use crate::layout::hit_test::{HitTestCache, HitTestResult};
use kozan_primitives::geometry::Point;

use super::InputContext;

pub(crate) struct MouseEventManager {
    hovered_node: Option<RawId>,
    mousedown_node: Option<RawId>,
    mousedown_button: Option<MouseButton>,
    last_cursor: Point,
    hit_cache: HitTestCache,
    suppress_hover: bool,
}

impl MouseEventManager {
    pub fn new() -> Self {
        Self {
            hovered_node: None,
            mousedown_node: None,
            mousedown_button: None,
            last_cursor: Point::ZERO,
            hit_cache: HitTestCache::new(),
            suppress_hover: false,
        }
    }

    pub fn on_mouse_move(&mut self, ctx: &InputContext, me: crate::input::MouseMoveEvent) -> bool {
        let point = Point::new(me.x as f32, me.y as f32);
        self.last_cursor = point;

        if self.suppress_hover {
            self.suppress_hover = false;
            return false;
        }

        let hit = self.hit_at(ctx, point);
        let changed = self.update_hover(ctx, &hit, point, me.modifiers);

        if let Some(handle) = hit.node_index.and_then(|i| ctx.doc.handle_for_index(i)) {
            handle.dispatch_event(&MouseMoveEvent {
                x: point.x,
                y: point.y,
                modifiers: me.modifiers,
            });
        }
        changed
    }

    pub fn on_mouse_button(
        &mut self,
        ctx: &InputContext,
        me: crate::input::MouseButtonEvent,
    ) -> bool {
        let point = Point::new(me.x as f32, me.y as f32);
        self.last_cursor = point;
        let hit = self.hit_at(ctx, point);

        let Some(handle) = hit.node_index.and_then(|i| ctx.doc.handle_for_index(i)) else {
            return false;
        };
        let raw_id = handle.raw();

        match me.state {
            ButtonState::Pressed => {
                self.mousedown_node = Some(raw_id);
                self.mousedown_button = Some(me.button);
                ctx.doc.set_active_element(hit.node_index);
                handle.dispatch_event(&MouseDownEvent {
                    x: point.x,
                    y: point.y,
                    button: me.button,
                    modifiers: me.modifiers,
                });
                // HTML §6.6.4: focus moves on mousedown, not click.
                if me.button == MouseButton::Left {
                    let target = hit
                        .node_index
                        .and_then(|idx| ctx.doc.find_focusable_ancestor(idx));
                    ctx.doc.set_focused_element(target, false);
                }
            }
            ButtonState::Released => {
                ctx.doc.set_active_element(None);
                handle.dispatch_event(&MouseUpEvent {
                    x: point.x,
                    y: point.y,
                    button: me.button,
                    modifiers: me.modifiers,
                });
                self.dispatch_click_events(&handle, raw_id, point, &me);
                self.mousedown_node = None;
                self.mousedown_button = None;
            }
        }
        true
    }

    pub fn on_mouse_leave(
        &mut self,
        ctx: &InputContext,
        me: crate::input::MouseLeaveEvent,
    ) -> bool {
        let Some(old_id) = self.hovered_node.take() else {
            return false;
        };
        ctx.doc.set_hover_element(None);
        if let Some(handle) = ctx.doc.resolve(old_id) {
            handle.dispatch_event(&MouseLeaveEvent {
                x: self.last_cursor.x,
                y: self.last_cursor.y,
                modifiers: me.modifiers,
            });
            handle.dispatch_event(&MouseOutEvent {
                x: self.last_cursor.x,
                y: self.last_cursor.y,
                modifiers: me.modifiers,
            });
        }
        true
    }

    pub fn invalidate_hit_cache(&mut self) {
        self.hit_cache.invalidate();
    }

    pub fn suppress_hover(&mut self) {
        self.suppress_hover = true;
    }

    #[cfg(test)]
    pub fn hovered_node(&self) -> Option<RawId> {
        self.hovered_node
    }

    fn hit_at(&mut self, ctx: &InputContext, point: Point) -> HitTestResult {
        self.hit_cache
            .test(ctx.hit_tester, ctx.fragment, point)
            .clone()
    }

    fn update_hover(
        &mut self,
        ctx: &InputContext,
        hit: &HitTestResult,
        point: Point,
        modifiers: crate::input::Modifiers,
    ) -> bool {
        let new_node = hit.node_index;

        let same_node = match (self.hovered_node, new_node) {
            (Some(old), Some(new_idx)) => old.index() == new_idx,
            (None, None) => true,
            _ => false,
        };

        if same_node {
            return false;
        }

        // Document handles LCA-based state toggling internally.
        ctx.doc.set_hover_element(new_node);

        if let Some(old_id) = self.hovered_node {
            if let Some(handle) = ctx.doc.resolve(old_id) {
                handle.dispatch_event(&MouseLeaveEvent {
                    x: point.x,
                    y: point.y,
                    modifiers,
                });
                handle.dispatch_event(&MouseOutEvent {
                    x: point.x,
                    y: point.y,
                    modifiers,
                });
            }
        }

        if let Some(new_idx) = new_node {
            if let Some(handle) = ctx.doc.handle_for_index(new_idx) {
                handle.dispatch_event(&MouseEnterEvent {
                    x: point.x,
                    y: point.y,
                    modifiers,
                });
                handle.dispatch_event(&MouseOverEvent {
                    x: point.x,
                    y: point.y,
                    modifiers,
                });
                self.hovered_node = Some(handle.raw());
            } else {
                self.hovered_node = None;
            }
        } else {
            self.hovered_node = None;
        }

        true
    }

    fn dispatch_click_events(
        &self,
        handle: &Handle,
        raw_id: RawId,
        point: Point,
        me: &crate::input::MouseButtonEvent,
    ) {
        if self.mousedown_node != Some(raw_id) || self.mousedown_button != Some(me.button) {
            return;
        }
        handle.dispatch_event(&ClickEvent {
            x: point.x,
            y: point.y,
            button: me.button,
            modifiers: me.modifiers,
        });
        if me.click_count == 2 {
            handle.dispatch_event(&DblClickEvent {
                x: point.x,
                y: point.y,
                button: me.button,
                modifiers: me.modifiers,
            });
        }
        if me.button == MouseButton::Right {
            handle.dispatch_event(&ContextMenuEvent {
                x: point.x,
                y: point.y,
                modifiers: me.modifiers,
            });
        }
    }
}
