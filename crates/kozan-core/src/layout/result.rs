//! Layout result — the output of a layout algorithm.
//!
//! Chrome equivalent: `NGLayoutResult`. Wraps the fragment plus
//! additional information needed by the parent (out-of-flow descendants,
//! break tokens for fragmentation, intrinsic sizes).
//!
//! The parent layout algorithm calls `child.layout(space)` and gets
//! back a `LayoutResult`. It reads the fragment to know the child's size,
//! then positions it.

use std::sync::Arc;

use super::fragment::Fragment;

/// The result of running a layout algorithm on a node.
///
/// Chrome equivalent: `NGLayoutResult`.
///
/// Contains the fragment (immutable output) plus metadata the parent needs.
/// Caching is handled externally (by `LayoutObject` or Taffy's `Cache`).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LayoutResult {
    /// The computed fragment for this node.
    pub fragment: Arc<Fragment>,

    /// Intrinsic inline sizes (min-content and max-content).
    /// Cached here because flex/grid algorithms query them.
    /// None = not yet computed.
    pub intrinsic_sizes: Option<IntrinsicSizes>,

    /// Margins that escaped from first/last child through this block.
    /// Used by the parent for proper parent-child margin collapsing.
    /// Zero for blocks that establish a BFC or have border/padding.
    pub escaped_margins: EscapedMargins,
}

/// Intrinsic (preferred) sizes for a node.
///
/// Chrome equivalent: `MinMaxSizes`.
/// Used by flex, grid, and shrink-to-fit layout to determine
/// how wide a node "wants" to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntrinsicSizes {
    /// Minimum content size (narrowest the content can be without overflow).
    /// The width of the longest unbreakable word or widest atomic inline.
    pub min_content: f32,

    /// Maximum content size (widest the content would be with infinite space).
    /// All content on one line.
    pub max_content: f32,
}

/// Margins that escaped through a block due to CSS parent-child margin collapsing.
///
/// CSS rule: if a block has no border-top/padding-top and doesn't establish a BFC,
/// the first child's top margin "escapes" and becomes part of the parent's top margin.
/// Same for the bottom edge. The escaped margin is `max(parent_margin, child_margin)`.
///
/// Chrome equivalent: part of `NGMarginStrut` propagation in `NGBlockLayoutAlgorithm`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EscapedMargins {
    /// First child's margin that escaped through the top of this block.
    pub top: f32,
    /// Last child's margin that escaped through the bottom of this block.
    pub bottom: f32,
}

impl IntrinsicSizes {
    /// Clamp a given available size to the intrinsic range.
    /// Used in shrink-to-fit calculations.
    ///
    /// CSS: `width = min(max_content, max(min_content, available))`.
    #[inline]
    #[must_use] 
    pub fn shrink_to_fit(&self, available: f32) -> f32 {
        self.max_content.min(self.min_content.max(available))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::{BoxFragmentData, Fragment};
    use kozan_primitives::geometry::Size;

    #[test]
    fn layout_result_construction() {
        let fragment = Fragment::new_box(
            Size::new(800.0, 200.0),
            BoxFragmentData::default(),
        );

        let result = LayoutResult {
            fragment,
            intrinsic_sizes: None,
            escaped_margins: EscapedMargins::default(),
        };

        assert!(result.fragment.is_box());
    }

    #[test]
    fn intrinsic_sizes_shrink_to_fit() {
        let sizes = IntrinsicSizes {
            min_content: 100.0,
            max_content: 500.0,
        };

        // Available > max_content → use max_content.
        assert_eq!(sizes.shrink_to_fit(1000.0), 500.0);

        // Available between min and max → use available.
        assert_eq!(sizes.shrink_to_fit(300.0), 300.0);

        // Available < min_content → use min_content.
        assert_eq!(sizes.shrink_to_fit(50.0), 100.0);
    }
}
