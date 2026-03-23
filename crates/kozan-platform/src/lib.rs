//! `kozan-platform` — Abstract platform layer for Kozan.
//!
//! Like Chrome's `content/` — defines the contract between the engine
//! and the windowing system. **Zero windowing-backend dependency.**
//!
//! The windowing backend (e.g., `kozan-winit`) implements `PlatformHost`
//! and drives the event loop. This crate provides:
//! - Abstract types: `WindowId`, `ViewId`, `WindowConfig`
//! - `PlatformHost` trait: view→main-thread communication
//! - `ViewEvent` / `LifecycleEvent`: main→view-thread messages
//! - `ViewThreadHandle` / `ViewContext`: per-view threading model
//! - `WindowManager<R>`: the brain — routes events, owns pipelines
//! - `Renderer` / `RenderSurface` traits: GPU backend abstraction
//!
//! # Architecture
//!
//! ```text
//! kozan-winit (or any backend)
//!   ├── implements PlatformHost
//!   ├── creates OS windows
//!   ├── passes raw window handles to WindowManager
//!   └── converts OS events → WindowManager.on_*()
//!
//! kozan-platform (this crate — THE BRAIN)
//!   ├── WindowManager<R: Renderer>: owns all windows + renderer
//!   ├── WindowPipeline: spawns view + render threads per window
//!   ├── RenderLoop: compositor + vsync loop
//!   ├── ViewContext: user-facing API inside view thread
//!   └── Renderer / RenderSurface traits
//!
//! kozan-vello (or any GPU backend)
//!   ├── implements Renderer + RenderSurface
//!   └── zero winit knowledge
//!
//! kozan-core (engine)
//!   ├── input/ types (InputEvent, Modifiers, etc.)
//!   ├── widget/ (FrameWidget, Viewport — future)
//!   └── dom/, events/, style/
//! ```

pub mod context;
pub mod event;
pub mod host;
pub mod id;
pub mod pipeline;
pub mod renderer;
pub mod request;
pub mod time;
pub mod view_thread;
pub mod window_manager;
pub(crate) mod window_state;

pub use context::ViewContext;
pub use event::{LifecycleEvent, ViewEvent};
pub use host::PlatformHost;
pub use id::{ViewId, WindowId};
pub use pipeline::ViewportInfo;
pub use pipeline::render_loop::RenderEvent;
pub use renderer::{Renderer, RendererError, RenderParams, RenderSurface};
pub use request::WindowConfig;
pub use view_thread::SpawnError;
pub use window_manager::{WindowManager, WindowCreateConfig, CreateWindowError};
