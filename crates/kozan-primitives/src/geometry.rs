/// A 2D point in logical coordinates (before DPI scaling).
///
/// A single `f32`-based type used throughout the platform. A
/// physical vs logical distinction (for DPI awareness) can be
/// added later with a unit marker generic if needed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn distance_to(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    #[must_use]
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

impl std::ops::Add<Offset> for Point {
    type Output = Self;
    fn add(self, rhs: Offset) -> Self {
        Self {
            x: self.x + rhs.dx,
            y: self.y + rhs.dy,
        }
    }
}

impl std::ops::Sub for Point {
    type Output = Offset;
    fn sub(self, rhs: Self) -> Offset {
        Offset {
            dx: self.x - rhs.x,
            dy: self.y - rhs.y,
        }
    }
}

impl std::ops::Sub<Offset> for Point {
    type Output = Self;
    fn sub(self, rhs: Offset) -> Self {
        Self {
            x: self.x - rhs.dx,
            y: self.y - rhs.dy,
        }
    }
}

/// A 2D displacement vector.
///
/// Semantically different from [`Point`]: a point is a position,
/// an offset is a delta between two positions. `Point - Point = Offset`,
/// `Point + Offset = Point`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offset {
    pub dx: f32,
    pub dy: f32,
}

impl Offset {
    pub const ZERO: Self = Self { dx: 0.0, dy: 0.0 };

    #[must_use]
    pub const fn new(dx: f32, dy: f32) -> Self {
        Self { dx, dy }
    }

    #[must_use]
    pub fn length(self) -> f32 {
        (self.dx * self.dx + self.dy * self.dy).sqrt()
    }
}

impl std::ops::Add for Offset {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            dx: self.dx + rhs.dx,
            dy: self.dy + rhs.dy,
        }
    }
}

impl std::ops::Neg for Offset {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            dx: -self.dx,
            dy: -self.dy,
        }
    }
}

/// A 2D size (width × height). Never negative.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub fn area(self) -> f32 {
        self.width * self.height
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        point.x >= 0.0 && point.x < self.width && point.y >= 0.0 && point.y < self.height
    }
}

/// An axis-aligned rectangle defined by origin + size.
///
/// Origin + size is the canonical representation. Edge accessors
/// (`left`, `top`, `right`, `bottom`) are provided for convenience.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    #[must_use]
    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Construct from left, top, right, bottom edges.
    #[must_use]
    pub fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            origin: Point::new(left, top),
            size: Size::new(right - left, bottom - top),
        }
    }

    #[must_use]
    pub fn x(&self) -> f32 {
        self.origin.x
    }
    #[must_use]
    pub fn y(&self) -> f32 {
        self.origin.y
    }
    #[must_use]
    pub fn width(&self) -> f32 {
        self.size.width
    }
    #[must_use]
    pub fn height(&self) -> f32 {
        self.size.height
    }

    #[must_use]
    pub fn left(&self) -> f32 {
        self.origin.x
    }
    #[must_use]
    pub fn top(&self) -> f32 {
        self.origin.y
    }
    #[must_use]
    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width * 0.5,
            self.origin.y + self.size.height * 0.5,
        )
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size.is_empty()
    }

    #[must_use]
    pub fn contains_point(&self, p: Point) -> bool {
        p.x >= self.left() && p.x < self.right() && p.y >= self.top() && p.y < self.bottom()
    }

    #[must_use]
    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.left() >= self.left()
            && other.top() >= self.top()
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    /// Returns the intersection of two rects, or `None` if they don't overlap.
    #[must_use]
    pub fn intersection(&self, other: &Rect) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }
        let left = self.left().max(other.left());
        let top = self.top().max(other.top());
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Some(Self::from_ltrb(left, top, right, bottom))
    }

    /// Smallest rect that contains both.
    #[must_use]
    pub fn union(&self, other: &Rect) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::from_ltrb(left, top, right, bottom)
    }

    /// Expand each edge outward.
    #[must_use]
    pub fn inflate(&self, dx: f32, dy: f32) -> Self {
        Self::new(
            self.origin.x - dx,
            self.origin.y - dy,
            self.size.width + dx * 2.0,
            self.size.height + dy * 2.0,
        )
    }

    /// Shrink each edge inward by per-edge amounts. Clamps size to zero.
    #[must_use]
    pub fn inset(&self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self::new(
            self.origin.x + left,
            self.origin.y + top,
            (self.size.width - left - right).max(0.0),
            (self.size.height - top - bottom).max(0.0),
        )
    }

    /// Expand each edge outward by per-edge amounts.
    #[must_use]
    pub fn outset(&self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self::new(
            self.origin.x - left,
            self.origin.y - top,
            self.size.width + left + right,
            self.size.height + top + bottom,
        )
    }

    #[must_use]
    pub fn translate(&self, offset: Offset) -> Self {
        Self {
            origin: self.origin + offset,
            size: self.size,
        }
    }
}

/// Per-edge values (top, right, bottom, left). Used for margin, padding,
/// border widths — anything that has four directional values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges<T: Copy> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> Edges<T> {
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn all(value: T) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(vertical: T, horizontal: T) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// Per-corner values (top-left, top-right, bottom-right, bottom-left).
/// Used for border-radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners<T: Copy> {
    pub top_left: T,
    pub top_right: T,
    pub bottom_right: T,
    pub bottom_left: T,
}

impl<T: Copy> Corners<T> {
    pub const fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn all(value: T) -> Self {
        Self {
            top_left: value,
            top_right: value,
            bottom_right: value,
            bottom_left: value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_offset_arithmetic() {
        let a = Point::new(10.0, 20.0);
        let b = Point::new(30.0, 50.0);

        let delta = b - a;
        assert_eq!(delta, Offset::new(20.0, 30.0));

        let c = a + delta;
        assert_eq!(c, b);
    }

    #[test]
    fn point_distance() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        assert!((a.distance_to(b) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn size_empty_and_area() {
        assert!(Size::ZERO.is_empty());
        assert!(!Size::new(10.0, 5.0).is_empty());
        assert_eq!(Size::new(3.0, 4.0).area(), 12.0);
    }

    #[test]
    fn rect_edges() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.left(), 10.0);
        assert_eq!(r.top(), 20.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 70.0);
        assert_eq!(r.center(), Point::new(60.0, 45.0));
    }

    #[test]
    fn rect_from_ltrb() {
        let r = Rect::from_ltrb(10.0, 20.0, 110.0, 70.0);
        assert_eq!(r.origin, Point::new(10.0, 20.0));
        assert_eq!(r.size, Size::new(100.0, 50.0));
    }

    #[test]
    fn rect_contains_point() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(r.contains_point(Point::new(50.0, 50.0)));
        assert!(r.contains_point(Point::new(0.0, 0.0)));
        assert!(!r.contains_point(Point::new(100.0, 100.0))); // exclusive right/bottom
        assert!(!r.contains_point(Point::new(-1.0, 50.0)));
    }

    #[test]
    fn rect_intersection() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let c = Rect::new(200.0, 200.0, 10.0, 10.0);

        let ab = a.intersection(&b).unwrap();
        assert_eq!(ab, Rect::new(50.0, 50.0, 50.0, 50.0));

        assert!(a.intersection(&c).is_none());
    }

    #[test]
    fn rect_union() {
        let a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let b = Rect::new(25.0, 25.0, 50.0, 50.0);

        let u = a.union(&b);
        assert_eq!(u, Rect::new(0.0, 0.0, 75.0, 75.0));
    }

    #[test]
    fn rect_inflate() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        let expanded = r.inflate(5.0, 5.0);
        assert_eq!(expanded, Rect::new(5.0, 5.0, 30.0, 30.0));
    }

    #[test]
    fn rect_translate() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        let moved = r.translate(Offset::new(5.0, -5.0));
        assert_eq!(moved, Rect::new(15.0, 15.0, 30.0, 40.0));
    }

    #[test]
    fn edges_constructors() {
        let uniform = Edges::all(8.0f32);
        assert_eq!(uniform.top, 8.0);
        assert_eq!(uniform.right, 8.0);

        let sym = Edges::symmetric(10.0f32, 20.0);
        assert_eq!(sym.top, 10.0);
        assert_eq!(sym.right, 20.0);
        assert_eq!(sym.bottom, 10.0);
        assert_eq!(sym.left, 20.0);
    }

    #[test]
    fn corners_constructor() {
        let c = Corners::all(4.0f32);
        assert_eq!(c.top_left, 4.0);
        assert_eq!(c.bottom_right, 4.0);
    }

    #[test]
    fn offset_neg() {
        let o = Offset::new(10.0, -5.0);
        assert_eq!(-o, Offset::new(-10.0, 5.0));
    }
}
