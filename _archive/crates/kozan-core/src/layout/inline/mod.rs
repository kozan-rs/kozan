//! Inline formatting context — handles text and inline-level elements.
//!
//! Chrome equivalent: `ng/inline/` (`NGInlineNode`, `NGInlineItem`,
//! `NGLineBreaker`, `NGInlineLayoutAlgorithm`).
//!
//! # Architecture
//!
//! ```text
//! 1. CollectInlines: DOM subtree → flat Vec<InlineItem>
//! 2. ShapeText: text items → shaped glyph runs (via Parley, future)
//! 3. LineBreaker: measure items, break into lines
//! 4. BuildLines: position items within line boxes, align baselines
//! 5. Output: Vec<LineFragment> each containing positioned inline fragments
//! ```

pub mod context;
pub mod font_system;
pub mod item;
pub mod line_breaker;
pub mod measurer;

pub use context::InlineFormattingContext;
pub use font_system::{FontQuery, FontSystem};
pub use item::InlineItem;
pub use measurer::{
    FontHeight, FontMetrics, TextMeasurer, TextMetrics, WrappedTextMetrics, resolve_line_height,
};
