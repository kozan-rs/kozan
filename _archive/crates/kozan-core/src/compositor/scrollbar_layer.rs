//! Solid color overlay scrollbar layer.
//!
//! Chrome: `cc::SolidColorScrollbarLayerImpl` + `ScrollbarAnimationController`.

use std::any::Any;

use smallvec::{SmallVec, smallvec};

use kozan_primitives::geometry::{Point, Rect};

use crate::scroll::scrollbar::{self, MARGIN, Orientation, THICKNESS};

use super::frame::FrameQuad;
use super::layer::{LayerContent, QuadContext};
use super::scrollbar_theme::ScrollbarTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollbarPart {
    Thumb,
    BackTrack,
    ForwardTrack,
    NoPart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollbarState {
    Idle,
    Hovered,
    Active,
}

/// Chrome: `cc::SolidColorScrollbarLayerImpl`.
#[derive(Clone)]
pub(crate) struct ScrollbarLayer {
    pub scroll_element_id: u32,
    pub orientation: Orientation,
    pub current_pos: f32,
    pub clip_layer_length: f32,
    pub scroll_layer_length: f32,
    pub cross_length: f32,
    pub state: ScrollbarState,
    /// Chrome: `ScrollbarAnimationController::opacity_`.
    pub opacity: f32,
}

impl ScrollbarLayer {
    pub(crate) fn new(scroll_element_id: u32, orientation: Orientation) -> Self {
        Self {
            scroll_element_id,
            orientation,
            current_pos: 0.0,
            clip_layer_length: 0.0,
            scroll_layer_length: 0.0,
            cross_length: 0.0,
            state: ScrollbarState::Idle,
            opacity: 0.0,
        }
    }

    pub(crate) fn snapshot(&self) -> Self {
        self.clone()
    }

    pub(crate) fn update_geometry(
        &mut self,
        current_pos: f32,
        clip_layer_length: f32,
        scroll_layer_length: f32,
        cross_length: f32,
    ) {
        self.current_pos = current_pos;
        self.clip_layer_length = clip_layer_length;
        self.scroll_layer_length = scroll_layer_length;
        self.cross_length = cross_length;
    }

    pub(crate) fn can_scroll(&self) -> bool {
        self.clip_layer_length < self.scroll_layer_length
    }

    pub(crate) fn thumb_rect(&self) -> Option<Rect> {
        scrollbar::compute_thumb_rect(
            self.orientation,
            self.current_pos,
            self.clip_layer_length,
            self.scroll_layer_length,
            self.cross_length,
        )
    }

    pub(crate) fn set_state(&mut self, state: ScrollbarState) {
        self.state = state;
    }

    /// Chrome: hit-testable area is wider than the visual thumb for easy grabbing.
    /// `kOffSideMultiplier` in Chrome expands the hit area.
    pub(crate) fn identify_part(&self, local_point: Point) -> ScrollbarPart {
        let Some(thumb) = self.thumb_rect() else {
            return ScrollbarPart::NoPart;
        };

        // Hit area is 3x the visual thickness for easy targeting.
        let hit_expansion = THICKNESS * 1.5;
        let in_track = match self.orientation {
            Orientation::Vertical => {
                local_point.x >= thumb.x() - hit_expansion
                    && local_point.x <= thumb.right() + hit_expansion
            }
            Orientation::Horizontal => {
                local_point.y >= thumb.y() - hit_expansion
                    && local_point.y <= thumb.bottom() + hit_expansion
            }
        };
        if !in_track {
            return ScrollbarPart::NoPart;
        }

        if thumb.contains_point(local_point) {
            return ScrollbarPart::Thumb;
        }

        // Expand thumb hit area vertically/horizontally too.
        let expanded_thumb = match self.orientation {
            Orientation::Vertical => Rect::new(
                thumb.x() - hit_expansion,
                thumb.y(),
                thumb.width() + 2.0 * hit_expansion,
                thumb.height(),
            ),
            Orientation::Horizontal => Rect::new(
                thumb.x(),
                thumb.y() - hit_expansion,
                thumb.width(),
                thumb.height() + 2.0 * hit_expansion,
            ),
        };
        if expanded_thumb.contains_point(local_point) {
            return ScrollbarPart::Thumb;
        }

        match self.orientation {
            Orientation::Vertical if local_point.y < thumb.y() => ScrollbarPart::BackTrack,
            Orientation::Horizontal if local_point.x < thumb.x() => ScrollbarPart::BackTrack,
            _ => ScrollbarPart::ForwardTrack,
        }
    }
}

impl LayerContent for ScrollbarLayer {
    /// Emit a screen-space quad for the scrollbar thumb.
    ///
    /// Chrome: `SolidColorScrollbarLayerImpl::AppendQuads()` emits quads
    /// in the layer's own coordinate space. The layer's `draw_transform`
    /// positions it on screen without page zoom affecting dimensions.
    ///
    /// Here we do the equivalent explicitly: convert from content space
    /// (where layout computed the geometry) to screen-logical space
    /// (where the renderer applies only device_scale). Position and
    /// track length scale by page_zoom; thickness stays constant.
    fn append_quads(&self, ctx: &QuadContext) -> SmallVec<[FrameQuad; 2]> {
        if !self.can_scroll() || self.opacity <= 0.0 {
            return SmallVec::new();
        }
        let Some(thumb) = self.thumb_rect() else {
            return SmallVec::new();
        };

        use super::frame::QuadSpace;

        let theme = ScrollbarTheme::get();
        let color = theme.thumb_color(self.state);
        let cr = ctx.container_rect;
        let z = ctx.page_zoom;

        // Container edges in screen-logical space.
        let scr_x = cr.x() * z;
        let scr_y = cr.y() * z;
        let scr_w = cr.width() * z;
        let scr_h = cr.height() * z;

        // Thumb track position (proportion along the scroll axis) scales
        // with zoom. Cross-axis position is pinned at a fixed screen-pixel
        // offset from the container edge — THICKNESS and MARGIN stay constant.
        let screen_rect = match self.orientation {
            Orientation::Vertical => Rect::new(
                scr_x + scr_w - THICKNESS - MARGIN,
                scr_y + thumb.y() * z,
                THICKNESS,
                thumb.height() * z,
            ),
            Orientation::Horizontal => Rect::new(
                scr_x + thumb.x() * z,
                scr_y + scr_h - THICKNESS - MARGIN,
                thumb.width() * z,
                THICKNESS,
            ),
        };
        let screen_clip = Rect::new(scr_x, scr_y, scr_w, scr_h);

        smallvec![FrameQuad {
            rect: screen_rect,
            clip: Some(screen_clip),
            color,
            radius: THICKNESS / 2.0,
            opacity: self.opacity,
            space: QuadSpace::Screen,
        }]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertical(pos: f32, viewport: f32, content: f32) -> ScrollbarLayer {
        let mut sb = ScrollbarLayer::new(1, Orientation::Vertical);
        sb.update_geometry(pos, viewport, content, 200.0);
        sb.opacity = 1.0;
        sb
    }

    fn ctx(w: f32, h: f32) -> QuadContext {
        QuadContext {
            _origin: Point::ZERO,
            container_rect: Rect::new(0.0, 0.0, w, h),
            page_zoom: 1.0,
        }
    }

    #[test]
    fn no_quads_without_overflow() {
        let sb = vertical(0.0, 400.0, 400.0);
        assert!(sb.append_quads(&ctx(200.0, 400.0)).is_empty());
    }

    #[test]
    fn no_quads_when_opacity_zero() {
        let mut sb = vertical(0.0, 400.0, 1200.0);
        sb.opacity = 0.0;
        assert!(sb.append_quads(&ctx(200.0, 400.0)).is_empty());
    }

    #[test]
    fn produces_quad_with_overflow() {
        let sb = vertical(0.0, 400.0, 1200.0);
        let quads = sb.append_quads(&ctx(200.0, 400.0));
        assert_eq!(quads.len(), 1);
    }

    #[test]
    fn identify_thumb() {
        let sb = vertical(0.0, 400.0, 1200.0);
        let thumb = sb.thumb_rect().expect("overflow");
        let mid = Point::new(thumb.x() + 1.0, thumb.y() + thumb.height() / 2.0);
        assert_eq!(sb.identify_part(mid), ScrollbarPart::Thumb);
    }

    #[test]
    fn identify_back_track() {
        let sb = vertical(400.0, 400.0, 1200.0);
        let thumb = sb.thumb_rect().expect("overflow");
        let above = Point::new(thumb.x() + 1.0, thumb.y() - 5.0);
        assert_eq!(sb.identify_part(above), ScrollbarPart::BackTrack);
    }

    #[test]
    fn identify_forward_track() {
        let sb = vertical(0.0, 400.0, 1200.0);
        let thumb = sb.thumb_rect().expect("overflow");
        let below = Point::new(thumb.x() + 1.0, thumb.bottom() + 5.0);
        assert_eq!(sb.identify_part(below), ScrollbarPart::ForwardTrack);
    }

    #[test]
    fn identify_outside() {
        let sb = vertical(0.0, 400.0, 1200.0);
        assert_eq!(sb.identify_part(Point::new(0.0, 200.0)), ScrollbarPart::NoPart);
    }
}
