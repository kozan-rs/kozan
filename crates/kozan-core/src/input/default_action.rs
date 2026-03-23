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
pub(crate) enum DefaultAction {
    /// No default behavior for this event.
    None,
    /// Apply scroll delta to the chain starting at `target`.
    Scroll { target: u32, delta: Offset },
    /// JS called `preventDefault()` on a scrollable event.
    ScrollPrevented,
    /// Tab key — advance focus to the next focusable element.
    FocusNext,
    /// Shift+Tab — move focus to the previous focusable element.
    FocusPrev,
    /// Enter/Space on a focused element — trigger its activation behavior.
    Activate,
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
}
