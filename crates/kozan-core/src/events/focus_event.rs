//! Focus DOM events — Chrome: `blink/core/dom/events/focus_event.h`.
//!
//! DOM-level focus events dispatched through the tree.
//! `Focus`/`Blur` do NOT bubble. `FocusIn`/`FocusOut` DO bubble.
//! This mirrors the W3C spec and Chrome's behavior.

use crate::id::RawId;
use kozan_macros::Event;

/// DOM `focus` event — fired when an element receives focus.
/// Does NOT bubble (use `FocusInEvent` for bubbling).
///
/// Chrome: `FocusEvent` with type `"focus"`.
#[derive(Debug, Clone, Event)]
#[event()]
#[non_exhaustive]
pub struct FocusEvent {
    /// The node that previously had focus (if any).
    /// Uses `RawId` (not raw `u32`) — safe, generation-checked reference.
    /// Chrome: `FocusEvent.relatedTarget` returns an `EventTarget`.
    pub related_target: Option<RawId>,
}

/// DOM `blur` event — fired when an element loses focus.
/// Does NOT bubble (use `FocusOutEvent` for bubbling).
///
/// Chrome: `FocusEvent` with type `"blur"`.
#[derive(Debug, Clone, Event)]
#[event()]
#[non_exhaustive]
pub struct BlurEvent {
    /// The node that is receiving focus (if any).
    pub related_target: Option<RawId>,
}

/// DOM `focusin` event — fired when an element is about to receive focus.
/// Bubbles (unlike `FocusEvent`).
///
/// Chrome: `FocusEvent` with type `"focusin"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles)]
#[non_exhaustive]
pub struct FocusInEvent {
    /// The node that previously had focus (if any).
    pub related_target: Option<RawId>,
}

/// DOM `focusout` event — fired when an element is about to lose focus.
/// Bubbles (unlike `BlurEvent`).
///
/// Chrome: `FocusEvent` with type `"focusout"`.
#[derive(Debug, Clone, Event)]
#[event(bubbles)]
#[non_exhaustive]
pub struct FocusOutEvent {
    /// The node that is receiving focus (if any).
    pub related_target: Option<RawId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Bubbles, Cancelable, Event};
    use crate::id::RawId;

    #[test]
    fn focus_does_not_bubble() {
        let evt = FocusEvent {
            related_target: None,
        };
        assert_eq!(evt.bubbles(), Bubbles::No);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }

    #[test]
    fn blur_does_not_bubble() {
        let evt = BlurEvent {
            related_target: Some(RawId::new(42, 0)),
        };
        assert_eq!(evt.bubbles(), Bubbles::No);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }

    #[test]
    fn focusin_bubbles() {
        let evt = FocusInEvent {
            related_target: None,
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }

    #[test]
    fn focusout_bubbles() {
        let evt = FocusOutEvent {
            related_target: Some(RawId::new(7, 1)),
        };
        assert_eq!(evt.bubbles(), Bubbles::Yes);
        assert_eq!(evt.cancelable(), Cancelable::No);
    }
}
