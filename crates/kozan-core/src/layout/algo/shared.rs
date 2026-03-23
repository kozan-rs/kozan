//! Shared layout utilities — direction handling and style conversion.
//!
//! Chrome equivalent: helpers in `ComputedStyleUtils` and `WritingMode`.

use taffy::prelude as tf;

use crate::layout::fragment::ChildFragment;

use style::properties::ComputedValues;
use style::computed_values::direction::T as Direction;

use crate::styling::taffy_bridge::convert;

// ============================================================
// Inline direction (CSS direction + writing-mode)
// ============================================================

/// Inline axis direction derived from CSS `direction` + `writing-mode`.
///
/// Chrome equivalent: `WritingMode` + `TextDirection` combined.
///
/// **Single Responsibility:** ALL direction-dependent layout logic lives here.
/// The layout view, fragment builder, and style conversion all delegate
/// directional decisions to this struct. When we add vertical writing modes,
/// this struct gains an `axis` field and all callers get the new behavior
/// for free.
///
/// Currently: horizontal-tb with ltr/rtl.
pub(crate) struct InlineDirection {
    /// Whether the inline axis is reversed (right-to-left).
    pub reversed: bool,
    // Future: horizontal: bool, vertical_lr: bool
}

impl InlineDirection {
    /// Derive from a computed style.
    pub fn from_style(style: &ComputedValues) -> Self {
        Self {
            reversed: style.clone_direction() == Direction::Rtl,
        }
    }

    // ---- Pre-layout: adjust Taffy styles ----

    /// Adjust a Taffy style for this direction.
    ///
    /// Called once per node during style conversion. Encodes direction
    /// information into Taffy's native capabilities:
    /// - Flex: flips row <-> row-reverse (CSS spec section 9.1)
    /// - Block: sets `text_align` = `LegacyRight` (Taffy right-aligns children)
    pub fn adjust_taffy_style(&self, s: &mut tf::Style<style::Atom>) {
        if !self.reversed {
            return;
        }

        // Flex row: RTL reverses the inline axis.
        s.flex_direction = match s.flex_direction {
            tf::FlexDirection::Row => tf::FlexDirection::RowReverse,
            tf::FlexDirection::RowReverse => tf::FlexDirection::Row,
            other => other, // column axis unaffected
        };

        // Block: inline-start is the right edge in RTL.
        if matches!(s.display, tf::Display::Block) {
            s.text_align = taffy::TextAlign::LegacyRight;
        }
    }

    /// Swap left/right insets and margins for an absolute/fixed child.
    ///
    /// CSS spec: in RTL, the over-constrained resolution rule for absolute
    /// positioning swaps — `right` wins instead of `left`. Taffy always uses
    /// LTR resolution. By swapping insets+margins BEFORE layout, Taffy's
    /// LTR logic produces the correct intermediate result. The post-layout
    /// mirror (`fixup_children`) then flips coordinates to complete the
    /// transformation.
    ///
    /// Only call when the **containing block** (parent) is RTL.
    pub fn swap_absolute_insets(&self, s: &mut tf::Style<style::Atom>) {
        if !self.reversed {
            return;
        }
        std::mem::swap(&mut s.inset.left, &mut s.inset.right);
        std::mem::swap(&mut s.margin.left, &mut s.margin.right);
    }

    // ---- Post-layout: fix child positions ----

    /// Apply direction-dependent post-layout fixup to a container's children.
    ///
    /// Taffy computes all positions in LTR. This method applies the correct
    /// RTL transformation based on the container's display type:
    ///
    /// - **Grid:** mirror ALL children within the content area. Grid tracks
    ///   are direction-independent in Taffy; we flip the entire result.
    /// - **Block/Flex:** mirror only absolute/fixed children within the
    ///   padding box (the containing block for positioned elements).
    ///   In-flow children are already correct via `adjust_taffy_style`.
    ///
    /// `child_positions` are `taffy::Position` values from each child's style.
    #[allow(clippy::too_many_arguments)]
    pub fn fixup_children(
        &self,
        display: tf::Display,
        children: &mut [ChildFragment],
        child_positions: &[tf::Position],
        border_left: f32,
        border_right: f32,
        padding_left: f32,
        padding_right: f32,
        parent_width: f32,
    ) {
        if !self.reversed {
            return;
        }

        match display {
            tf::Display::Grid => {
                // Grid: mirror ALL children within the content area.
                let content_left = border_left + padding_left;
                let content_w = (parent_width - content_left - border_right - padding_right).max(0.0);
                self.mirror_x(children, content_left, content_w);
            }
            _ => {
                // Block/Flex: mirror only positioned children within the padding box.
                let pb_left = border_left;
                let pb_w = (parent_width - border_left - border_right).max(0.0);

                for (child, &pos) in children.iter_mut().zip(child_positions.iter()) {
                    if pos == tf::Position::Absolute {
                        let child_w = child.fragment.size.width;
                        let x_in_pb = child.offset.x - pb_left;
                        child.offset.x = pb_left + pb_w - x_in_pb - child_w;
                    }
                }
            }
        }
    }

    /// Mirror x-positions within a reference box.
    ///
    /// Pure geometry: `new_x = box_left + box_w - (x - box_left) - child_w`.
    fn mirror_x(
        &self,
        children: &mut [ChildFragment],
        box_left: f32,
        box_w: f32,
    ) {
        for child in children.iter_mut() {
            let child_w = child.fragment.size.width;
            let x_in_box = child.offset.x - box_left;
            child.offset.x = box_left + box_w - x_in_box - child_w;
        }
    }
}

// ============================================================
// Taffy item style conversion
// ============================================================

/// Convert a child item's `ComputedValues` to a Taffy `Style`.
///
/// Delegates to `crate::styling::taffy_bridge::convert::to_taffy_style()`
/// which handles the full Stylo -> Taffy conversion, then applies
/// direction-dependent adjustments (RTL).
pub(crate) fn computed_to_taffy_item_style(style: &ComputedValues) -> tf::Style<style::Atom> {
    let mut s = convert::to_taffy_style(style);

    // Apply direction-dependent adjustments (RTL, future: vertical writing modes).
    InlineDirection::from_style(style).adjust_taffy_style(&mut s);

    s
}
