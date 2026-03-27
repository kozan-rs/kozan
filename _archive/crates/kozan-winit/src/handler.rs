//! App handler — thin winit adapter, routes OS events to platform.
//!
//! Chrome: Browser Process main loop. Pure event router — ZERO rendering,
//! ZERO compositing, ZERO input state tracking. All logic lives in
//! `kozan-platform::WindowManager`.

use std::collections::HashMap;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId as WinitWindowId};

use kozan_core::input::*;
use kozan_platform::host::PlatformHost;
use kozan_platform::id::WindowId;
use kozan_platform::renderer::Renderer;
use kozan_platform::request::WindowConfig;
use kozan_platform::{ViewContext, ViewportInfo, WindowCreateConfig, WindowManager};

use crate::convert::*;
use crate::host_impl::{InternalRequest, WinitPlatformHost};

/// Stored window config + init closure, waiting for `resumed()`.
pub(crate) struct PendingWindow {
    pub config: WindowConfig,
    pub view_init: Box<dyn FnOnce(&ViewContext) + Send>,
}

/// The winit `ApplicationHandler`. Pure event router — delegates everything
/// to `WindowManager` in kozan-platform.
///
/// Owns only:
/// - OS window handles (must outlive GPU surfaces — dropped after pipeline shutdown)
/// - ID mappings (winit ↔ kozan)
/// - Pending windows (deferred until `resumed()`)
/// - Reference to host (for SurfaceLost callbacks)
pub(crate) struct AppHandler<R: Renderer> {
    manager: WindowManager<R>,
    os_windows: HashMap<WinitWindowId, Arc<Window>>,
    winit_to_kozan: HashMap<WinitWindowId, WindowId>,
    kozan_to_winit: HashMap<WindowId, WinitWindowId>,
    pending: Vec<PendingWindow>,
    host: Arc<WinitPlatformHost>,
}

impl<R: Renderer> AppHandler<R> {
    pub fn new(renderer: R, pending: Vec<PendingWindow>, host: Arc<WinitPlatformHost>) -> Self {
        Self {
            manager: WindowManager::new(renderer),
            os_windows: HashMap::new(),
            winit_to_kozan: HashMap::new(),
            kozan_to_winit: HashMap::new(),
            pending,
            host,
        }
    }

    // ── Window lifecycle ─────────────────────────────────────

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        config: WindowConfig,
        view_init: Box<dyn FnOnce(&ViewContext) + Send>,
    ) {
        let window = match self.create_os_window(event_loop, &config) {
            Some(w) => Arc::new(w),
            None => return,
        };

        let winit_id = window.id();
        let kozan_id = WindowId::next();
        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        let refresh_rate = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz());

        let proxy = self.host.proxy();
        let lost_id = kozan_id;
        let on_surface_lost = Box::new(move || {
            let _ = proxy.send_event(InternalRequest::SurfaceLost(lost_id));
        });

        // X11: increments the _NET_WM_SYNC_REQUEST counter so the WM
        // waits for the new frame before showing the new window geometry.
        // Wayland: ack_configure equivalent. Other platforms: no-op.
        let pre_present_window = Arc::clone(&window);
        let pre_present_hook: Box<dyn Fn() + Send> = Box::new(move || {
            pre_present_window.pre_present_notify();
        });

        let create_config = WindowCreateConfig {
            window_id: kozan_id,
            host: self.host.clone() as Arc<dyn PlatformHost>,
            on_surface_lost,
            viewport: ViewportInfo {
                width: size.width,
                height: size.height,
                scale_factor,
                refresh_rate_millihertz: refresh_rate,
            },
            pre_present_hook: Some(pre_present_hook),
        };

        if let Err(e) = self
            .manager
            .create_window(&window, create_config, move |ctx| view_init(ctx))
        {
            eprintln!("kozan: failed to create window: {e}");
            return;
        }

        self.host.increment_windows();
        self.kozan_to_winit.insert(kozan_id, winit_id);
        self.winit_to_kozan.insert(winit_id, kozan_id);
        self.os_windows.insert(winit_id, window);
    }

    fn create_os_window(
        &self,
        event_loop: &ActiveEventLoop,
        config: &WindowConfig,
    ) -> Option<Window> {
        let attrs = Window::default_attributes()
            .with_title(&config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                config.width as f64,
                config.height as f64,
            ))
            .with_resizable(config.resizable)
            .with_decorations(config.decorations);

        match event_loop.create_window(attrs) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("kozan: failed to create OS window: {e}");
                None
            }
        }
    }

    /// Shuts down the pipeline FIRST (joins threads, drops GPU surface),
    /// THEN drops the OS window handle. This ordering is safety-critical:
    /// the GPU surface holds a raw pointer to the OS window.
    fn remove_window(&mut self, winit_id: WinitWindowId) {
        if let Some(kozan_id) = self.winit_to_kozan.remove(&winit_id) {
            self.manager.close_window(kozan_id);
            self.kozan_to_winit.remove(&kozan_id);
            self.host.decrement_windows();
        }
        self.os_windows.remove(&winit_id);
    }
}

// ── ApplicationHandler ───────────────────────────────────────

impl<R: Renderer> ApplicationHandler<InternalRequest> for AppHandler<R> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        for pw in std::mem::take(&mut self.pending) {
            self.create_window(event_loop, pw.config, pw.view_init);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        winit_id: WinitWindowId,
        event: WindowEvent,
    ) {
        let Some(&kozan_id) = self.winit_to_kozan.get(&winit_id) else {
            return;
        };

        match event {
            // ── Lifecycle ────────────────────────────────────
            WindowEvent::CloseRequested => {
                self.remove_window(winit_id);
                if self.os_windows.is_empty() {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                self.manager.on_resize(kozan_id, size.width, size.height);
            }

            WindowEvent::Focused(focused) => {
                self.manager.on_focus_changed(kozan_id, focused);
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let refresh_rate = self
                    .os_windows
                    .get(&winit_id)
                    .and_then(|w| w.current_monitor())
                    .and_then(|m| m.refresh_rate_millihertz());
                self.manager
                    .on_scale_factor_changed(kozan_id, scale_factor, refresh_rate);
            }

            WindowEvent::RedrawRequested => {
                self.manager.on_redraw(kozan_id);
            }

            // ── Keyboard ─────────────────────────────────────
            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                self.manager.on_modifiers_changed(
                    kozan_id,
                    state.shift_key(),
                    state.control_key(),
                    state.alt_key(),
                    state.super_key(),
                );
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let ke = kozan_core::input::keyboard::KeyboardEvent {
                    physical_key: convert_key_code(&event.physical_key),
                    logical_key: convert_key(&event.logical_key),
                    state: convert_button_state(event.state),
                    modifiers: kozan_core::Modifiers::EMPTY,
                    location: convert_key_location(event.location),
                    text: event.text.map(|s| s.to_string()),
                    repeat: event.repeat,
                    timestamp: std::time::Instant::now(),
                };
                self.manager.on_keyboard_input(kozan_id, ke);
            }

            // ── Mouse ────────────────────────────────────────
            WindowEvent::CursorMoved { position, .. } => {
                self.manager
                    .on_cursor_moved(kozan_id, position.x, position.y);
            }

            WindowEvent::CursorEntered { .. } => {
                self.manager.on_cursor_entered(kozan_id);
            }

            WindowEvent::CursorLeft { .. } => {
                self.manager.on_cursor_left(kozan_id);
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let button = convert_mouse_button(button);
                let state = convert_button_state(state);
                self.manager.on_mouse_input(kozan_id, button, state);
            }

            // ── Scroll ───────────────────────────────────────
            WindowEvent::MouseWheel { delta, .. } => {
                let wheel_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(lx, ly) => WheelDelta::Lines(lx, ly),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        let scale = self.manager.scale_factor(kozan_id).unwrap_or(1.0);
                        WheelDelta::Pixels(pos.x / scale, pos.y / scale)
                    }
                };
                self.manager.on_mouse_wheel(kozan_id, wheel_delta);
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: InternalRequest) {
        match event {
            InternalRequest::Redraw(kozan_id) => {
                if let Some(&winit_id) = self.kozan_to_winit.get(&kozan_id) {
                    if let Some(window) = self.os_windows.get(&winit_id) {
                        window.request_redraw();
                    }
                }
            }
            InternalRequest::SetTitle { window_id, title } => {
                if let Some(&winit_id) = self.kozan_to_winit.get(&window_id) {
                    if let Some(window) = self.os_windows.get(&winit_id) {
                        window.set_title(&title);
                    }
                }
            }
            InternalRequest::CloseWindow(kozan_id) => {
                if let Some(&winit_id) = self.kozan_to_winit.get(&kozan_id) {
                    self.remove_window(winit_id);
                    if self.os_windows.is_empty() {
                        event_loop.exit();
                    }
                }
            }
            InternalRequest::CreateWindow { config } => {
                self.create_window(event_loop, config, Box::new(|_| {}));
            }
            InternalRequest::ResizeWindow {
                window_id,
                width,
                height,
            } => {
                if let Some(&winit_id) = self.kozan_to_winit.get(&window_id) {
                    if let Some(window) = self.os_windows.get(&winit_id) {
                        let _ =
                            window.request_inner_size(winit::dpi::PhysicalSize::new(width, height));
                    }
                }
            }
            InternalRequest::SurfaceLost(kozan_id) => {
                eprintln!("kozan: surface lost for window {kozan_id:?}, closing");
                if let Some(&winit_id) = self.kozan_to_winit.get(&kozan_id) {
                    self.remove_window(winit_id);
                    if self.os_windows.is_empty() {
                        event_loop.exit();
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}
}
