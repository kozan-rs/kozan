//! Type conversions between Kozan primitives and vello/kurbo/peniko types.

use kozan_core::paint::display_item::BorderRadii;
use kozan_primitives::color::Color;
use kozan_primitives::geometry::Rect;
use vello::kurbo;
use vello::peniko;

/// Convert a Kozan `Color` (straight alpha, f32) to a `peniko::Color`.
#[inline]
pub fn to_peniko_color(c: Color) -> peniko::Color {
    peniko::Color::from_rgba8(
        (c.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.b * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.a * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Convert a Kozan `Rect` to a `kurbo::Rect`.
#[inline]
pub fn to_kurbo_rect(r: Rect) -> kurbo::Rect {
    kurbo::Rect::new(
        r.x() as f64,
        r.y() as f64,
        (r.x() + r.width()) as f64,
        (r.y() + r.height()) as f64,
    )
}

/// Convert a Kozan `Rect` + `BorderRadii` to a `kurbo::RoundedRect`.
#[inline]
pub fn to_kurbo_rounded_rect(r: Rect, radii: BorderRadii) -> kurbo::RoundedRect {
    kurbo::RoundedRect::new(
        r.x() as f64,
        r.y() as f64,
        (r.x() + r.width()) as f64,
        (r.y() + r.height()) as f64,
        (
            radii.top_left as f64,
            radii.top_right as f64,
            radii.bottom_right as f64,
            radii.bottom_left as f64,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_red_converts_to_peniko() {
        let c = Color::rgb(1.0, 0.0, 0.0);
        let p = to_peniko_color(c);
        // peniko Color stores components as [u8; 4] via from_rgba8
        assert_eq!(p, peniko::Color::from_rgba8(255, 0, 0, 255));
    }

    #[test]
    fn half_alpha_converts_correctly() {
        let c = Color::rgba(0.0, 0.0, 0.0, 0.5);
        let p = to_peniko_color(c);
        assert_eq!(p, peniko::Color::from_rgba8(0, 0, 0, 128));
    }

    #[test]
    fn transparent_black_converts_to_zero_alpha() {
        let c = Color::TRANSPARENT;
        let p = to_peniko_color(c);
        assert_eq!(p, peniko::Color::from_rgba8(0, 0, 0, 0));
    }

    #[test]
    fn rect_at_origin_converts_to_kurbo() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let k = to_kurbo_rect(r);
        assert_eq!(k, kurbo::Rect::new(0.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn rect_with_offset_converts_to_kurbo() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        let k = to_kurbo_rect(r);
        // kurbo::Rect is (x0, y0, x1, y1)
        assert_eq!(k, kurbo::Rect::new(10.0, 20.0, 40.0, 60.0));
    }

    #[test]
    fn zero_rect_converts_to_zero_kurbo() {
        let r = Rect::ZERO;
        let k = to_kurbo_rect(r);
        assert_eq!(k, kurbo::Rect::new(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn rounded_rect_preserves_radii() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let radii = BorderRadii {
            top_left: 5.0,
            top_right: 10.0,
            bottom_right: 15.0,
            bottom_left: 20.0,
        };
        let rr = to_kurbo_rounded_rect(r, radii);
        let rect = rr.rect();
        assert_eq!(rect, kurbo::Rect::new(0.0, 0.0, 100.0, 100.0));
        let kr = rr.radii();
        assert_eq!(kr.top_left, 5.0);
        assert_eq!(kr.top_right, 10.0);
        assert_eq!(kr.bottom_right, 15.0);
        assert_eq!(kr.bottom_left, 20.0);
    }

    #[test]
    fn zero_radii_produces_sharp_corners() {
        let r = Rect::new(0.0, 0.0, 50.0, 50.0);
        let radii = BorderRadii::default();
        let rr = to_kurbo_rounded_rect(r, radii);
        let kr = rr.radii();
        assert_eq!(kr.top_left, 0.0);
        assert_eq!(kr.top_right, 0.0);
        assert_eq!(kr.bottom_right, 0.0);
        assert_eq!(kr.bottom_left, 0.0);
    }
}
