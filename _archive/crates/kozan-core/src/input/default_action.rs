//! Default actions — browser behavior that runs after DOM event dispatch.
//!
//! Chrome: `EventHandler::DefaultKeyboardEventHandler()`, `ScrollManager`.
//!
//! The EventHandler dispatches DOM events first. If JS didn't call
//! `preventDefault()`, the default action runs. This separation lets
//! the compositor eventually handle scroll without a main-thread round-trip.

use kozan_primitives::geometry::Offset;

/// Browser-level default behavior produced by event dispatch.
///
/// The EventHandler returns this; the coordinator (FrameWidget) executes it.
/// Designed for compositor handoff — scroll actions carry only primitive
/// data (target ID + pixel delta), no DOM references.
///
/// Activation (Enter/Space) is handled per-element via `Handle::default_event_handler`
/// rather than as a centralized action — mirrors Chrome's `Node::DefaultEventHandler()`.
pub(crate) enum DefaultAction {
    /// No default behavior for this event.
    None,
    /// Apply scroll delta to the chain starting at `target`.
    Scroll { target: u32, delta: Offset },
    /// JS called `preventDefault()` on a scrollable event.
    ScrollPrevented,
    /// Tab / Shift+Tab — handled by `LocalFrame` where `FocusController` is accessible.
    FocusNavigate { forward: bool },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_carries_target_and_delta() {
        let action = DefaultAction::Scroll {
            target: 5,
            delta: Offset::new(0.0, 120.0),
        };
        match action {
            DefaultAction::Scroll { target, delta } => {
                assert_eq!(target, 5);
                assert_eq!(delta.dy, 120.0);
            }
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn focus_navigate_carries_direction() {
        let fwd = DefaultAction::FocusNavigate { forward: true };
        let bwd = DefaultAction::FocusNavigate { forward: false };
        assert!(matches!(fwd, DefaultAction::FocusNavigate { forward: true }));
        assert!(matches!(bwd, DefaultAction::FocusNavigate { forward: false }));
    }
}
