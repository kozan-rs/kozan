//! `kozan-vello` — vello + wgpu renderer backend for Kozan.
//!
//! Implements the `renderer` trait contract using vello for 2D rendering
//! and wgpu for GPU access.
//!
//! # Usage
//!
//! ```ignore
//! use kozan_vello::VelloRenderer;
//!
//! let renderer = VelloRenderer::new()?;
//! // Pass to kozan-winit's WinitApp — it creates VelloSurfaces per window.
//! app.run_with_renderer(renderer, |ctx| { ... });
//! ```
//!
//! # Crate dependencies
//!
//! ```text
//! kozan-vello
//!   ├── kozan-core      (DisplayList, DrawCommand)
//!   ├── kozan-primitives (Color, Rect, geometry)
//!   ├── wgpu            (GPU command submission)
//!   └── vello           (2D scene rendering)
//! ```
//!

pub mod canvas_player;
mod convert;
mod renderer;
mod scene;
mod surface;

pub use renderer::VelloRenderer;
pub use surface::VelloSurface;
