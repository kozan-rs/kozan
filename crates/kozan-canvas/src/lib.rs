//! Renderer-agnostic Canvas 2D recording API for Kozan.
//!
//! Chrome equivalent: the Blink Canvas 2D subsystem (`CanvasRenderingContext2D`,
//! `Canvas2DRecorderContext`, `CanvasPath`) plus the cc recording layer
//! (`PaintOpBuffer`, `PaintRecord`, `PaintOp`).
//!
//! This crate records drawing commands without executing them. A backend-specific
//! player (e.g., `VelloCanvasPlayer` in `kozan-vello`) replays the recording
//! to produce actual pixels. This separation ensures zero coupling between the
//! Canvas API and any particular renderer.

pub mod blend;
pub mod context;
pub mod image;
pub mod line;
pub mod op;
pub mod path;
pub mod recording;
pub mod shadow;
pub mod state;
pub mod style;
pub mod text;

pub use context::CanvasRenderingContext2D;
pub use recording::CanvasRecording;
