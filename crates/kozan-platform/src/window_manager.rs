//! Window manager — owns all windows, routes events, holds the renderer.
//!
//! Chrome: `BrowserMainThread` manages `RenderWidgetHostImpl` instances.
//! The OS adapter (kozan-winit) calls methods here — it never touches
//! threads, channels, or input state directly.
//!
//! Generic over `R: Renderer` — the renderer backend is injected at app
//! startup. Neither the OS adapter nor the renderer know about each other;
//! both only know about the traits defined in `kozan-platform`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use kozan_core::input::*;
use kozan_primitives::geometry::{Offset, Point};

use crate::context::ViewContext;
use crate::event::{LifecycleEvent, ViewEvent};
use crate::host::PlatformHost;
use crate::id::WindowId;
use crate::pipeline::render_loop::{OnSurfaceLost, RenderEvent};
use crate::pipeline::{PipelineConfig, ViewportInfo, WindowPipeline};
use crate::renderer::{Renderer, RendererError};
use crate::view_thread::SpawnError;
use crate::window_state::WindowState;

/// Configuration for creating a window through the manager.
///
/// Contains everything except the window handle and the renderer —
/// the manager holds the renderer, and the OS adapter passes the handle.
pub struct WindowCreateConfig {
    pub window_id: WindowId,
    pub host: Arc<dyn PlatformHost>,
    pub on_surface_lost: OnSurfaceLost,
    pub viewport: ViewportInfo,
    /// Called before every `present()` on the render thread.
    /// On X11: increments the `_NET_WM_SYNC_REQUEST` counter.
    pub pre_present_hook: Option<Box<dyn Fn() + Send>>,
}

/// Error when window creation fails.
#[derive(Debug)]
pub enum CreateWindowError {
    Renderer(RendererError),
    Spawn(SpawnError),
}

impl std::fmt::Display for CreateWindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Renderer(e) => write!(f, "renderer: {e}"),
            Self::Spawn(e) => write!(f, "spawn: {e}"),
        }
    }
}

impl std::error::Error for CreateWindowError {}

/// Owns all windows. The OS adapter calls methods on this.
///
/// Generic over `R: Renderer` — the renderer creates per-window GPU
/// surfaces from raw window handles. The OS adapter provides the handles,
/// the renderer creates the surfaces, and this manager owns the pipelines.
pub struct WindowManager<R: Renderer> {
    renderer: R,
    windows: HashMap<WindowId, WindowState>,
}

impl<R: Renderer> WindowManager<R> {
    pub fn new(renderer: R) -> Self {
        Self {
            renderer,
            windows: HashMap::new(),
        }
    }

    /// Create a window: create GPU surface from handle, spawn view + render threads.
    ///
    /// The OS adapter creates the OS window and passes the raw handle here.
    /// This manager uses the renderer to create the surface, then spawns
    /// the pipeline (render + view threads). The OS adapter keeps the window
    /// handle alive until after `close_window` returns.
    pub fn create_window<W, F>(
        &mut self,
        window_handle: &W,
        config: WindowCreateConfig,
        view_init: F,
    ) -> Result<(), CreateWindowError>
    where
        W: HasWindowHandle + HasDisplayHandle,
        F: FnOnce(&ViewContext) + Send + 'static,
    {
        let vp = config.viewport;
        let surface = self
            .renderer
            .create_surface(window_handle, vp.width, vp.height)
            .map_err(CreateWindowError::Renderer)?;

        let pipeline_config = PipelineConfig {
            surface,
            on_surface_lost: config.on_surface_lost,
            host: config.host,
            window_id: config.window_id,
            viewport: vp,
            pre_present_hook: config.pre_present_hook,
        };

        let pipeline =
            WindowPipeline::spawn(pipeline_config, view_init).map_err(CreateWindowError::Spawn)?;

        self.windows.insert(
            config.window_id,
            WindowState::new(pipeline, vp.scale_factor),
        );
        Ok(())
    }

    pub fn close_window(&mut self, id: WindowId) {
        if let Some(mut state) = self.windows.remove(&id) {
            state.shutdown();
        }
    }

    pub fn has_windows(&self) -> bool {
        !self.windows.is_empty()
    }

    pub fn shutdown_all(&mut self) {
        for (_, mut state) in self.windows.drain() {
            state.shutdown();
        }
    }

    /// Scale factor for a window — used by the OS adapter for pixel delta conversion.
    pub fn scale_factor(&self, id: WindowId) -> Option<f64> {
        self.windows.get(&id).map(|s| s.input().scale_factor())
    }

    // ── Lifecycle events ─────────────────────────────────────

    pub fn on_resize(&mut self, id: WindowId, width: u32, height: u32) {
        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };
        state.send_to_render(RenderEvent::Resize { width, height });
        state.send_to_view(ViewEvent::Lifecycle(LifecycleEvent::Resized {
            width,
            height,
        }));
    }

    pub fn on_scale_factor_changed(
        &mut self,
        id: WindowId,
        scale_factor: f64,
        refresh_rate_millihertz: Option<u32>,
    ) {
        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };
        state.input_mut().set_scale_factor(scale_factor);
        state.send_to_render(RenderEvent::ScaleFactorChanged(scale_factor));
        state.send_to_view(ViewEvent::Lifecycle(LifecycleEvent::ScaleFactorChanged {
            scale_factor,
            refresh_rate_millihertz,
        }));
    }

    pub fn on_focus_changed(&mut self, id: WindowId, focused: bool) {
        let Some(state) = self.windows.get(&id) else {
            return;
        };
        state.send_to_view(ViewEvent::Lifecycle(LifecycleEvent::Focused(focused)));
    }

    /// Trigger a repaint — sends Paint to the view thread.
    pub fn on_redraw(&self, id: WindowId) {
        let Some(state) = self.windows.get(&id) else {
            return;
        };
        state.send_to_view(ViewEvent::Paint);
    }

    // ── Mouse events ─────────────────────────────────────────

    pub fn on_cursor_moved(&mut self, id: WindowId, physical_x: f64, physical_y: f64) {
        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };
        state
            .input_mut()
            .set_cursor_physical(physical_x, physical_y);
        let (x, y) = state.input().cursor();
        let modifiers = state.input().modifiers();
        state.send_to_view(ViewEvent::Input(InputEvent::MouseMove(MouseMoveEvent {
            x,
            y,
            modifiers,
            timestamp: Instant::now(),
        })));
    }

    pub fn on_cursor_entered(&mut self, id: WindowId) {
        let Some(state) = self.windows.get(&id) else {
            return;
        };
        let (x, y) = state.input().cursor();
        let modifiers = state.input().modifiers();
        state.send_to_view(ViewEvent::Input(InputEvent::MouseEnter(MouseEnterEvent {
            x,
            y,
            modifiers,
            timestamp: Instant::now(),
        })));
    }

    pub fn on_cursor_left(&mut self, id: WindowId) {
        let Some(state) = self.windows.get(&id) else {
            return;
        };
        let modifiers = state.input().modifiers();
        state.send_to_view(ViewEvent::Input(InputEvent::MouseLeave(MouseLeaveEvent {
            modifiers,
            timestamp: Instant::now(),
        })));
    }

    pub fn on_mouse_input(&mut self, id: WindowId, button: MouseButton, btn_state: ButtonState) {
        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };
        state.input_mut().update_button_modifier(&button, btn_state);
        let (x, y) = state.input().cursor();
        let modifiers = state.input().modifiers();
        state.send_to_view(ViewEvent::Input(InputEvent::MouseButton(
            MouseButtonEvent {
                x,
                y,
                button,
                state: btn_state,
                modifiers,
                click_count: 1,
                timestamp: Instant::now(),
            },
        )));
    }

    // ── Scroll ───────────────────────────────────────────────

    pub fn on_mouse_wheel(&mut self, id: WindowId, delta: WheelDelta) {
        let Some(state) = self.windows.get(&id) else {
            return;
        };
        let (x, y) = state.input().cursor();
        let modifiers = state.input().modifiers();

        state.send_to_render(RenderEvent::Scroll {
            delta: Offset::new(-delta.px_dx(), -delta.px_dy()),
            point: Point::new(x as f32, y as f32),
        });

        state.send_to_view(ViewEvent::Input(InputEvent::Wheel(wheel::WheelEvent {
            x,
            y,
            delta,
            modifiers,
            timestamp: Instant::now(),
        })));
    }

    // ── Keyboard ─────────────────────────────────────────────

    pub fn on_keyboard_input(
        &mut self,
        id: WindowId,
        key: KeyCode,
        key_state: ButtonState,
        text: Option<String>,
        repeat: bool,
    ) {
        let Some(state) = self.windows.get(&id) else {
            return;
        };
        let mut modifiers = state.input().modifiers();
        if repeat {
            modifiers = modifiers.with_auto_repeat();
        }
        state.send_to_view(ViewEvent::Input(InputEvent::Keyboard(
            keyboard::KeyboardEvent {
                key,
                state: key_state,
                modifiers,
                text,
                timestamp: Instant::now(),
            },
        )));
    }

    pub fn on_modifiers_changed(
        &mut self,
        id: WindowId,
        shift: bool,
        ctrl: bool,
        alt: bool,
        meta: bool,
    ) {
        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };
        state
            .input_mut()
            .set_modifiers_from_keyboard(shift, ctrl, alt, meta);
    }
}
