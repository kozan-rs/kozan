//! `kozan` — the Kozan UI platform.
//!
//! Single entry point for application authors. Re-exports everything
//! needed to build a Kozan application — no internal crate knowledge required.
//!
//! # Quick start
//!
//! ```ignore
//! use kozan::prelude::*;
//!
//! fn main() -> kozan::Result<()> {
//!     App::new()
//!         .window(WindowConfig::default(), |ctx| {
//!             let doc = ctx.document();
//!             let div = doc.div();
//!             div.style().w(px(200.0)).bg(hex("#ff4444"));
//!             doc.body().child(div);
//!         })
//!         .run()
//! }
//! ```
//!
//! # Feature flags
//!
//! | Flag    | What it enables           | Default |
//! |---------|---------------------------|---------|
//! | `winit` | winit windowing backend   | yes     |
//! | `vello` | vello/wgpu GPU renderer   | yes     |

pub mod prelude;

/// Platform time utilities (sleep, interval, timeout).
pub mod time {
    pub use kozan_platform::time::*;
}

// Everything from the prelude is available at crate root too.
pub use prelude::*;

// Items not in the prelude — less commonly needed types.
pub use kozan_core::styling::AbsoluteColor;
pub use kozan_core::styling::BorderStyle;
pub use kozan_core::styling::units::CssValue;

// ── Error ────────────────────────────────────────────────────

/// Unified error type for application-level failures.
///
/// Chrome: error codes from `ContentMain()`.
#[derive(Debug)]
pub enum Error {
    /// GPU renderer initialization failed (no adapter, device creation, etc.).
    Gpu(kozan_platform::RendererError),
    /// Platform event loop failed (creation or runtime).
    #[cfg(feature = "winit")]
    Platform(kozan_winit::RunError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gpu(e) => write!(f, "gpu: {e}"),
            #[cfg(feature = "winit")]
            Self::Platform(e) => write!(f, "platform: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Convenience alias for `std::result::Result<T, kozan::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

// ── App ──────────────────────────────────────────────────────

/// The Kozan application — lifecycle controller.
///
/// Chrome: `ChromeMain()` → `BrowserMain()` → `BrowserMainLoop`.
///
/// Collects window specifications during setup, then `run()` executes
/// the full lifecycle:
///
/// 1. **GPU init** — create the renderer (can fail → `Error::Gpu`)
/// 2. **Platform init** — create the OS event loop
/// 3. **Window creation** — deferred until the platform is ready
/// 4. **Main loop** — blocks until all windows are closed
/// 5. **Shutdown** — orderly: pipelines → threads → OS windows
///
/// ```ignore
/// App::new()
///     .window(WindowConfig::default(), build_ui)
///     .run()
///     .expect("failed to run");
/// ```
pub struct App {
    windows: Vec<WindowSpec>,
}

struct WindowSpec {
    config: kozan_platform::WindowConfig,
    init: Box<dyn FnOnce(&kozan_platform::ViewContext) + Send>,
}

impl App {
    /// Create a new application.
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
        }
    }

    /// Add a window to the application.
    ///
    /// Windows are created in order when the platform is ready.
    /// Chrome: entries queued during `PreMainMessageLoopRun()`.
    pub fn window(
        mut self,
        config: kozan_platform::WindowConfig,
        init: impl FnOnce(&kozan_platform::ViewContext) + Send + 'static,
    ) -> Self {
        self.windows.push(WindowSpec {
            config,
            init: Box::new(init),
        });
        self
    }

    /// Add a window with default configuration.
    pub fn default_window(
        self,
        init: impl FnOnce(&kozan_platform::ViewContext) + Send + 'static,
    ) -> Self {
        self.window(kozan_platform::WindowConfig::default(), init)
    }

    /// Run the application. **Blocks until all windows are closed.**
    ///
    /// Lifecycle:
    /// 1. GPU initialization (renderer creation)
    /// 2. Event loop creation (platform-specific)
    /// 3. Window creation (deferred until platform ready)
    /// 4. Main event loop (blocks)
    /// 5. Shutdown (orderly thread termination)
    ///
    /// Returns `Err` if GPU or event loop initialization fails.
    #[cfg(all(feature = "winit", feature = "vello"))]
    pub fn run(self) -> Result<()> {
        let renderer = kozan_vello::VelloRenderer::new().map_err(Error::Gpu)?;

        let pending = self
            .windows
            .into_iter()
            .map(|spec| kozan_winit::PendingWindow::new(spec.config, |ctx| (spec.init)(ctx)))
            .collect();

        kozan_winit::run(renderer, pending).map_err(Error::Platform)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
