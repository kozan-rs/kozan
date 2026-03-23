//! `RenderSurface` — the per-window rendering target.
//!
//! Chrome: `viz::OutputSurface` / `cc::OutputSurface`.
//!
//! A `RenderSurface` is created once per OS window by the `Renderer`.
//! It owns the GPU swap chain for that window and renders one frame at a time.

use kozan_core::compositor::frame::CompositorFrame;

use super::error::RendererError;

/// Everything the renderer needs to produce one frame of pixels.
pub struct RenderParams<'a> {
    /// Compositor output: display list + scroll adjustments.
    pub frame: &'a CompositorFrame,
    /// Viewport width in physical pixels.
    pub width: u32,
    /// Viewport height in physical pixels.
    pub height: u32,
    /// DPI scale — logical pixels × scale_factor = physical pixels.
    pub scale_factor: f64,
}

/// A per-window GPU rendering target.
///
/// Implemented by the renderer backend (e.g., `kozan-vello::VelloSurface`).
/// One `RenderSurface` per OS window.
pub trait RenderSurface: Send {
    fn render(&mut self, params: &RenderParams) -> Result<(), RendererError>;

    /// Notify the surface that the window was resized.
    fn resize(&mut self, width: u32, height: u32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_surface_is_object_safe() {
        fn _accepts_dyn(_s: &dyn RenderSurface) {}
    }

    #[test]
    fn render_surface_requires_send() {
        fn _assert_send<T: Send>() {}
        fn _check<S: RenderSurface>() {
            _assert_send::<S>();
        }
    }
}
