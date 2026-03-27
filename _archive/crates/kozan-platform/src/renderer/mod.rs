//! Renderer abstraction — the contract between platform and GPU backend.
//!
//! Chrome: `viz::OutputSurface` + `GpuServiceImpl`.
//!
//! Defines the traits that GPU backends (kozan-vello) implement.
//! The platform layer uses these to run the render loop without
//! knowing which GPU API is underneath.

pub mod error;
pub mod factory;
pub mod surface;

pub use error::RendererError;
pub use factory::Renderer;
pub use surface::{RenderParams, RenderSurface};
