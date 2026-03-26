//! Platform host — abstract interface from view threads to the main thread.
//!
//! Like Chrome's `WidgetHost` / `FrameWidgetHost` Mojo interfaces.
//! The view thread communicates back to the main thread through this trait,
//! never through windowing-backend types (winit, SDL2, etc.).
//!
//! The windowing backend (e.g., `kozan-winit`) provides the concrete
//! implementation.

use crate::id::WindowId;
use crate::request::WindowConfig;

/// The interface from a view thread back to the main thread.
///
/// Implemented by the windowing backend (e.g., `kozan-winit`).
/// The view thread holds an `Arc<dyn PlatformHost>` and calls methods
/// without knowing anything about the underlying windowing system.
///
/// All methods are non-blocking — they send messages to the main thread.
/// If the main thread has exited, calls are silently dropped.
///
/// Query methods (`window_count`, `renderer_name`) read shared atomic
/// state directly — no message round-trip. Updated by the main thread,
/// readable from any view thread.
pub trait PlatformHost: Send + Sync {
    /// Request a redraw for this window.
    fn request_redraw(&self, window_id: WindowId);

    /// Set the window title.
    fn set_title(&self, window_id: WindowId, title: &str);

    /// Close the window.
    fn close_window(&self, window_id: WindowId);

    /// Resize the window.
    fn resize_window(&self, window_id: WindowId, width: u32, height: u32);

    /// Request a new window to be created.
    fn create_window(&self, config: WindowConfig);

    // ── Queries (lock-free reads of shared state) ────────────

    /// Number of open windows across the application.
    fn window_count(&self) -> u32 {
        0
    }

    /// Renderer backend name (e.g., "Vello/wgpu").
    fn renderer_name(&self) -> &str {
        "unknown"
    }
}
