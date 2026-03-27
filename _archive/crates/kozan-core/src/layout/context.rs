//! Layout context — shared state for a layout pass.
//!
//! Chrome equivalent: the combination of `LayoutView` (document-level state)
//! and `NGLayoutAlgorithmParams` (per-algorithm context).
//!
//! `LayoutContext` is created once per layout pass and threaded through
//! every algorithm call. It carries dependencies that algorithms need
//! but should NOT hardcode (text measurement, font system, etc.).
//!
//! # Why a struct, not individual parameters?
//!
//! 1. Adding a new dependency (e.g., image loader) doesn't change every
//!    function signature in the call chain.
//! 2. Algorithms can't accidentally create their own measurer —
//!    they MUST use the one provided.
//! 3. Mirrors Chrome: algorithms receive context, not individual services.

use super::inline::measurer::TextMeasurer;

/// Shared context for a layout pass.
///
/// Created once by the caller (platform/view layer) and passed through
/// the entire layout tree. Algorithms read from it, never construct
/// their own dependencies.
///
/// Chrome equivalent: combination of `Document::GetLayoutView()` context
/// and the font/shaping system accessible through `ComputedStyle::GetFont()`.
pub struct LayoutContext<'a> {
    /// The text measurer for this layout pass.
    ///
    /// Provides text width measurement and font metrics.
    /// Chrome: accessed via `Font` → `CachingWordShaper` → `HarfBuzzShaper`.
    ///
    /// The platform layer sets this to the appropriate implementation:
    /// - `DefaultTextMeasurer` for estimation (no font system yet)
    /// - Parley-based measurer when integrated
    /// - Custom measurer for testing
    pub text_measurer: &'a dyn TextMeasurer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::inline::measurer::FontMetrics;
    use crate::layout::inline::{FontSystem, TextMetrics};

    #[test]
    fn context_carries_measurer() {
        let measurer = FontSystem::new();
        let ctx = LayoutContext {
            text_measurer: &measurer,
        };

        // Algorithms use ctx.text_measurer, not create their own.
        let metrics = ctx.text_measurer.measure("Hello", 16.0);
        assert!(metrics.width > 0.0);
    }

    #[test]
    fn custom_measurer_via_context() {
        struct TestMeasurer;
        impl TextMeasurer for TestMeasurer {
            fn measure(&self, _text: &str, _font_size: f32) -> TextMetrics {
                TextMetrics { width: 42.0 }
            }
            fn font_metrics(&self, _font_size: f32) -> FontMetrics {
                FontMetrics {
                    ascent: 12.0,
                    descent: 4.0,
                    line_gap: 0.0,
                }
            }
        }

        let measurer = TestMeasurer;
        let ctx = LayoutContext {
            text_measurer: &measurer,
        };
        assert_eq!(ctx.text_measurer.measure("x", 16.0).width, 42.0);
    }
}
