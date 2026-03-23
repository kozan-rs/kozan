//! UI DOM events — Chrome: `blink/core/dom/events/ui_event.h`.
//!
//! DOM-level UI events (resize, scroll) dispatched through the tree.

use kozan_macros::Event;

/// DOM `scroll` event — fired when an element's scroll position changes.
/// Does NOT bubble (per W3C spec).
///
/// Chrome: `Event` with type `"scroll"`.
#[derive(Debug, Clone, Event)]
#[event()]
#[non_exhaustive]
pub struct ScrollEvent {
    /// Current horizontal scroll offset in CSS pixels.
    pub scroll_x: f32,
    /// Current vertical scroll offset in CSS pixels.
    pub scroll_y: f32,
}

/// DOM `resize` event — fired when the viewport or element size changes.
/// Does NOT bubble (per W3C spec).
///
/// Chrome: `UIEvent` with type `"resize"`.
#[derive(Debug, Clone, Event)]
#[event()]
#[non_exhaustive]
pub struct ResizeEvent {
    /// New width in CSS pixels.
    pub width: f32,
    /// New height in CSS pixels.
    pub height: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Bubbles, Cancelable, Event};

    #[test]
    fn scroll_event_does_not_bubble() {
        let evt = ScrollEvent {
            scroll_x: 0.0,
            scroll_y: 150.0,
        };
        assert_eq!(evt.bubbles(), Bubbles::No);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }

    #[test]
    fn resize_event_does_not_bubble() {
        let evt = ResizeEvent {
            width: 1920.0,
            height: 1080.0,
        };
        assert_eq!(evt.bubbles(), Bubbles::No);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }
}
