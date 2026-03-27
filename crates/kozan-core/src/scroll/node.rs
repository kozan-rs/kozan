//! Scroll node — per-element scroll geometry from layout.
//!
//! Chrome: `cc/input/scroll_node.h`.

use crate::layout::fragment::OverscrollBehavior;
use kozan_primitives::geometry::Size;

/// Scroll geometry for one scrollable element.
///
/// Offset lives separately in [`super::ScrollOffsets`] so topology
/// and state can change independently — needed for compositor-thread scroll.
#[derive(Clone)]
pub struct ScrollNode {
    /// DOM node index that owns this scroll container.
    pub dom_id: u32,
    /// Parent scroll container in the chain. `None` for the root scroller.
    pub parent: Option<u32>,
    /// Visible area — padding box from layout.
    pub container: Size,
    /// Total content extent — may exceed `container` when children overflow.
    pub content: Size,
    /// Whether CSS allows horizontal scrolling on this node.
    pub scrollable_x: bool,
    /// Whether CSS allows vertical scrolling on this node.
    pub scrollable_y: bool,
    /// CSS `overscroll-behavior-x` — controls scroll chain propagation.
    pub overscroll_x: OverscrollBehavior,
    /// CSS `overscroll-behavior-y` — controls scroll chain propagation.
    pub overscroll_y: OverscrollBehavior,
}

impl ScrollNode {
    /// Maximum horizontal scroll displacement before clamping.
    pub fn max_offset_x(&self) -> f32 {
        (self.content.width - self.container.width).max(0.0)
    }

    /// Maximum vertical scroll displacement before clamping.
    pub fn max_offset_y(&self) -> f32 {
        (self.content.height - self.container.height).max(0.0)
    }

    /// True when content overflows horizontally AND CSS enables scrolling.
    pub fn can_scroll_x(&self) -> bool {
        self.scrollable_x && self.max_offset_x() > 0.0
    }

    /// True when content overflows vertically AND CSS enables scrolling.
    pub fn can_scroll_y(&self) -> bool {
        self.scrollable_y && self.max_offset_y() > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(container: (f32, f32), content: (f32, f32)) -> ScrollNode {
        ScrollNode {
            dom_id: 1,
            parent: None,
            container: Size::new(container.0, container.1),
            content: Size::new(content.0, content.1),
            scrollable_x: true,
            scrollable_y: true,
            overscroll_x: OverscrollBehavior::Auto,
            overscroll_y: OverscrollBehavior::Auto,
        }
    }

    #[test]
    fn vertical_overflow_produces_max_offset() {
        let n = node((200.0, 400.0), (200.0, 1200.0));
        assert_eq!(n.max_offset_x(), 0.0);
        assert_eq!(n.max_offset_y(), 800.0);
    }

    #[test]
    fn no_overflow_means_zero_max() {
        let n = node((500.0, 500.0), (100.0, 100.0));
        assert_eq!(n.max_offset_x(), 0.0);
        assert_eq!(n.max_offset_y(), 0.0);
    }

    #[test]
    fn can_scroll_requires_overflow_and_enabled_axis() {
        let n = node((200.0, 400.0), (200.0, 1200.0));
        assert!(!n.can_scroll_x());
        assert!(n.can_scroll_y());
    }

    #[test]
    fn disabled_axis_blocks_scroll() {
        let mut n = node((200.0, 400.0), (800.0, 1200.0));
        n.scrollable_x = false;
        assert!(!n.can_scroll_x());
        assert!(n.can_scroll_y());
    }
}
