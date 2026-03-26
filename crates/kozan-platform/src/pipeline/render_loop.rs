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

pub enum RenderEvent {
    Commit(FrameOutput),
    Resize { width: u32, height: u32 },
    ScaleFactorChanged(f64),
    Scroll { delta: Offset, point: Point },
    /// Keyboard-driven scroll — target is already resolved by the view thread.
    ScrollTo { target: u32, delta: Offset },
    MouseDown { point: Point },
    MouseUp { point: Point },
    MouseMove { point: Point },
    Shutdown,
}

pub type OnSurfaceLost = Box<dyn FnOnce() + Send>;

pub(crate) struct RenderLoop<S> {
    surface: S,
    compositor: Compositor,
    view_tx: mpsc::Sender<ViewEvent>,
    on_surface_lost: Option<OnSurfaceLost>,
    width: u32,
    height: u32,
    /// DPI scale from the OS display. Updated by ScaleFactorChanged.
    device_scale_factor: f64,
    /// Page zoom from Ctrl+/-. Updated on each commit from FrameOutput.
    page_zoom_factor: f64,
    queued_scrolls: Vec<(Offset, Point)>,
    awaiting_resize_commit: bool,
}

impl<S: RenderSurface> RenderLoop<S> {
    pub fn new(
        surface: S,
        view_tx: mpsc::Sender<ViewEvent>,
        on_surface_lost: OnSurfaceLost,
        width: u32,
        height: u32,
        device_scale_factor: f64,
    ) -> Self {
        Self {
            surface,
            compositor: Compositor::new(),
            view_tx,
            on_surface_lost: Some(on_surface_lost),
            width,
            height,
            device_scale_factor,
            page_zoom_factor: 1.0,
            queued_scrolls: Vec::new(),
            awaiting_resize_commit: false,
        }
    }

    /// CSS pixels × content_scale = physical GPU pixels.
    /// Combines device DPI and page zoom. Pinch zoom is a separate
    /// compositor transform (future).
    fn content_scale(&self) -> f64 {
        self.device_scale_factor * self.page_zoom_factor
    }

    pub fn run(&mut self, rx: mpsc::Receiver<RenderEvent>) {
        if !self.wait_for_first_commit(&rx) {
            return;
        }
        loop {
            if !self.drain_events(&rx) {
                return;
            }
            if self.awaiting_resize_commit {
                if !self.wait_for_resize_commit(&rx) {
                    return;
                }
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
        if self.compositor.needs_animate() {
            // Chrome: `SetNeedsAnimateForScrollbarAnimation()` — keep rendering
            // during fade animation instead of blocking on the channel.
            loop {
                match rx.try_recv() {
                    Ok(RenderEvent::Shutdown) => return false,
                    Ok(event) => self.handle_event(event),
                    Err(mpsc::TryRecvError::Empty) => return true,
                    Err(mpsc::TryRecvError::Disconnected) => return false,
                }
            }
        } else {
            match rx.recv() {
                Ok(RenderEvent::Shutdown) | Err(_) => return false,
                Ok(event) => {
                    self.handle_event(event);
                    loop {
                        match rx.try_recv() {
                            Ok(RenderEvent::Shutdown) => return false,
                            Ok(event) => self.handle_event(event),
                            Err(mpsc::TryRecvError::Empty) => return true,
                            Err(mpsc::TryRecvError::Disconnected) => return false,
                        }
                    }
                }
            }
        }
    }

    fn render_frame(&mut self) -> bool {
        if let Some(frame) = self.compositor.produce_frame() {
            let params = RenderParams {
                frame: &frame,
                width: self.width,
                height: self.height,
                content_scale: self.content_scale(),
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
        true
    }

    fn wait_for_resize_commit(&mut self, rx: &mpsc::Receiver<RenderEvent>) -> bool {
        loop {
            match rx.recv() {
                Ok(RenderEvent::Commit(output)) => {
                    if output.viewport_width == self.width
                        && output.viewport_height == self.height
                    {
                        self.commit(output);
                        self.awaiting_resize_commit = false;
                        return self.render_frame();
                    }
                }
                Ok(RenderEvent::Resize { width, height }) => {
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
            RenderEvent::ScaleFactorChanged(sf) => self.device_scale_factor = sf,
            RenderEvent::Scroll { delta, point } => self.apply_scroll(delta, point),
            RenderEvent::ScrollTo { target, delta } => self.apply_scroll_to(target, delta),
            RenderEvent::MouseDown { point } => self.handle_mouse_down(point),
            RenderEvent::MouseUp { point } => self.handle_mouse_up(point),
            RenderEvent::MouseMove { point } => self.handle_mouse_move(point),
            RenderEvent::Shutdown => {}
        }
    }

    fn handle_non_resize(&mut self, event: RenderEvent) {
        match event {
            RenderEvent::ScaleFactorChanged(sf) => self.device_scale_factor = sf,
            RenderEvent::Scroll { delta, point } => self.apply_scroll(delta, point),
            RenderEvent::ScrollTo { target, delta } => self.apply_scroll_to(target, delta),
            RenderEvent::MouseDown { point } => self.handle_mouse_down(point),
            RenderEvent::MouseUp { point } => self.handle_mouse_up(point),
            RenderEvent::MouseMove { point } => self.handle_mouse_move(point),
            _ => {}
        }
    }

    fn commit(&mut self, output: FrameOutput) {
        self.page_zoom_factor = output.page_zoom_factor;
        self.compositor
            .commit(output.display_list, output.layer_tree, output.scroll_tree);
    }

    fn apply_scroll(&mut self, delta: Offset, point: Point) {
        if !self.compositor.has_content() {
            self.queued_scrolls.push((delta, point));
            return;
        }

        // Screen-logical coords → content-logical coords.
        // Page zoom shrinks the layout viewport, so a screen pixel covers
        // more content pixels. Hit-test and delta must be in content space.
        let z = self.page_zoom_factor as f32;
        let content_point = Point::new(point.x / z, point.y / z);
        let content_delta = Offset::new(delta.dx / z, delta.dy / z);

        let target = self.compositor.hit_test_scroll_target(content_point);

        if let Some(target) = target {
            if self.compositor.try_scroll(target, content_delta) {
                let _ = self.view_tx.send(ViewEvent::ScrollSync(
                    self.compositor.scroll_offsets().clone(),
                ));
            }
        }
    }

    fn apply_scroll_to(&mut self, target: u32, delta: Offset) {
        if !self.compositor.has_content() {
            return;
        }
        if self.compositor.try_scroll(target, delta) {
            let _ = self.view_tx.send(ViewEvent::ScrollSync(
                self.compositor.scroll_offsets().clone(),
            ));
        }
    }

    fn handle_mouse_down(&mut self, point: Point) {
        if !self.compositor.has_content() {
            return;
        }
        let z = self.page_zoom_factor as f32;
        let content_point = Point::new(point.x / z, point.y / z);
        if self.compositor.handle_mouse_down(content_point) {
            let _ = self.view_tx.send(ViewEvent::ScrollSync(
                self.compositor.scroll_offsets().clone(),
            ));
        }
    }

    fn handle_mouse_move(&mut self, point: Point) {
        if !self.compositor.has_content() {
            return;
        }
        let z = self.page_zoom_factor as f32;
        let content_point = Point::new(point.x / z, point.y / z);
        if self.compositor.handle_mouse_move(content_point) {
            let _ = self.view_tx.send(ViewEvent::ScrollSync(
                self.compositor.scroll_offsets().clone(),
            ));
        }
    }

    fn handle_mouse_up(&mut self, _point: Point) {
        self.compositor.handle_mouse_up();
    }

    fn replay_queued_scrolls(&mut self) {
        for (delta, point) in std::mem::take(&mut self.queued_scrolls) {
            self.apply_scroll(delta, point);
        }
    }
}
