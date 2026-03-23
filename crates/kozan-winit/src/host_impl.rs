//! Concrete `PlatformHost` backed by winit's `EventLoopProxy`.

use std::sync::Arc;
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
pub(crate) struct WinitPlatformHost {
    proxy: EventLoopProxy<InternalRequest>,
}

impl WinitPlatformHost {
    pub fn new(proxy: EventLoopProxy<InternalRequest>) -> Arc<Self> {
        Arc::new(Self { proxy })
    }

    pub(crate) fn proxy(&self) -> EventLoopProxy<InternalRequest> {
        self.proxy.clone()
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
}
