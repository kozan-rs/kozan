//! Frame — document lifecycle.
//!
//! Chrome: `blink/core/frame/` — `LocalFrame`, `LocalFrameView`.

pub(crate) mod frame_view;
mod local_frame;

pub(crate) use local_frame::LocalFrame;
