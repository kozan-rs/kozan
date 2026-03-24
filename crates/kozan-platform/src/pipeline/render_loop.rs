//! Render loop — per-window compositor + vsync loop.
//!
//! Chrome: `cc::LayerTreeHostImpl` on the compositor thread.
//! Runs on its own thread, owns the GPU surface and Compositor.
//! Handles scroll at vsync rate without view thread involvement.

use std::sync::mpsc;

use kozan_core::compositor::Compositor;
use kozan_primitives::geometry::{Offset, Point};

use crate::context::FrameOutput;
use crate::event::ViewEvent;
use crate::renderer::{RenderParams, RenderSurface, RendererError};

/// Events received by the render loop.
///
/// Chrome: tasks posted to the compositor thread's task queue.
/// Each variant transfers ownership — no shared state.
pub enum RenderEvent {
    /// View thread finished painting — commit to compositor.
    Commit(FrameOutput),
    /// Surface resized (physical pixels).
    Resize { width: u32, height: u32 },
    /// DPI scale factor changed.
    ScaleFactorChanged(f64),
    /// Wheel/touch scroll — compositor handles directly.
    /// Carries cursor position for hit testing which scroller to target.
    Scroll { delta: Offset, point: Point },
    /// Clean shutdown.
    Shutdown,
}

/// Error callback — called when the GPU surface is lost.
/// The platform-specific adapter provides this.
pub type OnSurfaceLost = Box<dyn FnOnce() + Send>;

/// The per-window render loop. Generic over the GPU surface.
///
/// Chrome: `LayerTreeHostImpl` + `OutputSurface` + `SchedulerStateMachine`.
pub(crate) struct RenderLoop<S> {
    surface: S,
    compositor: Compositor,
    view_tx: mpsc::Sender<ViewEvent>,
    on_surface_lost: Option<OnSurfaceLost>,
    width: u32,
    height: u32,
    scale_factor: f64,
    queued_scrolls: Vec<(Offset, Point)>,
    /// When `true`, the surface was reconfigured for a new size and we must
    /// wait for the view thread to commit a matching frame before presenting.
    /// Chrome: `SynchronizeVisualProperties` — never show stale-size content.
    awaiting_resize_commit: bool,
}

impl<S: RenderSurface> RenderLoop<S> {
    pub fn new(
        surface: S,
        view_tx: mpsc::Sender<ViewEvent>,
        on_surface_lost: OnSurfaceLost,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        Self {
            surface,
            compositor: Compositor::new(),
            view_tx,
            on_surface_lost: Some(on_surface_lost),
            width,
            height,
            scale_factor,
            queued_scrolls: Vec::new(),
            awaiting_resize_commit: false,
        }
    }

    /// The main entry point — runs until Shutdown or channel disconnect.
    pub fn run(&mut self, rx: mpsc::Receiver<RenderEvent>) {
        if !self.wait_for_first_commit(&rx) {
            return;
        }
        loop {
            if !self.drain_events(&rx) {
                return;
            }
            // Sync resize: surface was reconfigured — block until the view
            // thread commits a frame at the new size so we never present
            // stale content stretched to the wrong dimensions.
            if self.awaiting_resize_commit {
                if !self.wait_for_resize_commit(&rx) {
                    return;
                }
                // Frame was already rendered + presented inside
                // wait_for_resize_commit.
                continue;
            }
            if !self.render_frame() {
                return;
            }
        }
    }

    fn wait_for_first_commit(&mut self, rx: &mpsc::Receiver<RenderEvent>) -> bool {
        loop {
            match rx.recv() {
                Ok(RenderEvent::Commit(output)) => {
                    self.commit(output);
                    self.replay_queued_scrolls();
                    return true;
                }
                Ok(RenderEvent::Shutdown) | Err(_) => return false,
                Ok(event) => self.handle_event(event),
            }
        }
    }

    fn drain_events(&mut self, rx: &mpsc::Receiver<RenderEvent>) -> bool {
        loop {
            match rx.try_recv() {
                Ok(RenderEvent::Shutdown) => return false,
                Ok(event) => self.handle_event(event),
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => return false,
            }
        }
    }

    fn render_frame(&mut self) -> bool {
        if let Some(frame) = self.compositor.produce_frame() {
            let params = RenderParams {
                frame: &frame,
                width: self.width,
                height: self.height,
                scale_factor: self.scale_factor,
            };
            match self.surface.render(&params) {
                Ok(()) => return true,
                Err(RendererError::SurfaceLost) => {
                    if let Some(cb) = self.on_surface_lost.take() {
                        cb();
                    }
                    return false;
                }
                Err(e) => {
                    eprintln!("kozan: render error: {e}");
                    return true;
                }
            }
        }
        // No content — wait for next event.
        true
    }

    /// Block until a `Commit` arrives whose viewport matches the current
    /// surface size.
    ///
    /// Chrome: the display compositor waits for a `CompositorFrame` whose
    /// `LocalSurfaceId` matches the new size before presenting. We do the
    /// same — block here so the user never sees the old frame stretched
    /// onto the new surface. Stale commits (wrong size) are silently
    /// dropped. Intermediate `Resize` events are coalesced.
    fn wait_for_resize_commit(&mut self, rx: &mpsc::Receiver<RenderEvent>) -> bool {
        loop {
            match rx.recv() {
                Ok(RenderEvent::Commit(output)) => {
                    if output.viewport_width == self.width
                        && output.viewport_height == self.height
                    {
                        // Matching size — commit, render, present.
                        self.commit(output);
                        self.awaiting_resize_commit = false;
                        return self.render_frame();
                    }
                    // Stale commit from before the resize — drop it.
                }
                Ok(RenderEvent::Resize { width, height }) => {
                    // Coalesce: update to latest size without rendering in between.
                    self.width = width;
                    self.height = height;
                    self.surface.resize(width, height);
                }
                Ok(RenderEvent::Shutdown) | Err(_) => return false,
                Ok(other) => self.handle_non_resize(other),
            }
        }
    }

    fn handle_event(&mut self, event: RenderEvent) {
        match event {
            RenderEvent::Commit(output) => self.commit(output),
            RenderEvent::Resize { width, height } => {
                self.width = width;
                self.height = height;
                self.surface.resize(width, height);
                self.awaiting_resize_commit = true;
            }
            RenderEvent::ScaleFactorChanged(sf) => self.scale_factor = sf,
            RenderEvent::Scroll { delta, point } => self.apply_scroll(delta, point),
            RenderEvent::Shutdown => {}
        }
    }

    /// Handle a non-resize, non-commit event during the resize wait.
    fn handle_non_resize(&mut self, event: RenderEvent) {
        match event {
            RenderEvent::ScaleFactorChanged(sf) => self.scale_factor = sf,
            RenderEvent::Scroll { delta, point } => self.apply_scroll(delta, point),
            _ => {}
        }
    }

    fn commit(&mut self, output: FrameOutput) {
        self.compositor
            .commit(output.display_list, output.layer_tree, output.scroll_tree);
    }

    fn apply_scroll(&mut self, delta: Offset, point: Point) {
        if !self.compositor.has_content() {
            self.queued_scrolls.push((delta, point));
            return;
        }

        // Hit test: find the scrollable container under the cursor.
        // Chrome: InputHandler::HitTestScrollNode() on compositor thread.
        let target = self
            .compositor
            .hit_test_scroll_target(point)
            .or_else(|| self.compositor.scroll_tree().root_scroller());

        if let Some(target) = target {
            if self.compositor.try_scroll(target, delta) {
                let _ = self.view_tx.send(ViewEvent::ScrollSync(
                    self.compositor.scroll_offsets().clone(),
                ));
            }
        }
    }

    fn replay_queued_scrolls(&mut self) {
        for (delta, point) in std::mem::take(&mut self.queued_scrolls) {
            self.apply_scroll(delta, point);
        }
    }
}
