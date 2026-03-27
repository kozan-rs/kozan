//! Concrete `PlatformHost` backed by winit's `EventLoopProxy`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use winit::event_loop::EventLoopProxy;

use kozan_platform::host::PlatformHost;
use kozan_platform::id::WindowId;
use kozan_platform::request::WindowConfig;

/// Internal request type — uses Kozan's own types, sent via winit proxy.
#[derive(Debug)]
pub(crate) enum InternalRequest {
    Redraw(WindowId),
    SetTitle {
        window_id: WindowId,
        title: String,
    },
    CloseWindow(WindowId),
    ResizeWindow {
        window_id: WindowId,
        width: u32,
        height: u32,
    },
    CreateWindow {
        config: WindowConfig,
    },
    /// Render thread lost its GPU surface — window needs recreation or close.
    SurfaceLost(WindowId),
}

/// winit-backed `PlatformHost`.
///
/// Atomic counters are updated by the main thread (handler) and read
/// lock-free from any view thread via `PlatformHost` query methods.
pub(crate) struct WinitPlatformHost {
    proxy: EventLoopProxy<InternalRequest>,
    window_count: AtomicU32,
    renderer_name: &'static str,
}

impl WinitPlatformHost {
    pub fn new(proxy: EventLoopProxy<InternalRequest>, renderer_name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            proxy,
            window_count: AtomicU32::new(0),
            renderer_name,
        })
    }

    pub(crate) fn proxy(&self) -> EventLoopProxy<InternalRequest> {
        self.proxy.clone()
    }

    /// Called by the handler when a window is successfully created.
    pub(crate) fn increment_windows(&self) {
        self.window_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Called by the handler when a window is closed.
    pub(crate) fn decrement_windows(&self) {
        self.window_count.fetch_sub(1, Ordering::Relaxed);
    }
}

impl PlatformHost for WinitPlatformHost {
    fn request_redraw(&self, window_id: WindowId) {
        let _ = self.proxy.send_event(InternalRequest::Redraw(window_id));
    }

    fn set_title(&self, window_id: WindowId, title: &str) {
        let _ = self.proxy.send_event(InternalRequest::SetTitle {
            window_id,
            title: title.to_string(),
        });
    }

    fn close_window(&self, window_id: WindowId) {
        let _ = self
            .proxy
            .send_event(InternalRequest::CloseWindow(window_id));
    }

    fn resize_window(&self, window_id: WindowId, width: u32, height: u32) {
        let _ = self.proxy.send_event(InternalRequest::ResizeWindow {
            window_id,
            width,
            height,
        });
    }

    fn create_window(&self, config: WindowConfig) {
        let _ = self
            .proxy
            .send_event(InternalRequest::CreateWindow { config });
    }

    fn window_count(&self) -> u32 {
        self.window_count.load(Ordering::Relaxed)
    }

    fn renderer_name(&self) -> &str {
        self.renderer_name
    }
}
