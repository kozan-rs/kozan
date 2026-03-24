use crate::geometry::{Corners, Point, Rect, Size};

/// A rectangle with independently rounded corners.
///
/// Used throughout the platform for border-radius rendering, clipping
/// regions, and hit testing. Each corner has its own elliptical radius
/// defined as a [`Size`] (horizontal × vertical), supporting non-circular
/// corners like `border-radius: 20px / 10px`.
///
/// When all four radii are zero this degenerates to a plain [`Rect`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedRect {
    pub rect: Rect,
    pub radii: Corners<Size>,
}

impl RoundedRect {
    #[must_use]
    pub fn new(rect: Rect, radii: Corners<Size>) -> Self {
        Self { rect, radii }
    }

    /// A rounded rect with no rounding — equivalent to a plain rect.
    #[must_use]
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            rect,
            radii: Corners::all(Size::ZERO),
        }
    }

    /// All four corners share the same circular radius.
    #[must_use]
    pub fn uniform(rect: Rect, radius: f32) -> Self {
        Self {
            rect,
            radii: Corners::all(Size::new(radius, radius)),
        }
    }

    /// True when all corner radii are zero.
    #[must_use]
    pub fn is_sharp(&self) -> bool {
        self.radii.top_left == Size::ZERO
            && self.radii.top_right == Size::ZERO
            && self.radii.bottom_right == Size::ZERO
            && self.radii.bottom_left == Size::ZERO
    }

    /// True when the rounded rect forms a full ellipse/circle (each
    /// corner's radius is exactly half the rect's dimension).
    #[must_use]
    pub fn is_ellipse(&self) -> bool {
        let hw = self.rect.width() * 0.5;
        let hh = self.rect.height() * 0.5;
        let expected = Size::new(hw, hh);
        self.radii.top_left == expected
            && self.radii.top_right == expected
            && self.radii.bottom_right == expected
            && self.radii.bottom_left == expected
    }

    /// Scale radii down proportionally when the sum of adjacent radii
    /// exceeds the rect's dimension. This implements the CSS spec's
    /// corner-overlap rule.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        let w = self.rect.width();
        let h = self.rect.height();

        if w <= 0.0 || h <= 0.0 {
            return Self::from_rect(self.rect);
        }

        // For each edge, if the sum of its two corner radii exceeds the
        // edge length, compute a scale factor. The final factor is the
        // minimum across all four edges.
        let mut scale = 1.0f32;

        // Top edge
        let top_sum = self.radii.top_left.width + self.radii.top_right.width;
        if top_sum > w {
            scale = scale.min(w / top_sum);
        }

        // Bottom edge
        let bottom_sum = self.radii.bottom_left.width + self.radii.bottom_right.width;
        if bottom_sum > w {
            scale = scale.min(w / bottom_sum);
        }

        // Left edge
        let left_sum = self.radii.top_left.height + self.radii.bottom_left.height;
        if left_sum > h {
            scale = scale.min(h / left_sum);
        }

        // Right edge
        let right_sum = self.radii.top_right.height + self.radii.bottom_right.height;
        if right_sum > h {
            scale = scale.min(h / right_sum);
        }

        if scale < 1.0 {
            self.radii.top_left = scale_size(self.radii.top_left, scale);
            self.radii.top_right = scale_size(self.radii.top_right, scale);
            self.radii.bottom_right = scale_size(self.radii.bottom_right, scale);
            self.radii.bottom_left = scale_size(self.radii.bottom_left, scale);
        }

        self
    }

    /// Hit test: is `point` inside this rounded rect?
    ///
    /// First checks the bounding rect, then tests against the elliptical
    /// corner arcs for points that fall within a corner region.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        if !self.rect.contains_point(point) {
            return false;
        }

        let norm = self.normalized();

        // Check each corner's elliptical region
        let x = point.x;
        let y = point.y;
        let left = norm.rect.left();
        let top = norm.rect.top();
        let right = norm.rect.right();
        let bottom = norm.rect.bottom();

        // Top-left corner
        let r = norm.radii.top_left;
        if x < left + r.width
            && y < top + r.height
            && !point_in_ellipse(
                x - (left + r.width),
                y - (top + r.height),
                r.width,
                r.height,
            )
        {
            return false;
        }

        // Top-right corner
        let r = norm.radii.top_right;
        if x > right - r.width
            && y < top + r.height
            && !point_in_ellipse(
                x - (right - r.width),
                y - (top + r.height),
                r.width,
                r.height,
            )
        {
            return false;
        }

        // Bottom-right corner
        let r = norm.radii.bottom_right;
        if x > right - r.width
            && y > bottom - r.height
            && !point_in_ellipse(
                x - (right - r.width),
                y - (bottom - r.height),
                r.width,
                r.height,
            )
        {
            return false;
        }

        // Bottom-left corner
        let r = norm.radii.bottom_left;
        if x < left + r.width
            && y > bottom - r.height
            && !point_in_ellipse(
                x - (left + r.width),
                y - (bottom - r.height),
                r.width,
                r.height,
            )
        {
            return false;
        }

        true
    }
}

fn scale_size(s: Size, factor: f32) -> Size {
    Size::new(s.width * factor, s.height * factor)
}

/// Test whether (dx, dy) relative to an ellipse center lies inside
/// the ellipse with semi-axes (rx, ry).
fn point_in_ellipse(dx: f32, dy: f32, rx: f32, ry: f32) -> bool {
    if rx <= 0.0 || ry <= 0.0 {
        return true;
    }
    let nx = dx / rx;
    let ny = dy / ry;
    nx * nx + ny * ny <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Rect {
        Rect::new(0.0, 0.0, 100.0, 100.0)
    }

    #[test]
    fn sharp_rect() {
        let rr = RoundedRect::from_rect(square());
        assert!(rr.is_sharp());
        assert!(!rr.is_ellipse());
    }

    #[test]
    fn uniform_radius() {
        let rr = RoundedRect::uniform(square(), 10.0);
        assert!(!rr.is_sharp());
        assert!(!rr.is_ellipse());
    }

    #[test]
    fn full_ellipse() {
        let rr = RoundedRect::uniform(square(), 50.0);
        assert!(rr.is_ellipse());
    }

    #[test]
    fn contains_center() {
        let rr = RoundedRect::uniform(square(), 20.0);
        assert!(rr.contains(Point::new(50.0, 50.0)));
    }

    #[test]
    fn excludes_outside() {
        let rr = RoundedRect::uniform(square(), 20.0);
        assert!(!rr.contains(Point::new(-1.0, 50.0)));
        assert!(!rr.contains(Point::new(101.0, 50.0)));
    }

    #[test]
    fn excludes_rounded_corner() {
        // Point at (1, 1) is inside the bounding rect but outside the
        // rounded corner with radius 20.
        let rr = RoundedRect::uniform(square(), 20.0);
        assert!(!rr.contains(Point::new(1.0, 1.0)));
    }

    #[test]
    fn includes_just_inside_corner() {
        let rr = RoundedRect::uniform(square(), 20.0);
        // Point at (10, 10) is inside the corner arc (distance from
        // corner center (20,20) is ~14.1, less than radius 20).
        assert!(rr.contains(Point::new(10.0, 10.0)));
    }

    #[test]
    fn normalize_scales_overlapping_radii() {
        // Radii sum to 200 on each edge but rect is only 100 wide.
        let rr = RoundedRect::uniform(square(), 100.0);
        let norm = rr.normalized();
        // After normalization, each radius should be 50 (half the edge).
        assert!((norm.radii.top_left.width - 50.0).abs() < 0.01);
    }

    #[test]
    fn sharp_rect_contains_like_rect() {
        let rr = RoundedRect::from_rect(square());
        assert!(rr.contains(Point::new(0.0, 0.0)));
        assert!(rr.contains(Point::new(99.0, 99.0)));
        assert!(!rr.contains(Point::new(100.0, 100.0)));
    }
}
