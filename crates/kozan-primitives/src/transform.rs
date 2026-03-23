use crate::geometry::{Point, Rect};

/// A 2D affine transformation (translate, rotate, scale, skew).
///
/// Stored as a 3×2 matrix internally via [`glam::Affine2`], which gives
/// us SIMD-optimized operations on supported platforms. The matrix layout:
///
/// ```text
/// | a  c  tx |
/// | b  d  ty |
/// | 0  0  1  |
/// ```
///
/// Use this for standard 2D UI work — CSS transforms, canvas drawing,
/// element positioning. For 3D perspective and rotations around X/Y axes,
/// use [`Transform3D`] instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineTransform {
    inner: glam::Affine2,
}

impl AffineTransform {
    pub const IDENTITY: Self = Self {
        inner: glam::Affine2::IDENTITY,
    };

    /// Construct from the six matrix components directly.
    #[must_use] 
    pub fn new(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Self {
        Self {
            inner: glam::Affine2::from_cols(
                glam::Vec2::new(a, b),
                glam::Vec2::new(c, d),
                glam::Vec2::new(tx, ty),
            ),
        }
    }

    #[must_use] 
    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            inner: glam::Affine2::from_translation(glam::Vec2::new(tx, ty)),
        }
    }

    #[must_use] 
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            inner: glam::Affine2::from_scale(glam::Vec2::new(sx, sy)),
        }
    }

    #[must_use] 
    pub fn uniform_scale(s: f32) -> Self {
        Self::scale(s, s)
    }

    /// Counter-clockwise rotation by `angle` radians.
    #[must_use] 
    pub fn rotate(angle: f32) -> Self {
        Self {
            inner: glam::Affine2::from_angle(angle),
        }
    }

    #[must_use] 
    pub fn is_identity(&self) -> bool {
        self.inner == glam::Affine2::IDENTITY
    }

    /// True when the transform only translates (no rotation, scale, or skew).
    #[must_use] 
    pub fn is_translation_only(&self) -> bool {
        self.inner.matrix2 == glam::Mat2::IDENTITY
    }

    /// True when axis-aligned rectangles stay axis-aligned after
    /// transformation (no rotation or skew — only scale and translation).
    #[must_use] 
    pub fn preserves_axis_alignment(&self) -> bool {
        let m = self.inner.matrix2;
        (m.x_axis.y == 0.0 && m.y_axis.x == 0.0) || (m.x_axis.x == 0.0 && m.y_axis.y == 0.0)
    }

    /// Compose: apply `self` first, then `other`.
    #[must_use] 
    pub fn then(&self, other: &Self) -> Self {
        Self {
            inner: other.inner * self.inner,
        }
    }

    /// Prepend a translation to this transform.
    #[must_use] 
    pub fn pre_translate(&self, tx: f32, ty: f32) -> Self {
        let t = glam::Affine2::from_translation(glam::Vec2::new(tx, ty));
        Self {
            inner: self.inner * t,
        }
    }

    /// Prepend a scale to this transform.
    #[must_use] 
    pub fn pre_scale(&self, sx: f32, sy: f32) -> Self {
        let s = glam::Affine2::from_scale(glam::Vec2::new(sx, sy));
        Self {
            inner: self.inner * s,
        }
    }

    /// Compute the inverse. Returns `None` for singular (degenerate)
    /// transforms where the determinant is zero.
    #[must_use] 
    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() < f32::EPSILON {
            return None;
        }
        Some(Self {
            inner: self.inner.inverse(),
        })
    }

    #[must_use] 
    pub fn determinant(&self) -> f32 {
        self.inner.matrix2.determinant()
    }

    #[must_use] 
    pub fn transform_point(&self, p: Point) -> Point {
        let result = self.inner.transform_point2(glam::Vec2::new(p.x, p.y));
        Point::new(result.x, result.y)
    }

    /// Transform a rectangle. When the transform involves rotation or skew,
    /// the result is the axis-aligned bounding box of the transformed corners.
    pub fn transform_rect(&self, r: &Rect) -> Rect {
        if self.preserves_axis_alignment() {
            let p0 = self.transform_point(r.origin);
            let p1 = self.transform_point(Point::new(r.right(), r.bottom()));
            let min_x = p0.x.min(p1.x);
            let min_y = p0.y.min(p1.y);
            let max_x = p0.x.max(p1.x);
            let max_y = p0.y.max(p1.y);
            return Rect::from_ltrb(min_x, min_y, max_x, max_y);
        }

        let corners = [
            self.transform_point(Point::new(r.left(), r.top())),
            self.transform_point(Point::new(r.right(), r.top())),
            self.transform_point(Point::new(r.right(), r.bottom())),
            self.transform_point(Point::new(r.left(), r.bottom())),
        ];

        let min_x = corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let min_y = corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_x = corners
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = corners
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);

        Rect::from_ltrb(min_x, min_y, max_x, max_y)
    }

    /// Access the underlying `glam::Affine2` for interop with rendering
    /// libraries that accept glam types directly.
    #[must_use] 
    pub fn as_raw(&self) -> &glam::Affine2 {
        &self.inner
    }
}

impl Default for AffineTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl std::ops::Mul for AffineTransform {
    type Output = Self;
    /// `a * b` means "apply b first, then a".
    fn mul(self, rhs: Self) -> Self {
        rhs.then(&self)
    }
}

/// A full 3D transformation stored as a 4×4 matrix.
///
/// Backed by [`glam::Mat4`] which provides SIMD-optimized operations.
/// Needed for perspective transforms, 3D rotations, and compositing
/// layers that exist in 3D space.
///
/// For pure 2D work, prefer [`AffineTransform`] — it uses fewer
/// operations and less memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform3D {
    inner: glam::Mat4,
}

impl Transform3D {
    pub const IDENTITY: Self = Self {
        inner: glam::Mat4::IDENTITY,
    };

    #[must_use] 
    pub fn translate(tx: f32, ty: f32, tz: f32) -> Self {
        Self {
            inner: glam::Mat4::from_translation(glam::Vec3::new(tx, ty, tz)),
        }
    }

    #[must_use] 
    pub fn scale(sx: f32, sy: f32, sz: f32) -> Self {
        Self {
            inner: glam::Mat4::from_scale(glam::Vec3::new(sx, sy, sz)),
        }
    }

    #[must_use] 
    pub fn rotate_x(angle: f32) -> Self {
        Self {
            inner: glam::Mat4::from_rotation_x(angle),
        }
    }

    #[must_use] 
    pub fn rotate_y(angle: f32) -> Self {
        Self {
            inner: glam::Mat4::from_rotation_y(angle),
        }
    }

    /// Rotation around the Z axis (equivalent to 2D rotation).
    #[must_use] 
    pub fn rotate_z(angle: f32) -> Self {
        Self {
            inner: glam::Mat4::from_rotation_z(angle),
        }
    }

    /// A perspective projection that makes distant objects appear smaller.
    /// `depth` is the distance from the viewer to the z=0 plane.
    #[must_use] 
    pub fn perspective(depth: f32) -> Self {
        if depth == 0.0 {
            return Self::IDENTITY;
        }
        let mut m = glam::Mat4::IDENTITY;
        // Set m[3][2] = -1/d for CSS-style perspective.
        *m.col_mut(2) = glam::Vec4::new(0.0, 0.0, 1.0, -1.0 / depth);
        Self { inner: m }
    }

    #[must_use] 
    pub fn is_identity(&self) -> bool {
        self.inner == glam::Mat4::IDENTITY
    }

    /// True when the transform operates only in the X/Y plane (no Z
    /// rotation, no perspective, no Z translation). Can be losslessly
    /// represented as an [`AffineTransform`].
    #[must_use] 
    pub fn is_2d(&self) -> bool {
        let m = self.inner;
        let c = |col: usize, row: usize| m.col(col)[row];
        c(0, 2) == 0.0
            && c(1, 2) == 0.0
            && c(2, 0) == 0.0
            && c(2, 1) == 0.0
            && c(2, 2) == 1.0
            && c(2, 3) == 0.0
            && c(3, 2) == 0.0
            && c(0, 3) == 0.0
            && c(1, 3) == 0.0
            && c(3, 3) == 1.0
    }

    /// Extract a 2D affine transform, dropping the Z components.
    /// Only meaningful when [`is_2d`](Self::is_2d) returns true.
    #[must_use] 
    pub fn to_affine(&self) -> AffineTransform {
        let m = self.inner;
        AffineTransform::new(
            m.col(0)[0],
            m.col(0)[1],
            m.col(1)[0],
            m.col(1)[1],
            m.col(3)[0],
            m.col(3)[1],
        )
    }

    /// Promote a 2D affine transform to a 3D matrix.
    #[must_use] 
    pub fn from_affine(t: &AffineTransform) -> Self {
        let a = t.inner;
        let m2 = a.matrix2;
        let tr = a.translation;
        Self {
            inner: glam::Mat4::from_cols(
                glam::Vec4::new(m2.x_axis.x, m2.x_axis.y, 0.0, 0.0),
                glam::Vec4::new(m2.y_axis.x, m2.y_axis.y, 0.0, 0.0),
                glam::Vec4::new(0.0, 0.0, 1.0, 0.0),
                glam::Vec4::new(tr.x, tr.y, 0.0, 1.0),
            ),
        }
    }

    #[must_use] 
    pub fn then(&self, other: &Self) -> Self {
        Self {
            inner: other.inner * self.inner,
        }
    }

    /// Compute the inverse. Returns `None` if the matrix is singular.
    #[must_use] 
    pub fn inverse(&self) -> Option<Self> {
        let det = self.inner.determinant();
        if det.abs() < f32::EPSILON {
            return None;
        }
        Some(Self {
            inner: self.inner.inverse(),
        })
    }

    #[must_use] 
    pub fn determinant(&self) -> f32 {
        self.inner.determinant()
    }

    /// Transform a 2D point through this 3D matrix. The point is treated
    /// as (x, y, 0, 1) and projected back to 2D by dividing by the
    /// homogeneous `w` coordinate.
    #[must_use] 
    pub fn transform_point(&self, p: Point) -> Point {
        let v = self.inner * glam::Vec4::new(p.x, p.y, 0.0, 1.0);
        if v.w.abs() < f32::EPSILON {
            return Point::new(v.x, v.y);
        }
        Point::new(v.x / v.w, v.y / v.w)
    }

    /// True when the back face of a transformed plane would be visible
    /// (the Z component of the transformed normal is negative).
    #[must_use] 
    pub fn is_back_face_visible(&self) -> bool {
        // The Z component of the transformed normal tells us if the
        // surface faces away from the viewer. Computed from the
        // determinant of the upper-left 3×3 sub-matrix.
        let m = self.inner;
        let col0 = glam::Vec3::new(m.col(0)[0], m.col(0)[1], m.col(0)[2]);
        let col1 = glam::Vec3::new(m.col(1)[0], m.col(1)[1], m.col(1)[2]);
        let col2 = glam::Vec3::new(m.col(2)[0], m.col(2)[1], m.col(2)[2]);
        col0.cross(col1).dot(col2) < 0.0
    }

    /// Access the underlying `glam::Mat4` for interop with rendering
    /// libraries that accept glam types directly.
    #[must_use] 
    pub fn as_raw(&self) -> &glam::Mat4 {
        &self.inner
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl std::ops::Mul for Transform3D {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        rhs.then(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn point_approx_eq(a: Point, b: Point) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
    }

    #[test]
    fn identity_does_nothing() {
        let p = Point::new(42.0, 17.0);
        assert_eq!(AffineTransform::IDENTITY.transform_point(p), p);
    }

    #[test]
    fn translate_moves_point() {
        let t = AffineTransform::translate(10.0, 20.0);
        assert_eq!(t.transform_point(Point::ZERO), Point::new(10.0, 20.0));
    }

    #[test]
    fn scale_multiplies() {
        let t = AffineTransform::scale(2.0, 3.0);
        assert_eq!(
            t.transform_point(Point::new(5.0, 10.0)),
            Point::new(10.0, 30.0)
        );
    }

    #[test]
    fn rotate_90_degrees() {
        let t = AffineTransform::rotate(FRAC_PI_2);
        let result = t.transform_point(Point::new(1.0, 0.0));
        assert!(point_approx_eq(result, Point::new(0.0, 1.0)));
    }

    #[test]
    fn compose_scale_then_translate() {
        let s = AffineTransform::scale(2.0, 2.0);
        let t = AffineTransform::translate(10.0, 0.0);
        let combined = s.then(&t);
        let p = combined.transform_point(Point::new(5.0, 0.0));
        assert_eq!(p, Point::new(20.0, 0.0));
    }

    #[test]
    fn inverse_roundtrip() {
        let t = AffineTransform::translate(10.0, 20.0)
            .then(&AffineTransform::scale(2.0, 3.0))
            .then(&AffineTransform::rotate(0.5));

        let inv = t.inverse().unwrap();
        let p = Point::new(42.0, 17.0);
        let roundtrip = inv.transform_point(t.transform_point(p));
        assert!(point_approx_eq(roundtrip, p));
    }

    #[test]
    fn singular_matrix_no_inverse() {
        let t = AffineTransform::scale(0.0, 1.0);
        assert!(t.inverse().is_none());
    }

    #[test]
    fn transform_rect_with_translation() {
        let t = AffineTransform::translate(10.0, 10.0);
        let r = Rect::new(0.0, 0.0, 50.0, 50.0);
        assert_eq!(t.transform_rect(&r), Rect::new(10.0, 10.0, 50.0, 50.0));
    }

    #[test]
    fn transform_rect_with_scale() {
        let t = AffineTransform::scale(2.0, 0.5);
        let r = Rect::new(10.0, 10.0, 20.0, 40.0);
        let result = t.transform_rect(&r);
        assert_eq!(result, Rect::new(20.0, 5.0, 40.0, 20.0));
    }

    #[test]
    fn mul_operator() {
        let a = AffineTransform::translate(5.0, 0.0);
        let b = AffineTransform::scale(2.0, 2.0);
        let p = (a * b).transform_point(Point::new(10.0, 0.0));
        assert_eq!(p, Point::new(25.0, 0.0));
    }

    #[test]
    fn is_translation_only() {
        assert!(AffineTransform::translate(1.0, 2.0).is_translation_only());
        assert!(!AffineTransform::scale(2.0, 1.0).is_translation_only());
        assert!(!AffineTransform::rotate(0.1).is_translation_only());
    }

    #[test]
    fn pre_translate() {
        let t = AffineTransform::scale(2.0, 2.0).pre_translate(5.0, 0.0);
        let p = t.transform_point(Point::ZERO);
        assert_eq!(p, Point::new(10.0, 0.0));
    }

    // --- Transform3D ---

    #[test]
    fn identity_3d() {
        let p = Point::new(42.0, 17.0);
        assert_eq!(Transform3D::IDENTITY.transform_point(p), p);
    }

    #[test]
    fn translate_3d() {
        let t = Transform3D::translate(10.0, 20.0, 0.0);
        assert_eq!(t.transform_point(Point::ZERO), Point::new(10.0, 20.0));
    }

    #[test]
    fn rotate_z_matches_2d() {
        let t2d = AffineTransform::rotate(FRAC_PI_2);
        let t3d = Transform3D::rotate_z(FRAC_PI_2);
        let p = Point::new(1.0, 0.0);
        assert!(point_approx_eq(
            t2d.transform_point(p),
            t3d.transform_point(p)
        ));
    }

    #[test]
    fn affine_roundtrip() {
        let t2d = AffineTransform::new(2.0, 0.5, -0.3, 1.5, 10.0, 20.0);
        let t3d = Transform3D::from_affine(&t2d);
        assert!(t3d.is_2d());

        let back = t3d.to_affine();
        assert_eq!(back, t2d);
    }

    #[test]
    fn perspective_leaves_z0_unchanged() {
        let t = Transform3D::perspective(100.0);
        let p = Point::new(50.0, 50.0);
        assert_eq!(t.transform_point(p), p);
    }

    #[test]
    fn is_2d_checks() {
        assert!(Transform3D::IDENTITY.is_2d());
        assert!(Transform3D::translate(1.0, 2.0, 0.0).is_2d());
        assert!(!Transform3D::translate(0.0, 0.0, 5.0).is_2d());
        assert!(!Transform3D::rotate_x(0.1).is_2d());
        assert!(!Transform3D::perspective(100.0).is_2d());
    }

    #[test]
    fn back_face_detection() {
        assert!(!Transform3D::IDENTITY.is_back_face_visible());
        // Use a scale with negative X — this flips the face unambiguously,
        // unlike rotate_y(PI) which has float precision issues at exactly 180°.
        assert!(Transform3D::scale(-1.0, 1.0, 1.0).is_back_face_visible());
        assert!(!Transform3D::scale(1.0, 1.0, 1.0).is_back_face_visible());
    }

    #[test]
    fn inverse_3d_roundtrip() {
        let t = Transform3D::translate(5.0, 10.0, 0.0)
            .then(&Transform3D::rotate_z(0.7))
            .then(&Transform3D::scale(2.0, 3.0, 1.0));

        let inv = t.inverse().unwrap();
        let p = Point::new(42.0, 17.0);
        let roundtrip = inv.transform_point(t.transform_point(p));
        assert!(point_approx_eq(roundtrip, p));
    }
}
