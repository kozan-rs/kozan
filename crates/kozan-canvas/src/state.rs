//! Canvas state stack for save()/restore().
//!
//! Chrome equivalent: `CanvasRenderingContext2DState` + the `state_stack_`
//! vector on `Canvas2DRecorderContext`.

use kozan_primitives::color::Color;
use kozan_primitives::transform::AffineTransform;

use crate::blend::BlendMode;
use crate::line::{LineCap, LineJoin};
use crate::shadow::ShadowState;
use crate::style::PaintStyle;
use crate::text::{TextAlign, TextBaseline, TextDirection};

/// Canvas drawing state — saved/restored by save()/restore().
///
/// Chrome equivalent: `CanvasRenderingContext2DState`.
/// Contains ALL style properties. The transform is kept here for
/// `getTransform()`/`setTransform()` but is also recorded as ops for replay.
#[derive(Clone, Debug)]
pub struct CanvasState {
    pub fill_style: PaintStyle,
    pub stroke_style: PaintStyle,
    pub line_width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub line_dash: Vec<f32>,
    pub line_dash_offset: f32,
    pub global_alpha: f32,
    pub global_composite_operation: BlendMode,
    pub shadow: ShadowState,
    pub image_smoothing_enabled: bool,
    pub font: String,
    pub text_align: TextAlign,
    pub text_baseline: TextBaseline,
    pub direction: TextDirection,
    pub transform: AffineTransform,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            fill_style: PaintStyle::Color(Color::BLACK),
            stroke_style: PaintStyle::Color(Color::BLACK),
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            line_dash: Vec::new(),
            line_dash_offset: 0.0,
            global_alpha: 1.0,
            global_composite_operation: BlendMode::SourceOver,
            shadow: ShadowState::default(),
            image_smoothing_enabled: true,
            font: "10px sans-serif".to_string(),
            text_align: TextAlign::Start,
            text_baseline: TextBaseline::Alphabetic,
            direction: TextDirection::Ltr,
            transform: AffineTransform::IDENTITY,
        }
    }
}

/// The state stack — `save()` pushes a clone, `restore()` pops it.
///
/// Chrome equivalent: `state_stack_: HeapVector<Member<CanvasRenderingContext2DState>>`
/// on `Canvas2DRecorderContext`.
#[derive(Debug, Clone)]
pub struct CanvasStateStack {
    stack: Vec<CanvasState>,
    current: CanvasState,
}

impl CanvasStateStack {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            current: CanvasState::default(),
        }
    }

    #[must_use]
    pub fn current(&self) -> &CanvasState {
        &self.current
    }

    pub fn current_mut(&mut self) -> &mut CanvasState {
        &mut self.current
    }

    pub fn save(&mut self) {
        self.stack.push(self.current.clone());
    }

    /// Restore the previous state. No-op if the stack is empty (per spec).
    pub fn restore(&mut self) -> bool {
        if let Some(state) = self.stack.pop() {
            self.current = state;
            true
        } else {
            false
        }
    }

    /// Reset to the initial default state and clear the stack.
    pub fn reset(&mut self) {
        self.stack.clear();
        self.current = CanvasState::default();
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Default for CanvasStateStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_restore_preserves_state() {
        let mut stack = CanvasStateStack::new();
        stack.current_mut().line_width = 5.0;
        stack.save();
        stack.current_mut().line_width = 10.0;
        assert_eq!(stack.current().line_width, 10.0);
        stack.restore();
        assert_eq!(stack.current().line_width, 5.0);
    }

    #[test]
    fn restore_on_empty_is_noop() {
        let mut stack = CanvasStateStack::new();
        let original_width = stack.current().line_width;
        assert!(!stack.restore());
        assert_eq!(stack.current().line_width, original_width);
    }

    #[test]
    fn reset_clears_everything() {
        let mut stack = CanvasStateStack::new();
        stack.current_mut().global_alpha = 0.5;
        stack.save();
        stack.current_mut().global_alpha = 0.1;
        stack.reset();
        assert_eq!(stack.current().global_alpha, 1.0);
        assert_eq!(stack.depth(), 0);
    }

    #[test]
    fn nested_save_restore() {
        let mut stack = CanvasStateStack::new();
        stack.current_mut().line_width = 1.0;
        stack.save();
        stack.current_mut().line_width = 2.0;
        stack.save();
        stack.current_mut().line_width = 3.0;
        stack.save();
        stack.current_mut().line_width = 4.0;

        assert_eq!(stack.depth(), 3);
        assert_eq!(stack.current().line_width, 4.0);
        stack.restore();
        assert_eq!(stack.current().line_width, 3.0);
        stack.restore();
        assert_eq!(stack.current().line_width, 2.0);
        stack.restore();
        assert_eq!(stack.current().line_width, 1.0);
        assert!(!stack.restore());
    }
}
