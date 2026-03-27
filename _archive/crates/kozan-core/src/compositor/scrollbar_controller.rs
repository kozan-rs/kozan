//! Scrollbar input controller — mouse interaction with scrollbar layers.
//!
//! Chrome: `cc/input/scrollbar_controller.cc`.
//! Compositor thread. Converts mouse events on scrollbar layers
//! into scroll offset changes.

use kozan_primitives::geometry::{Offset, Point};

use super::scrollbar_layer::{ScrollbarLayer, ScrollbarPart};
use crate::scroll::scrollbar::{self, Orientation};

struct DragState {
    scroll_element_id: u32,
    orientation: Orientation,
    drag_origin: f32,
    scroll_pos_at_start: f32,
    /// Chrome: `(max_scroll) / (track_length - thumb_length)`.
    ratio: f32,
}

/// Chrome: `cc::ScrollbarController`.
pub(crate) struct ScrollbarController {
    drag: Option<DragState>,
}

/// What the compositor should do after a scrollbar mouse event.
pub(crate) enum ScrollbarAction {
    /// Set scroll offset to this absolute position.
    ScrollTo { element_id: u32, offset: Offset },
    /// No action needed.
    None,
}

impl ScrollbarController {
    pub(crate) fn new() -> Self {
        Self { drag: None }
    }

    /// Chrome: `HandlePointerDown` → hit-test scrollbar, start drag or page-scroll.
    pub(crate) fn handle_pointer_down(
        &mut self,
        scrollbar: &ScrollbarLayer,
        local_point: Point,
    ) -> ScrollbarAction {
        let part = scrollbar.identify_part(local_point);
        let id = scrollbar.scroll_element_id;

        match part {
            ScrollbarPart::Thumb => {
                self.start_drag(scrollbar, local_point);
                ScrollbarAction::None
            }
            ScrollbarPart::BackTrack => {
                let step = page_step(scrollbar.clip_layer_length);
                let target = (scrollbar.current_pos - step).max(0.0);
                ScrollbarAction::ScrollTo {
                    element_id: id,
                    offset: pos_to_offset(scrollbar.orientation, target),
                }
            }
            ScrollbarPart::ForwardTrack => {
                let max = (scrollbar.scroll_layer_length - scrollbar.clip_layer_length).max(0.0);
                let step = page_step(scrollbar.clip_layer_length);
                let target = (scrollbar.current_pos + step).min(max);
                ScrollbarAction::ScrollTo {
                    element_id: id,
                    offset: pos_to_offset(scrollbar.orientation, target),
                }
            }
            ScrollbarPart::NoPart => ScrollbarAction::None,
        }
    }

    /// Chrome: `HandlePointerMove` → thumb drag.
    pub(crate) fn handle_pointer_move(&self, local_point: Point) -> ScrollbarAction {
        let Some(drag) = &self.drag else {
            return ScrollbarAction::None;
        };

        let pointer_pos = match drag.orientation {
            Orientation::Vertical => local_point.y,
            Orientation::Horizontal => local_point.x,
        };

        let target = drag.scroll_pos_at_start + (pointer_pos - drag.drag_origin) * drag.ratio;
        ScrollbarAction::ScrollTo {
            element_id: drag.scroll_element_id,
            offset: pos_to_offset(drag.orientation, target),
        }
    }

    /// Chrome: `HandlePointerUp` → end drag.
    pub(crate) fn handle_pointer_up(&mut self) {
        self.drag = None;
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub(crate) fn dragged_element(&self) -> Option<u32> {
        self.drag.as_ref().map(|d| d.scroll_element_id)
    }

    fn start_drag(&mut self, sb: &ScrollbarLayer, local_point: Point) {
        let track = sb.clip_layer_length - 2.0 * scrollbar::MARGIN;
        let Some(thumb) = sb.thumb_rect() else { return };
        let thumb_len = match sb.orientation {
            Orientation::Vertical => thumb.height(),
            Orientation::Horizontal => thumb.width(),
        };
        let max_scroll = (sb.scroll_layer_length - sb.clip_layer_length).max(0.0);
        if track <= thumb_len || max_scroll <= 0.0 {
            return;
        }

        self.drag = Some(DragState {
            scroll_element_id: sb.scroll_element_id,
            orientation: sb.orientation,
            drag_origin: match sb.orientation {
                Orientation::Vertical => local_point.y,
                Orientation::Horizontal => local_point.x,
            },
            scroll_pos_at_start: sb.current_pos,
            ratio: max_scroll / (track - thumb_len),
        });
    }
}

fn pos_to_offset(orientation: Orientation, pos: f32) -> Offset {
    match orientation {
        Orientation::Vertical => Offset::new(0.0, pos),
        Orientation::Horizontal => Offset::new(pos, 0.0),
    }
}

/// Chrome: `ScrollUtils::CalculatePageStep` — 87.5% of viewport.
fn page_step(viewport_length: f32) -> f32 {
    (viewport_length * 0.875).max(1.0)
}
