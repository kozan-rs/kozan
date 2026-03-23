//! `Renderer` — the factory that creates per-window `RenderSurface`s.
//!
//! Chrome equivalent: `viz::GpuServiceImpl` — the process-level GPU manager
//! that creates `OutputSurface`s for each renderer.
//!
//! # Architecture
//!
//! ```text
//! Renderer (one per app, owns wgpu Device + Queue)
//!   └── create_surface(window_handle, w, h)
//!         └── RenderSurface (one per window, owns swap chain + vello renderer)
//! ```
//!
//! The `Renderer` trait is **backend-independent**. `kozan-vello` provides
//! `VelloRenderer: Renderer`. Future backends (software rasterizer, custom
//! GPU renderer) implement the same trait — zero changes to `kozan-platform`
//! or `kozan-winit`.
//!
//! # No winit dependency
//!
//! The window parameter uses `raw-window-handle` traits only.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use super::error::RendererError;
use super::surface::RenderSurface;

/// Process-level GPU renderer — creates per-window `RenderSurface`s.
///
/// Implemented by the renderer backend (e.g., `kozan-vello::VelloRenderer`).
/// One `Renderer` instance per application process.
///
/// # Safety contract for `create_surface`
///
/// The window passed to `create_surface` **must outlive** the returned
/// `RenderSurface`. This is upheld by `kozan-winit`'s `WindowState`, which
/// owns both the `Window` and the `RenderSurface` — the window is dropped
/// after the surface in `WindowState::shutdown()`.
pub trait Renderer: Send + Sync + 'static {
    /// The surface type this renderer produces.
    type Surface: RenderSurface;

    /// Create a rendering surface for a window.
    ///
    /// # Arguments
    ///
    /// - `window` — the OS window to render into (raw handle, no winit type)
    /// - `width` / `height` — initial viewport size in physical pixels
    ///
    /// # Safety
    ///
    /// `window` must remain valid (not dropped) for the entire lifetime of
    /// the returned `Surface`. See trait-level docs.
    fn create_surface<W>(
        &self,
        window: &W,
        width: u32,
        height: u32,
    ) -> Result<Self::Surface, RendererError>
    where
        W: HasWindowHandle + HasDisplayHandle;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_trait_is_object_safe_for_concrete_surface() {
        // Renderer has an associated type, so it's not fully object-safe
        // without specifying Surface. Verify it compiles as a trait bound.
        fn _accepts_renderer<R: Renderer>(_r: &R) {}
    }

    #[test]
    fn renderer_requires_send_sync_static() {
        fn _assert_send_sync_static<T: Send + Sync + 'static>() {}
        // This confirms the supertrait bounds compile for any implementor.
        fn _check<R: Renderer>() {
            _assert_send_sync_static::<R>();
        }
    }
}
