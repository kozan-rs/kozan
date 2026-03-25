//! Event handler — converts platform input events to DOM events + dispatches.
//!
//! Chrome equivalent: `EventHandler` (`blink/core/input/event_handler.h`).
//!
//! # Architecture
//!
//! ```text
//! Platform Input → EventHandler → Hit Test → Update ElementState
//!                      ↑              ↓            ↓
//!                  State (hover,   DOM Events   Stylo :hover
//!                  focus, mousedown)             matches auto
//! ```
//!
//! # Chrome's approach (what we follow)
//!
//! - Hit test finds target element
//! - `ElementState` flags (HOVER, ACTIVE, FOCUS) are set on DOM nodes
//! - Stylo reads `ElementState` during selector matching
//! - CSS `:hover` rules apply automatically — zero manual class toggling
//! - Only affected elements are restyled (not the whole tree)
//!
//! # Decoupling via `DispatchSurface`
//!
//! The handler does NOT depend on `Document` directly. It takes
//! `&dyn DispatchSurface` — a minimal trait with only the methods it needs.

use std::sync::Arc;

use crate::dom::document::Document;
use crate::dom::handle::Handle;
use crate::events::keyboard_event::{KeyDownEvent, KeyUpEvent};
use crate::events::mouse_event::{
    ClickEvent, ContextMenuEvent, DblClickEvent, MouseDownEvent, MouseEnterEvent, MouseLeaveEvent,
    MouseMoveEvent, MouseOutEvent, MouseOverEvent, MouseUpEvent,
};
use crate::events::wheel_event::WheelEvent;
use crate::id::RawId;
use crate::input::default_action::DefaultAction;
use crate::input::{ButtonState, InputEvent, KeyCode, MouseButton};
use crate::layout::fragment::Fragment;
use crate::layout::hit_test::{HitTestCache, HitTestResult, HitTester};
use crate::scroll::ScrollTree;
use kozan_primitives::geometry::{Offset, Point};

/// Per-frame context needed by the event handler.
pub(crate) struct InputContext<'a> {
    pub surface: &'a dyn DispatchSurface,
    pub fragment: &'a Arc<Fragment>,
    pub hit_tester: &'a HitTester<'a>,
    pub viewport_height: f32,
    pub scroll_tree: &'a ScrollTree,
}

/// What happened after processing an input event.
///
/// The coordinator (FrameWidget) reads this to decide what to do next —
/// restyle, repaint, scroll, or nothing.
pub(crate) struct InputResult {
    pub state_changed: bool,
    pub default_action: DefaultAction,
}

impl InputResult {
    fn state(changed: bool) -> Self {
        Self {
            state_changed: changed,
            default_action: DefaultAction::None,
        }
    }
}

/// Minimal abstraction over the DOM for event dispatch.
///
/// Decouples `EventHandler` from `Document` internals.
/// Chrome equivalent: `EventHandler` operates on `LocalFrame`.
pub(crate) trait DispatchSurface {
    /// Resolve a node arena index to a Handle.
    fn handle_for_index(&self, index: u32) -> Option<Handle>;

    /// Resolve a `RawId` (index + generation) to a Handle.
    /// Generation-checked: returns `None` if the node was destroyed.
    fn resolve(&self, id: RawId) -> Option<Handle>;

    /// The root node handle for fallback keyboard routing.
    fn root_handle(&self) -> Option<Handle>;

    /// Set or clear the HOVER element state flag on a node.
    /// Chrome: `Element::SetHovered()` → triggers style invalidation.
    #[allow(dead_code)] // API for upcoming platform input dispatch
    fn set_hover_state(&self, index: u32, hovered: bool);

    /// Set or clear HOVER on a node AND all its ancestors.
    /// Chrome: hovering a child also hovers its parents.
    /// `.card:hover` stays active when cursor is on `.card-icon` (child).
    fn set_hover_chain(&self, index: u32, hovered: bool);

    fn set_active_chain(&self, index: u32, active: bool);

    fn find_focusable_ancestor(&self, index: u32) -> Option<u32>;

    /// UI Events §5.2.2 focus transition with event dispatch.
    fn move_focus(
        &self,
        old: Option<RawId>,
        new: Option<u32>,
        focus_visible: bool,
    ) -> Option<RawId>;

    /// Current focused element (authoritative — Document owns this).
    fn focused_element(&self) -> Option<RawId>;
}

impl DispatchSurface for Document {
    #[inline]
    fn handle_for_index(&self, index: u32) -> Option<Handle> {
        Document::handle_for_index(self, index)
    }

    #[inline]
    fn resolve(&self, id: RawId) -> Option<Handle> {
        Document::resolve(self, id)
    }

    #[inline]
    fn root_handle(&self) -> Option<Handle> {
        self.handle_for_index(Document::root_index(self))
    }

    fn set_hover_state(&self, index: u32, hovered: bool) {
        Document::set_hover_state(self, index, hovered);
    }

    fn set_hover_chain(&self, index: u32, hovered: bool) {
        Document::set_hover_chain(self, index, hovered);
    }

    fn set_active_chain(&self, index: u32, active: bool) {
        Document::set_active_chain(self, index, active);
    }

    fn find_focusable_ancestor(&self, index: u32) -> Option<u32> {
        Document::find_focusable_ancestor(self, index)
    }

    fn move_focus(
        &self,
        old: Option<RawId>,
        new: Option<u32>,
        focus_visible: bool,
    ) -> Option<RawId> {
        Document::move_focus(self, old, new, focus_visible)
    }

    fn focused_element(&self) -> Option<RawId> {
        self.focused_element
    }
}

/// Handles input-to-DOM-event conversion and dispatch.
///
/// Tracks hover, focus, and mousedown state. Sets `ElementState` flags
/// on DOM nodes so Stylo's `:hover`/`:active`/`:focus` CSS selectors
/// work automatically — zero manual class toggling.
pub(crate) struct EventHandler {
    hovered_node: Option<RawId>,
    mousedown_node: Option<RawId>,
    mousedown_button: Option<MouseButton>,
    last_cursor: Point,
    hit_cache: HitTestCache,
    /// Suppresses :hover during scroll to avoid flashing.
    suppress_hover: bool,
}

impl EventHandler {
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

    /// Dispatch an input event. Returns what happened — the coordinator
    /// decides what to do (restyle, repaint, scroll).
    pub fn handle_input(&mut self, event: InputEvent, ctx: &InputContext) -> InputResult {
        match event {
            InputEvent::MouseMove(me) => InputResult::state(self.on_mouse_move(ctx, me)),
            InputEvent::MouseButton(me) => InputResult::state(self.on_mouse_button(ctx, me)),
            InputEvent::Keyboard(ke) => self.on_keyboard(ctx, ke),
            InputEvent::Wheel(we) => self.on_wheel(ctx, we),
            InputEvent::MouseEnter(_) => InputResult::state(false),
            InputEvent::MouseLeave(me) => InputResult::state(self.on_mouse_leave(ctx, me)),
        }
    }

    fn hit_at(&mut self, ctx: &InputContext, point: Point) -> HitTestResult {
        self.hit_cache
            .test(ctx.hit_tester, ctx.fragment, point)
            .clone()
    }

    fn on_mouse_move(&mut self, ctx: &InputContext, me: crate::input::MouseMoveEvent) -> bool {
        let point = Point::new(me.x as f32, me.y as f32);
        self.last_cursor = point;

        // After scroll ends, the first mouse move clears suppression
        // but still skips hover to avoid a flash on the element under
        // the cursor at the scroll-end position.
        if self.suppress_hover {
            self.suppress_hover = false;
            return false;
        }

        let hit = self.hit_at(ctx, point);
        let changed = self.update_hover(ctx, &hit, point, me.modifiers);

        if let Some(handle) = hit.node_index.and_then(|i| ctx.surface.handle_for_index(i)) {
            handle.dispatch_event(&MouseMoveEvent {
                x: point.x,
                y: point.y,
                modifiers: me.modifiers,
            });
        }
        changed
    }

    fn on_mouse_button(&mut self, ctx: &InputContext, me: crate::input::MouseButtonEvent) -> bool {
        let point = Point::new(me.x as f32, me.y as f32);
        self.last_cursor = point;
        let hit = self.hit_at(ctx, point);

        let Some(handle) = hit.node_index.and_then(|i| ctx.surface.handle_for_index(i)) else {
            return false;
        };
        let raw_id = handle.raw();

        match me.state {
            ButtonState::Pressed => {
                self.mousedown_node = Some(raw_id);
                self.mousedown_button = Some(me.button);
                if let Some(idx) = hit.node_index {
                    ctx.surface.set_active_chain(idx, true);
                }
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
                        .and_then(|idx| ctx.surface.find_focusable_ancestor(idx));
                    ctx.surface.move_focus(ctx.surface.focused_element(), target, false);
                }
            }
            ButtonState::Released => {
                if let Some(old_id) = self.mousedown_node {
                    if ctx.surface.resolve(old_id).is_some() {
                        ctx.surface.set_active_chain(old_id.index(), false);
                    }
                }
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

    /// Chrome: `EventHandler::KeyEvent()` → dispatch → `DefaultKeyboardEventHandler()`.
    fn on_keyboard(&mut self, ctx: &InputContext, ke: crate::input::KeyboardEvent) -> InputResult {
        let handle = ctx
            .surface
            .focused_element()
            .and_then(|id| ctx.surface.resolve(id))
            .or_else(|| ctx.surface.root_handle());
        let Some(handle) = handle else {
            return InputResult::state(false);
        };

        let allow_default = match ke.state {
            ButtonState::Pressed => handle.dispatch_event(&KeyDownEvent {
                key: ke.key,
                modifiers: ke.modifiers,
                text: ke.text,
            }),
            ButtonState::Released => {
                handle.dispatch_event(&KeyUpEvent {
                    key: ke.key,
                    modifiers: ke.modifiers,
                });
                return InputResult::state(true);
            }
        };

        let action = if allow_default {
            self.default_keyboard_action(ctx, ke.key, ke.modifiers)
        } else {
            DefaultAction::None
        };

        InputResult {
            state_changed: true,
            default_action: action,
        }
    }

    fn default_keyboard_action(
        &self,
        ctx: &InputContext,
        key: KeyCode,
        modifiers: crate::input::Modifiers,
    ) -> DefaultAction {
        const LINE_PX: f32 = 40.0;
        let focused = ctx.surface.focused_element();

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
            // Space scrolls only when nothing is focused (no activation target).
            KeyCode::Space if focused.is_none() => {
                Offset::new(0.0, ctx.viewport_height - LINE_PX)
            }
            KeyCode::Tab if modifiers.shift() => return DefaultAction::FocusPrev,
            KeyCode::Tab => return DefaultAction::FocusNext,
            KeyCode::Enter | KeyCode::Space if focused.is_some() => {
                return DefaultAction::Activate;
            }
            _ => return DefaultAction::None,
        };

        // Chrome: keyboard scroll targets focused element's scrollable ancestor.
        let target = Self::focused_scroll_target(ctx)
            .or_else(|| ctx.scroll_tree.root_scroller())
            .unwrap_or(0);
        DefaultAction::Scroll { target, delta }
    }

    fn focused_scroll_target(ctx: &InputContext) -> Option<u32> {
        let focused_idx = ctx.surface.focused_element()?.index();
        let mut current = focused_idx;
        loop {
            if ctx.scroll_tree.contains(current) {
                return Some(current);
            }
            let handle = ctx.surface.handle_for_index(current)?;
            current = handle.parent()?.raw().index();
        }
    }

    /// Dispatch DOM WheelEvent only — scroll state is owned by the compositor.
    ///
    /// Chrome: EventHandler dispatches the DOM event. If JS calls
    /// preventDefault(), the compositor is told to cancel. Otherwise
    /// the compositor (on the render thread) handles scroll directly.
    /// The view thread NEVER mutates scroll offsets from wheel events.
    fn on_wheel(&mut self, ctx: &InputContext, we: crate::input::WheelEvent) -> InputResult {
        let point = Point::new(we.x as f32, we.y as f32);
        let hit = self.hit_at(ctx, point);

        let Some(target) = hit.node_index else {
            return InputResult::state(false);
        };
        let Some(handle) = ctx.surface.handle_for_index(target) else {
            return InputResult::state(false);
        };

        // W3C: positive deltaY = scroll down (opposite of platform convention).
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

    fn on_mouse_leave(&mut self, ctx: &InputContext, me: crate::input::MouseLeaveEvent) -> bool {
        let Some(old_id) = self.hovered_node.take() else {
            return false;
        };
        ctx.surface.set_hover_chain(old_id.index(), false);
        if let Some(handle) = ctx.surface.resolve(old_id) {
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

    /// Update hover state — set `ElementState::HOVER` on DOM nodes.
    /// Returns `true` if the hovered element changed.
    ///
    /// Chrome: `EventHandler::HandleMouseMoveOrLeaveEvent()` →
    /// `Element::SetHovered()` → style invalidation.
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

        // Chrome: `SetHoveredNode()` clears the entire ancestor chain.
        if let Some(old_id) = self.hovered_node {
            ctx.surface.set_hover_chain(old_id.index(), false);
            if let Some(handle) = ctx.surface.resolve(old_id) {
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

        // Set HOVER on new node AND all its ancestors.
        // Chrome: hovering `.card-icon` also hovers `.card` (parent).
        // CSS `.card:hover` stays active when cursor is on a child.
        if let Some(new_idx) = new_node {
            ctx.surface.set_hover_chain(new_idx, true);
            if let Some(handle) = ctx.surface.handle_for_index(new_idx) {
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

        true // Hover changed — needs restyle.
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

    /// Move focus via Tab/Shift+Tab through the tab order.
    pub fn navigate_focus(
        &self,
        surface: &dyn DispatchSurface,
        tab_order: &[u32],
        forward: bool,
    ) {
        if tab_order.is_empty() {
            return;
        }

        let focused = surface.focused_element();
        let current_pos =
            focused.and_then(|id| tab_order.iter().position(|&idx| idx == id.index()));

        let next_idx = match current_pos {
            Some(pos) => {
                if forward {
                    (pos + 1) % tab_order.len()
                } else {
                    (pos + tab_order.len() - 1) % tab_order.len()
                }
            }
            None => {
                if forward { 0 } else { tab_order.len() - 1 }
            }
        };

        surface.move_focus(focused, Some(tab_order[next_idx]), true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{
        ButtonState, InputEvent, KeyCode, Modifiers, MouseButton,
        default_action::DefaultAction,
        keyboard::KeyboardEvent as RawKeyboardEvent,
        mouse::{
            MouseButtonEvent as RawMouseButtonEvent, MouseEnterEvent as RawMouseEnterEvent,
            MouseLeaveEvent as RawMouseLeaveEvent, MouseMoveEvent as RawMouseMoveEvent,
        },
        wheel::{WheelDelta, WheelEvent as RawWheelEvent},
    };
    use crate::layout::fragment::{BoxFragmentData, Fragment};
    use crate::scroll::{ScrollOffsets, ScrollTree};
    use kozan_primitives::geometry::Size;
    use std::time::Instant;

    fn make_ctx(_doc: &Document) -> (Arc<Fragment>, ScrollOffsets, ScrollTree) {
        let frag = Fragment::new_box(Size::new(800.0, 600.0), BoxFragmentData::default());
        (frag, ScrollOffsets::new(), ScrollTree::new())
    }

    #[test]
    fn initial_state() {
        let handler = EventHandler::new();
        assert_eq!(handler.hovered_node(), None);
    }

    #[test]
    fn focus_owned_by_document() {
        let doc = Document::new();
        assert_eq!(doc.focused_element, None);
    }

    #[test]
    fn all_input_variants_do_not_panic() {
        let mut handler = EventHandler::new();
        let doc = Document::new();
        let (frag, offsets, tree) = make_ctx(&doc);
        let hit_tester = HitTester::new(&offsets);
        let ctx = InputContext {
            surface: &doc,
            fragment: &frag,
            hit_tester: &hit_tester,
            viewport_height: 600.0,
            scroll_tree: &tree,
        };
        let now = Instant::now();

        handler.handle_input(
            InputEvent::MouseMove(RawMouseMoveEvent {
                x: 0.0,
                y: 0.0,
                modifiers: Modifiers::EMPTY,
                timestamp: now,
            }),
            &ctx,
        );
        handler.handle_input(
            InputEvent::MouseButton(RawMouseButtonEvent {
                x: 0.0,
                y: 0.0,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                modifiers: Modifiers::EMPTY,
                click_count: 1,
                timestamp: now,
            }),
            &ctx,
        );
        handler.handle_input(
            InputEvent::Keyboard(RawKeyboardEvent {
                key: KeyCode::Enter,
                state: ButtonState::Pressed,
                modifiers: Modifiers::EMPTY,
                text: None,
                timestamp: now,
            }),
            &ctx,
        );
        handler.handle_input(
            InputEvent::Wheel(RawWheelEvent {
                x: 0.0,
                y: 0.0,
                delta: WheelDelta::Lines(0.0, -1.0),
                modifiers: Modifiers::EMPTY,
                timestamp: now,
            }),
            &ctx,
        );
        handler.handle_input(
            InputEvent::MouseEnter(RawMouseEnterEvent {
                x: 0.0,
                y: 0.0,
                modifiers: Modifiers::EMPTY,
                timestamp: now,
            }),
            &ctx,
        );
        handler.handle_input(
            InputEvent::MouseLeave(RawMouseLeaveEvent {
                modifiers: Modifiers::EMPTY,
                timestamp: now,
            }),
            &ctx,
        );
    }

    #[test]
    fn arrow_down_produces_scroll() {
        let mut handler = EventHandler::new();
        let doc = Document::new();
        let (frag, offsets, tree) = make_ctx(&doc);
        let hit_tester = HitTester::new(&offsets);
        let ctx = InputContext {
            surface: &doc,
            fragment: &frag,
            hit_tester: &hit_tester,
            viewport_height: 600.0,
            scroll_tree: &tree,
        };

        let result = handler.handle_input(
            InputEvent::Keyboard(RawKeyboardEvent {
                key: KeyCode::ArrowDown,
                state: ButtonState::Pressed,
                modifiers: Modifiers::EMPTY,
                text: None,
                timestamp: Instant::now(),
            }),
            &ctx,
        );

        match result.default_action {
            DefaultAction::Scroll { delta, .. } => assert_eq!(delta.dy, 40.0),
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn page_down_uses_viewport_height() {
        let mut handler = EventHandler::new();
        let doc = Document::new();
        let (frag, offsets, tree) = make_ctx(&doc);
        let hit_tester = HitTester::new(&offsets);
        let ctx = InputContext {
            surface: &doc,
            fragment: &frag,
            hit_tester: &hit_tester,
            viewport_height: 800.0,
            scroll_tree: &tree,
        };

        let result = handler.handle_input(
            InputEvent::Keyboard(RawKeyboardEvent {
                key: KeyCode::PageDown,
                state: ButtonState::Pressed,
                modifiers: Modifiers::EMPTY,
                text: None,
                timestamp: Instant::now(),
            }),
            &ctx,
        );

        match result.default_action {
            DefaultAction::Scroll { delta, .. } => assert_eq!(delta.dy, 760.0),
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn tab_produces_focus_next() {
        let mut handler = EventHandler::new();
        let doc = Document::new();
        let (frag, offsets, tree) = make_ctx(&doc);
        let hit_tester = HitTester::new(&offsets);
        let ctx = InputContext {
            surface: &doc,
            fragment: &frag,
            hit_tester: &hit_tester,
            viewport_height: 600.0,
            scroll_tree: &tree,
        };

        let result = handler.handle_input(
            InputEvent::Keyboard(RawKeyboardEvent {
                key: KeyCode::Tab,
                state: ButtonState::Pressed,
                modifiers: Modifiers::EMPTY,
                text: None,
                timestamp: Instant::now(),
            }),
            &ctx,
        );

        assert!(matches!(result.default_action, DefaultAction::FocusNext));
    }

    #[test]
    fn shift_tab_produces_focus_prev() {
        let mut handler = EventHandler::new();
        let doc = Document::new();
        let (frag, offsets, tree) = make_ctx(&doc);
        let hit_tester = HitTester::new(&offsets);
        let ctx = InputContext {
            surface: &doc,
            fragment: &frag,
            hit_tester: &hit_tester,
            viewport_height: 600.0,
            scroll_tree: &tree,
        };

        let result = handler.handle_input(
            InputEvent::Keyboard(RawKeyboardEvent {
                key: KeyCode::Tab,
                state: ButtonState::Pressed,
                modifiers: Modifiers::EMPTY.with_shift(),
                text: None,
                timestamp: Instant::now(),
            }),
            &ctx,
        );

        assert!(matches!(result.default_action, DefaultAction::FocusPrev));
    }
}
