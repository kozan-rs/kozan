//! Window pipeline — per-window view + render thread orchestration.
//!
//! Chrome: `RenderWidgetHost` — creates channels, spawns threads, routes events.
//! All channels created before any thread starts — no chicken-and-egg.

pub mod input_state;
pub mod render_loop;
pub mod view_loop;

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use kozan_core::widget::FrameWidget;
use kozan_scheduler::{Scheduler, WakeSender};

use crate::context::ViewContext;
use crate::event::ViewEvent;
use crate::host::PlatformHost;
use crate::id::WindowId;
use crate::renderer::RenderSurface;
use crate::view_thread::{SpawnError, ViewThreadHandle};

use self::render_loop::{OnSurfaceLost, RenderEvent, RenderLoop};

#[derive(Clone, Copy)]
pub struct ViewportInfo {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub refresh_rate_millihertz: Option<u32>,
}

pub struct PipelineConfig<S> {
    pub surface: S,
    pub on_surface_lost: OnSurfaceLost,
    pub host: Arc<dyn PlatformHost>,
    pub window_id: WindowId,
    pub viewport: ViewportInfo,
    /// Called before every `present()`. On X11 this increments the
    /// `_NET_WM_SYNC_REQUEST` counter so the WM shows new geometry
    /// only after the matching frame is ready.
    pub pre_present_hook: Option<Box<dyn Fn() + Send>>,
}

pub struct RenderThreadHandle {
    sender: mpsc::Sender<RenderEvent>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl RenderThreadHandle {
    #[must_use] 
    pub fn send(&self, event: RenderEvent) -> bool {
        self.sender.send(event).is_ok()
    }

    pub fn shutdown(&mut self) {
        let _ = self.sender.send(RenderEvent::Shutdown);
        if let Some(h) = self.join_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for RenderThreadHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Per-window thread pair.
pub struct WindowPipeline {
    render_handle: RenderThreadHandle,
    view_handle: ViewThreadHandle,
    render_tx: mpsc::Sender<RenderEvent>,
}

impl WindowPipeline {
    pub fn spawn<S, F>(config: PipelineConfig<S>, view_init: F) -> Result<Self, SpawnError>
    where
        S: RenderSurface + 'static,
        F: FnOnce(&ViewContext) + Send + 'static,
    {
        let vp = config.viewport;
        let (render_tx, render_rx) = mpsc::channel();
        let (view_tx, view_rx) = mpsc::channel();

        let mut surface = config.surface;
        if let Some(hook) = config.pre_present_hook {
            surface.set_pre_present_hook(hook);
        }

        let render_join = spawn_render(RenderDeps {
            surface,
            on_lost: config.on_surface_lost,
            rx: render_rx,
            view_tx: view_tx.clone(),
            viewport: vp,
        })?;

        let view_handle = spawn_view(
            ViewDeps {
                rx: view_rx,
                tx: view_tx,
                render_tx: render_tx.clone(),
                host: config.host,
                window_id: config.window_id,
                viewport: vp,
            },
            view_init,
        )?;

        Ok(Self {
            render_handle: RenderThreadHandle {
                sender: render_tx.clone(),
                join_handle: Some(render_join),
            },
            view_handle,
            render_tx,
        })
    }

    #[must_use] 
    pub fn send_to_view(&self, event: ViewEvent) -> bool {
        self.view_handle.send(event)
    }

    #[must_use] 
    pub fn send_to_render(&self, event: RenderEvent) -> bool {
        self.render_tx.send(event).is_ok()
    }

    pub fn shutdown(&mut self) {
        self.render_handle.shutdown();
        self.view_handle.shutdown();
    }
}

impl Drop for WindowPipeline {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ── Render thread ────────────────────────────────────────────

struct RenderDeps<S> {
    surface: S,
    on_lost: OnSurfaceLost,
    rx: mpsc::Receiver<RenderEvent>,
    view_tx: mpsc::Sender<ViewEvent>,
    viewport: ViewportInfo,
}

fn spawn_render<S: RenderSurface + 'static>(
    deps: RenderDeps<S>,
) -> Result<thread::JoinHandle<()>, SpawnError> {
    let vp = deps.viewport;
    thread::Builder::new()
        .name("kozan-render".into())
        .spawn(move || {
            RenderLoop::new(
                deps.surface,
                deps.view_tx,
                deps.on_lost,
                vp.width,
                vp.height,
                vp.scale_factor,
            )
            .run(deps.rx);
        })
        .map_err(SpawnError::ThreadSpawn)
}

// ── View thread ──────────────────────────────────────────────

struct ViewDeps {
    rx: mpsc::Receiver<ViewEvent>,
    tx: mpsc::Sender<ViewEvent>,
    render_tx: mpsc::Sender<RenderEvent>,
    host: Arc<dyn PlatformHost>,
    window_id: WindowId,
    viewport: ViewportInfo,
}

fn spawn_view<F: FnOnce(&ViewContext) + Send + 'static>(
    deps: ViewDeps,
    init: F,
) -> Result<ViewThreadHandle, SpawnError> {
    let (ws_tx, ws_rx) = mpsc::sync_channel::<WakeSender>(1);
    let tx_clone = deps.tx.clone();

    let join = thread::Builder::new()
        .name("kozan-view".into())
        .spawn(move || view_main(deps, ws_tx, init))
        .map_err(SpawnError::ThreadSpawn)?;

    let wake = ws_rx.recv().map_err(|_| SpawnError::SetupFailed)?;
    Ok(ViewThreadHandle::from_parts(tx_clone, wake, join))
}

fn view_main<F: FnOnce(&ViewContext)>(
    deps: ViewDeps,
    ws_tx: mpsc::SyncSender<WakeSender>,
    init: F,
) {
    let (mut scheduler, wake) = new_scheduler(&deps);
    if ws_tx.send(wake.clone()).is_err() {
        return;
    }

    let mut ctx = new_view_context(&deps, wake);
    init(&ctx);
    for future in ctx.take_staged_futures() {
        scheduler.spawn(future);
    }

    ctx.invalidate_style();
    scheduler.set_needs_frame();
    crate::pipeline::view_loop::run(&mut scheduler, &mut ctx, &deps.rx);
}

fn new_scheduler(deps: &ViewDeps) -> (Scheduler, WakeSender) {
    let (mut sched, mut wake) = Scheduler::new();

    if let Some(mhz) = deps.viewport.refresh_rate_millihertz {
        let budget = std::time::Duration::from_micros(1_000_000_000 / mhz as u64);
        sched.frame_scheduler_mut().set_frame_budget(budget);
    }

    let tx = deps.tx.clone();
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = tx.send(ViewEvent::Paint);
    });
    wake.set_notify(Arc::clone(&notify));
    sched.set_executor_notify(notify);

    (sched, wake)
}

fn new_view_context(deps: &ViewDeps, wake: WakeSender) -> ViewContext {
    let mut frame = FrameWidget::new();
    frame.resize(deps.viewport.width, deps.viewport.height);
    frame.set_scale_factor(deps.viewport.scale_factor);
    ViewContext::new(
        frame,
        wake,
        deps.host.clone(),
        deps.window_id,
        deps.render_tx.clone(),
    )
}
