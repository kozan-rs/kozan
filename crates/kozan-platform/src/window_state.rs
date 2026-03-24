//! Per-window state — pipeline + input tracking.
//!
//! Chrome: `RenderWidgetHostImpl` — per-widget state on the browser main thread.

use crate::event::ViewEvent;
use crate::pipeline::WindowPipeline;
use crate::pipeline::input_state::InputState;
use crate::pipeline::render_loop::RenderEvent;

/// Per-window state managed by WindowManager.
pub(crate) struct WindowState {
    pipeline: WindowPipeline,
    input: InputState,
}

impl WindowState {
    pub fn new(pipeline: WindowPipeline, scale_factor: f64) -> Self {
        Self {
            pipeline,
            input: InputState::new(scale_factor),
        }
    }

    pub fn input(&self) -> &InputState {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut InputState {
        &mut self.input
    }

    pub fn send_to_view(&self, event: ViewEvent) -> bool {
        self.pipeline.send_to_view(event)
    }

    pub fn send_to_render(&self, event: RenderEvent) -> bool {
        self.pipeline.send_to_render(event)
    }

    pub fn shutdown(&mut self) {
        self.pipeline.shutdown();
    }
}
