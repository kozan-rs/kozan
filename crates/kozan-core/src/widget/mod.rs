//! Widget — the engine's top-level entry point for rendering contexts.
//!
//! Chrome equivalent: `core/frame/` — `WebFrameWidgetImpl`, `LocalFrameView`.
//!
//! The platform creates a `FrameWidget` per view. All engine operations
//! (input handling, lifecycle updates, document access) go through it.

mod event_handler;
mod frame_widget;
mod viewport;

pub use frame_widget::FrameWidget;
pub use viewport::Viewport;
