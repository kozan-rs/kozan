//! Inline item — the atomic unit of inline layout.
//!
//! Chrome equivalent: `NGInlineItem`. A flat list of items representing
//! the inline content of a block container.
//!
//! The flat list is a deliberate Chrome optimization — not a tree.
//! It's cache-friendly and fast to iterate during line breaking.

use std::sync::Arc;
use style::properties::ComputedValues;

/// Stylo Arc for computed styles.
type StyleArc<T> = servo_arc::Arc<T>;

/// An item in the inline formatting context.
///
/// Chrome equivalent: `NGInlineItem` with its type.
#[derive(Debug, Clone)]
pub enum InlineItem {
    /// A text run — a segment of text with uniform style.
    /// The text will be shaped (future: via Parley/harfrust).
    Text {
        /// The text content.
        content: Arc<str>,
        /// Style for this text (font, color, etc.).
        style: StyleArc<ComputedValues>,
        /// Measured width (set during shaping/measurement).
        measured_width: f32,
        /// Measured height (ascent + descent).
        measured_height: f32,
        /// Baseline offset from top.
        baseline: f32,
    },

    /// Open tag — start of an inline element (e.g., `<span>`).
    /// Chrome: `kOpenTag`.
    OpenTag {
        style: StyleArc<ComputedValues>,
        /// Inline margin/border/padding on the start side.
        margin_inline_start: f32,
        border_inline_start: f32,
        padding_inline_start: f32,
    },

    /// Close tag — end of an inline element.
    /// Chrome: `kCloseTag`.
    CloseTag {
        /// Inline margin/border/padding on the end side.
        margin_inline_end: f32,
        border_inline_end: f32,
        padding_inline_end: f32,
    },

    /// An atomic inline — an inline-block, replaced element (img), etc.
    /// These are measured as a single unit and cannot be broken across lines.
    AtomicInline {
        /// The fragment produced by laying out this atomic inline.
        width: f32,
        height: f32,
        baseline: f32,
        /// Index into the layout tree.
        layout_id: u32,
        /// Style for this atomic inline (vertical-align, etc.).
        style: StyleArc<ComputedValues>,
    },

    /// A forced line break (`<br>`).
    ForcedBreak,
}

impl InlineItem {
    /// Get the inline size (width) of this item.
    #[must_use] 
    pub fn inline_size(&self) -> f32 {
        match self {
            InlineItem::Text { measured_width, .. } => *measured_width,
            InlineItem::OpenTag {
                margin_inline_start,
                border_inline_start,
                padding_inline_start,
                ..
            } => margin_inline_start + border_inline_start + padding_inline_start,
            InlineItem::CloseTag {
                margin_inline_end,
                border_inline_end,
                padding_inline_end,
                ..
            } => margin_inline_end + border_inline_end + padding_inline_end,
            InlineItem::AtomicInline { width, .. } => *width,
            InlineItem::ForcedBreak => 0.0,
        }
    }

    /// Whether this item forces a line break.
    #[must_use] 
    pub fn is_forced_break(&self) -> bool {
        matches!(self, InlineItem::ForcedBreak)
    }

    /// Whether this item is breakable (text can be split across lines).
    #[must_use] 
    pub fn is_breakable(&self) -> bool {
        matches!(self, InlineItem::Text { .. })
    }
}
