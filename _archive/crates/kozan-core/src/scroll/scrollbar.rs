//! Scrollbar geometry — pure math for thumb positioning.
//!
//! Chrome: `cc/input/scroll_utils.cc` + `ScrollbarLayerImplBase::ComputeThumbQuadRect()`.

use kozan_primitives::geometry::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

/// Chrome: `SolidColorScrollbarLayerImpl` constants.
pub(crate) const THICKNESS: f32 = 8.0;
pub(crate) const MARGIN: f32 = 2.0;
pub(crate) const MIN_THUMB_LENGTH: f32 = 20.0;

/// Chrome: `ScrollUtils::CalculateScrollbarThumbLength()` + `ComputeThumbQuadRect()`.
///
/// `clip_layer_length`: viewport size along the scroll axis.
/// `scroll_layer_length`: content size along the scroll axis.
/// `cross_length`: viewport size perpendicular to the scroll axis.
pub(crate) fn compute_thumb_rect(
    orientation: Orientation,
    current_pos: f32,
    clip_layer_length: f32,
    scroll_layer_length: f32,
    cross_length: f32,
) -> Option<Rect> {
    if clip_layer_length >= scroll_layer_length || scroll_layer_length <= 0.0 {
        return None;
    }

    let maximum = (scroll_layer_length - clip_layer_length).max(0.0);
    let track = clip_layer_length - 2.0 * MARGIN;
    if track <= 0.0 {
        return None;
    }

    let proportion = clip_layer_length / scroll_layer_length;
    let thumb_main = (proportion * track).clamp(MIN_THUMB_LENGTH, track);
    let ratio = if maximum > 0.0 {
        current_pos.clamp(0.0, maximum) / maximum
    } else {
        0.0
    };
    let thumb_offset = MARGIN + ratio * (track - thumb_main);

    Some(match orientation {
        Orientation::Vertical => Rect::new(
            cross_length - THICKNESS - MARGIN,
            thumb_offset,
            THICKNESS,
            thumb_main,
        ),
        Orientation::Horizontal => Rect::new(
            thumb_offset,
            cross_length - THICKNESS - MARGIN,
            thumb_main,
            THICKNESS,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_thumb_at_top() {
        let r = compute_thumb_rect(Orientation::Vertical, 0.0, 400.0, 1200.0, 200.0)
            .expect("overflow");
        assert!(r.y() >= 0.0);
        assert!(r.height() > 0.0);
        assert!(r.height() < 400.0);
        assert!(r.right() <= 200.0, "thumb must be within container width");
    }

    #[test]
    fn vertical_thumb_at_bottom() {
        let r = compute_thumb_rect(Orientation::Vertical, 800.0, 400.0, 1200.0, 200.0)
            .expect("overflow");
        assert!(r.bottom() <= 400.0);
    }

    #[test]
    fn no_thumb_without_overflow() {
        assert!(compute_thumb_rect(Orientation::Vertical, 0.0, 400.0, 400.0, 200.0).is_none());
    }

    #[test]
    fn horizontal_thumb() {
        let r = compute_thumb_rect(Orientation::Horizontal, 0.0, 400.0, 1200.0, 200.0)
            .expect("overflow");
        assert!(r.width() > 0.0);
        assert!(r.bottom() <= 200.0, "thumb must be within container height");
    }

    #[test]
    fn thumb_proportional_to_viewport() {
        let small = compute_thumb_rect(Orientation::Vertical, 0.0, 400.0, 4000.0, 200.0)
            .expect("thumb");
        let large = compute_thumb_rect(Orientation::Vertical, 0.0, 400.0, 800.0, 200.0)
            .expect("thumb");
        assert!(small.height() < large.height());
    }
}
