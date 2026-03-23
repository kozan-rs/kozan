//! View event — the top-level message type sent to view threads.
//!
//! Like Chrome's separation of input events, lifecycle events, and rendering
//! signals into different routing paths. `ViewEvent` is the single enum
//! sent over the mpsc channel, but it's **hierarchical** — input events are
//! nested under `ViewEvent::Input(InputEvent)`, not flattened.
//!
//! # Architecture
//!
//! ```text
//! ViewEvent
//! ├── Input(InputEvent)        ← mouse, keyboard, wheel events
//! │   ├── MouseMove(...)
//! │   ├── MouseButton(...)
//! │   ├── Wheel(...)
//! │   └── Keyboard(...)
//! ├── Lifecycle(LifecycleEvent) ← resize, focus, scale factor
//! │   ├── Resized(...)
//! │   ├── Focused(...)
//! │   └── ScaleFactorChanged(...)
//! ├── Paint                     ← rendering signal
//! └── Shutdown                  ← clean exit
//! ```
//!
//! # Why hierarchical?
//!
//! Chrome routes input events through `InputRouterImpl`, lifecycle events
//! through the frame lifecycle, and paint signals through the compositor.
//! Different routing paths, different handling. A flat enum forces one
//! match arm per event — hierarchical lets the view thread match at the
//! category level first, then delegate to specialized handlers.

use kozan_core::InputEvent;
use kozan_core::scroll::ScrollOffsets;

/// Top-level event sent from the main thread to a view thread.
///
/// Sent over `mpsc::Sender<ViewEvent>` — the single channel per view.
/// The view thread's event loop matches on the category, then delegates
/// to specialized handlers.
///
/// Chrome equivalent: the dispatch in `WebFrameWidgetImpl::HandleInputEvent()`
/// for input, and separate IPC messages for lifecycle.
pub enum ViewEvent {
    /// An input event (mouse, keyboard, wheel).
    /// Routed to the view's `EventHandler` for hit testing and DOM dispatch.
    Input(InputEvent),

    /// A lifecycle event (resize, focus change, scale factor change).
    /// Handled by the view's layout/rendering pipeline.
    Lifecycle(LifecycleEvent),

    /// Something changed — schedule a frame.
    Paint,

    /// Compositor posted updated scroll offsets after compositor-side scroll.
    /// Chrome: `ProxyImpl::SetNeedsCommitOnImplThread()` posts scroll state back.
    /// The view thread applies these before the next paint so positions match.
    ScrollSync(ScrollOffsets),

    /// Clean shutdown — the view thread should exit its event loop.
    Shutdown,
}

/// Lifecycle events — window/view state changes.
///
/// These are NOT input events — they don't go through hit testing or
/// DOM event dispatch. They're handled directly by the view's rendering
/// pipeline (layout invalidation, viewport update, etc.).
///
/// Chrome equivalent: separate IPC messages like `WidgetMsg_Resize`,
/// `WidgetMsg_SetFocus`, `WidgetMsg_UpdateScreenInfo`.
#[derive(Debug, Clone, Copy)]
pub enum LifecycleEvent {
    /// The view's area was resized.
    /// Triggers layout invalidation.
    ///
    /// Width and height in physical pixels.
    Resized { width: u32, height: u32 },

    /// The view gained or lost focus.
    /// Triggers focus/blur DOM events and caret visibility.
    Focused(bool),

    /// The display changed (e.g., moved to a different monitor).
    /// Triggers re-layout at the new DPI and updates frame budget.
    ///
    /// Chrome equivalent: `WidgetMsg_UpdateScreenInfo` with new
    /// `device_scale_factor` + vsync interval.
    ScaleFactorChanged {
        scale_factor: f64,
        /// Display refresh rate in millihertz (e.g., 144000 = 144Hz).
        /// `None` = unknown → keep current budget.
        refresh_rate_millihertz: Option<u32>,
    },
}

#[cfg(test)]
mod tests {
    use kozan_core::{Modifiers, input::MouseMoveEvent};

    use super::*;
    use std::time::Instant;

    #[test]
    fn view_event_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ViewEvent>();
    }

    #[test]
    fn lifecycle_event_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<LifecycleEvent>();
    }

    #[test]
    fn hierarchical_matching() {
        let evt = ViewEvent::Input(InputEvent::MouseMove(MouseMoveEvent {
            x: 10.0,
            y: 20.0,
            modifiers: Modifiers::EMPTY,
            timestamp: Instant::now(),
        }));

        match evt {
            ViewEvent::Input(InputEvent::MouseMove(m)) => {
                assert!((m.x - 10.0).abs() < f64::EPSILON);
                assert!((m.y - 20.0).abs() < f64::EPSILON);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn lifecycle_resize() {
        let evt = ViewEvent::Lifecycle(LifecycleEvent::Resized {
            width: 1920,
            height: 1080,
        });
        match evt {
            ViewEvent::Lifecycle(LifecycleEvent::Resized { width, height }) => {
                assert_eq!(width, 1920);
                assert_eq!(height, 1080);
            }
            _ => panic!("wrong variant"),
        }
    }
}
