//! Fragment — the immutable output of layout.
//!
//! Chrome equivalent: `NGPhysicalFragment` + `NGPhysicalBoxFragment` +
//! `NGPhysicalTextFragment`.
//!
//! # Architecture
//!
//! Fragments are the ONLY output of layout. They flow UP the tree
//! (children produce fragments, parents position them). Once created,
//! fragments are **immutable** — wrapped in `Arc` for safe sharing
//! between layout, paint, hit-testing, and the compositor.
//!
//! # Why immutable?
//!
//! Chrome's `LayoutNG` learned from legacy's bugs:
//! - **No hysteresis**: same inputs = same outputs, always.
//! - **Safe concurrent access**: paint reads while layout runs on next frame.
//! - **Fragment caching**: if `ConstraintSpace` matches, reuse the fragment.
//! - **No stale reads**: impossible to read half-updated data.
//!
//! # Fragment types
//!
//! ```text
//! Fragment
//! ├── BoxFragment    — block, flex, grid, inline-block containers
//! ├── TextFragment   — shaped text content (glyph runs)
//! └── LineFragment   — a line box in inline formatting context
//! ```

use std::sync::Arc;

use kozan_primitives::geometry::{Point, Size};
use style::properties::ComputedValues;

/// A positioned child within a parent fragment.
///
/// Chrome equivalent: `NGLink` — stores the offset separately from
/// the fragment, so the same fragment can appear at different positions
/// (e.g., in different columns) without cloning.
#[derive(Debug, Clone)]
pub struct ChildFragment {
    /// Offset relative to the parent fragment's top-left corner.
    pub offset: Point,
    /// The child's fragment (shared, immutable).
    pub fragment: Arc<Fragment>,
}

/// The immutable output of a layout algorithm.
///
/// Chrome equivalent: `NGPhysicalFragment`. Created by layout algorithms,
/// never modified afterwards. Wrapped in `Arc` for zero-cost sharing.
///
/// # Coordinate system
///
/// All coordinates are **physical** (not logical). Writing-mode conversion
/// happens at the algorithm level, not in the fragment.
/// Chrome uses physical fragments too (`NGPhysicalFragment`, not `NGLogicalFragment`).
#[derive(Debug, Clone)]
pub struct Fragment {
    /// The border-box size of this fragment.
    pub size: Size,
    /// What kind of fragment this is.
    pub kind: FragmentKind,
    /// The computed style for this fragment.
    /// Chrome: `NGPhysicalFragment::Style()`.
    /// Used by the paint phase to determine background, border, text color, etc.
    /// `None` for fragments not yet connected to paint (e.g., anonymous, line boxes).
    pub style: Option<servo_arc::Arc<ComputedValues>>,
    /// The DOM node index this fragment was generated from.
    /// `None` for anonymous boxes and line fragments.
    /// Used by the paint phase to look up additional data (text content, etc.).
    pub dom_node: Option<u32>,
}

/// The specific type of a fragment.
///
/// Chrome equivalent: `NGPhysicalFragment::Type` + subclass data.
#[derive(Debug, Clone)]
pub enum FragmentKind {
    /// A box fragment (block, flex, grid, inline-block container).
    /// Chrome: `NGPhysicalBoxFragment`.
    Box(BoxFragmentData),

    /// A text fragment (shaped glyph run).
    /// Chrome: `NGPhysicalTextFragment`.
    Text(TextFragmentData),

    /// A line box in an inline formatting context.
    /// Chrome: `NGPhysicalLineBoxFragment`.
    Line(LineFragmentData),
}

/// Content for replaced elements (canvas, image, video, custom).
///
/// Chrome: `LayoutReplaced::PaintReplaced()` — each replaced element type
/// overrides how it paints. A trait allows full customization: canvas, image,
/// video, 3D viewports, or any user-defined replaced content — without
/// modifying core types.
///
/// Stored as `Arc<dyn ReplacedContent>` on the fragment for zero-cost
/// sharing between layout, paint, and compositor threads.
pub trait ReplacedContent: Send + Sync + std::fmt::Debug {
    /// Produce a draw command for this replaced content.
    ///
    /// Chrome: `PaintReplaced(PaintInfo&)` on each replaced element type.
    /// The painter calls this and emits the result into the display list.
    fn to_draw_command(&self) -> crate::paint::display_item::DrawCommand;
}

/// Data specific to box fragments (containers).
///
/// Chrome equivalent: `NGPhysicalBoxFragment` fields.
#[derive(Debug, Clone, Default)]
pub struct BoxFragmentData {
    /// Positioned children within this box.
    pub children: Vec<ChildFragment>,
    /// Padding box insets (for hit-testing and paint).
    pub padding: PhysicalInsets,
    /// Border box insets (for border painting).
    pub border: PhysicalInsets,
    /// Content overflow extent (for scrolling).
    /// If larger than the box's size, there's scrollable overflow.
    pub scrollable_overflow: Size,
    /// Whether this box establishes a new stacking context.
    pub is_stacking_context: bool,
    /// Overflow behavior on inline axis.
    pub overflow_x: OverflowClip,
    /// Overflow behavior on block axis.
    pub overflow_y: OverflowClip,
    /// Content for replaced elements (canvas, image, video, custom).
    /// `None` for regular container boxes.
    pub replaced_content: Option<std::sync::Arc<dyn ReplacedContent>>,
    /// Overscroll behavior on inline axis (CSS `overscroll-behavior-x`).
    pub overscroll_x: OverscrollBehavior,
    /// Overscroll behavior on block axis (CSS `overscroll-behavior-y`).
    pub overscroll_y: OverscrollBehavior,
}

/// CSS `overscroll-behavior` — controls scroll chaining at boundaries.
///
/// Chrome: `cc::OverscrollBehavior` on scroll nodes.
/// Spec: <https://drafts.csswg.org/css-overscroll-1/#overscroll-behavior-properties>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverscrollBehavior {
    /// Scroll chaining proceeds normally (default).
    #[default]
    Auto,
    /// Do NOT chain scroll to ancestors. Local overscroll effects still apply.
    Contain,
    /// Do NOT chain AND no local overscroll effects.
    None,
}

/// How overflow content is handled for a box fragment.
///
/// Chrome equivalent: part of `NGPhysicalBoxFragment` overflow handling.
/// This is the RESOLVED overflow — after layout determines if there IS overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowClip {
    /// Content overflows visibly (default).
    #[default]
    Visible,
    /// Content is clipped to the box. No scroll mechanism.
    Hidden,
    /// Content is clipped. Scroll mechanism provided if overflow exists.
    Scroll,
    /// Like scroll, but scroll mechanism only shown when needed.
    Auto,
}

impl OverflowClip {
    /// Whether this mode clips content (for paint + hit-test).
    #[must_use] 
    pub fn clips(self) -> bool {
        matches!(self, Self::Hidden | Self::Scroll | Self::Auto)
    }

    /// Whether the user can scroll this axis (wheel/touch).
    /// `Hidden` clips but does NOT respond to user input.
    #[must_use] 
    pub fn is_user_scrollable(self) -> bool {
        matches!(self, Self::Scroll | Self::Auto)
    }
}

/// Data specific to text fragments (glyph runs).
///
/// Chrome equivalent: `NGPhysicalTextFragment` fields.
/// The actual glyphs and positions will come from Parley's shaping output.
#[derive(Debug, Clone)]
pub struct TextFragmentData {
    /// The text content this fragment represents.
    pub text_range: std::ops::Range<usize>,
    /// Baseline offset from the fragment's top edge.
    pub baseline: f32,
    /// The raw text content.
    pub text: Option<Arc<str>>,
    /// Pre-shaped glyph runs from Parley (`HarfRust`).
    /// Chrome: `ShapeResult` on `NGPhysicalTextFragment`.
    /// Shaped ONCE during layout, read by paint + renderer.
    /// Font data is `parley::FontData` = `peniko::Font` — zero conversion to vello.
    pub shaped_runs: Vec<crate::layout::inline::font_system::ShapedTextRun>,
}

/// Data specific to line box fragments.
///
/// Chrome equivalent: `NGPhysicalLineBoxFragment`.
/// A line box contains inline-level children (text, inline boxes).
#[derive(Debug, Clone)]
pub struct LineFragmentData {
    /// Inline-level children positioned within this line.
    pub children: Vec<ChildFragment>,
    /// The baseline of this line (from top of line box).
    pub baseline: f32,
}

/// Physical edge insets (top, right, bottom, left).
///
/// Used for padding and border widths on box fragments.
/// "Physical" means not affected by writing-mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct PhysicalInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl PhysicalInsets {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    #[must_use]
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Total inline (horizontal) insets.
    #[inline]
    #[must_use]
    pub fn inline_sum(&self) -> f32 {
        self.left + self.right
    }

    /// Total block (vertical) insets.
    #[inline]
    #[must_use]
    pub fn block_sum(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Fragment {
    /// Create a box fragment.
    #[must_use]
    pub fn new_box(size: Size, data: BoxFragmentData) -> Arc<Self> {
        Arc::new(Self {
            size,
            kind: FragmentKind::Box(data),
            style: None,
            dom_node: None,
        })
    }

    /// Create a box fragment with computed style and DOM node reference.
    ///
    /// Chrome: `NGPhysicalFragment` always has a style + layout object pointer.
    /// The style is needed by the paint phase for background, border, text color.
    /// The `dom_node` is needed for looking up text content and element data.
    #[must_use]
    pub fn new_box_styled(
        size: Size,
        data: BoxFragmentData,
        style: servo_arc::Arc<ComputedValues>,
        dom_node: Option<u32>,
    ) -> Arc<Self> {
        Arc::new(Self {
            size,
            kind: FragmentKind::Box(data),
            style: Some(style),
            dom_node,
        })
    }

    /// Create a text fragment.
    #[must_use]
    pub fn new_text(size: Size, data: TextFragmentData) -> Arc<Self> {
        Arc::new(Self {
            size,
            kind: FragmentKind::Text(data),
            style: None,
            dom_node: None,
        })
    }

    /// Create a text fragment with style (for font-size, color inheritance).
    #[must_use]
    pub fn new_text_styled(
        size: Size,
        data: TextFragmentData,
        style: servo_arc::Arc<style::properties::ComputedValues>,
        dom_node: Option<u32>,
    ) -> Arc<Self> {
        Arc::new(Self {
            size,
            kind: FragmentKind::Text(data),
            style: Some(style),
            dom_node,
        })
    }

    /// Create a line fragment.
    #[must_use]
    pub fn new_line(size: Size, data: LineFragmentData) -> Arc<Self> {
        Arc::new(Self {
            size,
            kind: FragmentKind::Line(data),
            style: None,
            dom_node: None,
        })
    }

    /// Whether this is a box fragment.
    #[must_use]
    pub fn is_box(&self) -> bool {
        matches!(self.kind, FragmentKind::Box(_))
    }

    /// Whether this is a text fragment.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self.kind, FragmentKind::Text(_))
    }

    /// Whether this is a line fragment.
    #[must_use]
    pub fn is_line(&self) -> bool {
        matches!(self.kind, FragmentKind::Line(_))
    }

    /// Panics if this is not a box fragment. Use `try_as_box()` when unsure.
    #[must_use]
    pub fn unwrap_box(&self) -> &BoxFragmentData {
        match &self.kind {
            FragmentKind::Box(data) => data,
            _ => panic!("Fragment is not a box"),
        }
    }

    /// Get line fragment data (panics if not a line).
    #[must_use]
    pub fn as_line(&self) -> &LineFragmentData {
        match &self.kind {
            FragmentKind::Line(data) => data,
            _ => panic!("Fragment is not a line"),
        }
    }

    /// Get box fragment data if this is a box.
    #[must_use]
    pub fn try_as_box(&self) -> Option<&BoxFragmentData> {
        match &self.kind {
            FragmentKind::Box(data) => Some(data),
            _ => None,
        }
    }

    /// Get text fragment data if this is text.
    #[must_use]
    pub fn try_as_text(&self) -> Option<&TextFragmentData> {
        match &self.kind {
            FragmentKind::Text(data) => Some(data),
            _ => None,
        }
    }

    /// CSS Overflow Module Level 3 §2.1 — this fragment's overflow extent.
    ///
    /// Per axis: if this fragment clips, returns its border box size.
    /// Otherwise, returns the max of its size and its scrollable overflow
    /// (which already includes descendants recursively).
    #[must_use]
    pub fn overflow_extent(&self) -> (f32, f32) {
        let Some(data) = self.try_as_box() else {
            return (self.size.width, self.size.height);
        };
        let w = if data.overflow_x.clips() {
            self.size.width
        } else {
            data.scrollable_overflow.width.max(self.size.width)
        };
        let h = if data.overflow_y.clips() {
            self.size.height
        } else {
            data.scrollable_overflow.height.max(self.size.height)
        };
        (w, h)
    }

    /// Get line fragment data if this is a line.
    #[must_use]
    pub fn try_as_line(&self) -> Option<&LineFragmentData> {
        match &self.kind {
            FragmentKind::Line(data) => Some(data),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_fragment_immutable_via_arc() {
        let fragment = Fragment::new_box(Size::new(100.0, 50.0), BoxFragmentData::default());
        // Arc means shared + immutable.
        let shared = Arc::clone(&fragment);
        assert_eq!(fragment.size.width, 100.0);
        assert_eq!(shared.size.height, 50.0);
        assert!(fragment.is_box());
    }

    #[test]
    fn text_fragment() {
        let fragment = Fragment::new_text(
            Size::new(80.0, 16.0),
            TextFragmentData {
                text_range: 0..5,
                baseline: 12.0,
                text: Some(Arc::from("Hello")),
                shaped_runs: Vec::new(),
            },
        );
        assert!(fragment.is_text());
        assert_eq!(fragment.try_as_text().unwrap().baseline, 12.0);
    }

    #[test]
    fn line_fragment_with_children() {
        let text = Fragment::new_text(
            Size::new(40.0, 16.0),
            TextFragmentData {
                text_range: 0..3,
                baseline: 12.0,
                text: Some(Arc::from("abc")),
                shaped_runs: Vec::new(),
            },
        );

        let line = Fragment::new_line(
            Size::new(200.0, 20.0),
            LineFragmentData {
                children: vec![ChildFragment {
                    offset: Point::new(0.0, 2.0),
                    fragment: text,
                }],
                baseline: 16.0,
            },
        );
        assert!(line.is_line());
        assert_eq!(line.try_as_line().unwrap().children.len(), 1);
    }

    #[test]
    fn nested_box_fragments() {
        let inner = Fragment::new_box(
            Size::new(50.0, 30.0),
            BoxFragmentData {
                padding: PhysicalInsets::new(5.0, 5.0, 5.0, 5.0),
                border: PhysicalInsets::new(1.0, 1.0, 1.0, 1.0),
                ..Default::default()
            },
        );

        let outer = Fragment::new_box(
            Size::new(200.0, 100.0),
            BoxFragmentData {
                children: vec![ChildFragment {
                    offset: Point::new(10.0, 10.0),
                    fragment: inner,
                }],
                ..Default::default()
            },
        );

        let children = &outer.unwrap_box().children;
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].offset.x, 10.0);
        assert_eq!(children[0].fragment.size.width, 50.0);
    }

    #[test]
    fn physical_insets() {
        let insets = PhysicalInsets::new(10.0, 20.0, 10.0, 20.0);
        assert_eq!(insets.inline_sum(), 40.0);
        assert_eq!(insets.block_sum(), 20.0);
    }

    #[test]
    fn fragment_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arc<Fragment>>();
    }
}
