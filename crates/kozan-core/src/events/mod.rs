//! Event system — Chrome's event architecture adapted for Rust.
//!
//! # Chrome mapping
//!
//! | Chrome                    | Kozan                                |
//! |---------------------------|--------------------------------------|
//! | `Event` (class)           | [`Event`] trait                      |
//! | `UIEvent`, `MouseEvent`   | Concrete structs implementing Event  |
//! | `EventTarget`             | Listener storage on `Document`         |
//! | `EventListenerMap`        | `EventListenerMap` (linear Vec)      |
//! | `RegisteredEventListener` | [`RegisteredListener`]               |
//! | `EventDispatcher`         | `dispatch()` free function           |
//! | `EventPath`               | `EventPath` (index-based)            |
//! | `DefaultEventHandler`     | Default event handler (planned)      |
//!
//! # Dispatch flow
//!
//! ```text
//! 1. Build EventPath (target → root)
//! 2. CAPTURE: root → target (capture listeners only)
//! 3. TARGET: both capture + bubble listeners
//! 4. BUBBLE: target → root (bubble listeners only, if event.bubbles)
//! 5. DEFAULT: target.default_event_handler() (if !preventDefault)
//! ```
//!
//! # Key design: take-call-put
//!
//! During dispatch, listeners are **taken** from storage, **called**,
//! then **put back**. This allows handlers to safely mutate the tree,
//! add/remove listeners, and create/destroy nodes — without `RefCell` or Mutex.

mod context;
pub(crate) mod dispatcher;
mod event;
mod event_target;
pub mod focus_event;
pub mod keyboard_event;
pub(crate) mod listener;
mod listener_map;
pub mod mouse_event;
mod path;
pub(crate) mod store;
pub mod ui_event;
pub mod wheel_event;

pub use context::{EventContext, Phase};
pub(crate) use dispatcher::dispatch;
pub use event::{Bubbles, Cancelable, Event};
pub use event_target::EventTarget;
pub use listener::{ListenerId, ListenerOptions, RegisteredListener};
pub use listener_map::EventListenerMap;
pub use path::EventPath;

// Re-export all DOM event types at the events level.
pub use focus_event::{BlurEvent, FocusEvent, FocusInEvent, FocusOutEvent};
pub use keyboard_event::{KeyDownEvent, KeyUpEvent};
pub use mouse_event::{
    ClickEvent, ContextMenuEvent, DblClickEvent, MouseDownEvent, MouseEnterEvent,
    MouseLeaveEvent, MouseMoveEvent, MouseOutEvent, MouseOverEvent, MouseUpEvent,
};
pub use ui_event::{ResizeEvent, ScrollEvent};
pub use wheel_event::WheelEvent;
