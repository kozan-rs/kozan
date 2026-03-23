//! `HTMLCanvasElement` — a canvas for 2D/WebGL rendering.
//!
//! Chrome equivalent: `HTMLCanvasElement`.
//! A replaced element — its intrinsic size comes from the `width`/`height`
//! attributes (defaulting to 300x150 per HTML spec).

use super::replaced::{IntrinsicSizing, ReplacedElement};
use crate::Handle;
use kozan_macros::{Element, Props};

/// Default canvas width per HTML spec.
const DEFAULT_CANVAS_WIDTH: f32 = 300.0;
/// Default canvas height per HTML spec.
const DEFAULT_CANVAS_HEIGHT: f32 = 150.0;

/// A canvas element (`<canvas>`).
///
/// Chrome equivalent: `HTMLCanvasElement`.
/// Replaced element — intrinsic size from width/height attributes.
#[derive(Copy, Clone, Element)]
#[element(tag = "canvas", data = CanvasData)]
pub struct HtmlCanvasElement(Handle);

/// Element-specific data for `<canvas>`.
#[derive(Clone, Props)]
#[props(element = HtmlCanvasElement)]
pub struct CanvasData {
    /// The canvas bitmap width in pixels.
    #[prop]
    pub canvas_width: f32,
    /// The canvas bitmap height in pixels.
    #[prop]
    pub canvas_height: f32,
}

impl Default for CanvasData {
    fn default() -> Self {
        Self {
            canvas_width: DEFAULT_CANVAS_WIDTH,
            canvas_height: DEFAULT_CANVAS_HEIGHT,
        }
    }
}

impl ReplacedElement for HtmlCanvasElement {
    fn intrinsic_sizing(&self) -> IntrinsicSizing {
        IntrinsicSizing::from_size(self.canvas_width(), self.canvas_height())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;

    #[test]
    fn canvas_default_size() {
        let doc = Document::new();
        let canvas = doc.create::<HtmlCanvasElement>();

        // HTML spec defaults: 300x150.
        let sizing = canvas.intrinsic_sizing();
        assert_eq!(sizing.width, Some(300.0));
        assert_eq!(sizing.height, Some(150.0));
    }

    #[test]
    fn canvas_custom_size() {
        let doc = Document::new();
        let canvas = doc.create::<HtmlCanvasElement>();

        canvas.set_canvas_width(1024.0);
        canvas.set_canvas_height(768.0);

        let sizing = canvas.intrinsic_sizing();
        assert_eq!(sizing.width, Some(1024.0));
        assert_eq!(sizing.height, Some(768.0));
    }
}
