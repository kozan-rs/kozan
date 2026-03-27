//! `kozan-winit` — winit windowing backend for Kozan.
//!
//! This is the **only crate** in the project that depends on winit.
//! It is a **dumb OS adapter** — creates windows, converts events,
//! delegates everything to `kozan-platform::WindowManager`.
//!
//! # Public API
//!
//! A single function: [`run()`]. The `kozan` facade crate calls this
//! internally from `App::run()`. Application authors never touch this
//! crate directly.
//!
//! # Architecture
//!
//! ```text
//! kozan (user-facing facade)
//!   └── App::run() calls kozan_winit::run()
//!
//! kozan-winit (this crate — dumb adapter)
//!   ├── run()             — creates EventLoop, runs AppHandler
//!   ├── AppHandler        — winit ApplicationHandler, routes OS events
//!   ├── WinitPlatformHost — PlatformHost impl via EventLoopProxy
//!   └── convert           — winit events → kozan InputEvent
//!
//! kozan-platform (the brain — ZERO winit)
//!   ├── WindowManager<R> — owns renderer, pipelines, input state
//!   └── ViewContext, ViewEvent, etc.
//! ```

mod convert;
mod handler;
mod host_impl;

use winit::event_loop::EventLoop;

use kozan_platform::ViewContext;
use kozan_platform::renderer::Renderer;
use kozan_platform::request::WindowConfig;

use handler::AppHandler;
use host_impl::{InternalRequest, WinitPlatformHost};

/// A window to be created when the platform is ready.
///
/// Chrome: entries in `BrowserMainParts::PreMainMessageLoopRun()` that
/// become real windows once the event loop is running.
pub struct PendingWindow {
    pub config: WindowConfig,
    pub init: Box<dyn FnOnce(&ViewContext) + Send>,
}

impl PendingWindow {
    pub fn new<F>(config: WindowConfig, init: F) -> Self
    where
        F: FnOnce(&ViewContext) + Send + 'static,
    {
        Self {
            config,
            init: Box::new(init),
        }
    }
}

/// Error from the winit event loop.
#[derive(Debug)]
pub enum RunError {
    /// Failed to build the winit event loop.
    EventLoopCreation(String),
    /// The event loop returned an error.
    EventLoop(winit::error::EventLoopError),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoopCreation(e) => write!(f, "event loop creation failed: {e}"),
            Self::EventLoop(e) => write!(f, "event loop error: {e}"),
        }
    }
}

impl std::error::Error for RunError {}

/// Run the Kozan application on winit.
///
/// Chrome: `BrowserMainLoop::MainMessageLoopRun()`.
///
/// This is the **only public function** in this crate. It:
/// 1. Creates the winit event loop
/// 2. Creates the `WindowManager` (which holds the renderer)
/// 3. Runs the event loop (blocks until all windows close)
/// 4. Returns when the event loop exits
///
/// The `kozan` facade crate calls this from `App::run()`.
/// Application authors never call this directly.
pub fn run<R: Renderer>(renderer: R, windows: Vec<PendingWindow>) -> Result<(), RunError> {
    let event_loop = EventLoop::<InternalRequest>::with_user_event()
        .build()
        .map_err(|e| RunError::EventLoopCreation(e.to_string()))?;

    let host = WinitPlatformHost::new(event_loop.create_proxy(), renderer.name());

    let pending = windows
        .into_iter()
        .map(|pw| handler::PendingWindow {
            config: pw.config,
            view_init: pw.init,
        })
        .collect();

    let mut handler = AppHandler::new(renderer, pending, host);

    event_loop
        .run_app(&mut handler)
        .map_err(RunError::EventLoop)
}
