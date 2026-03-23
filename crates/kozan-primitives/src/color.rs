/// sRGB color with alpha, stored as four `f32` channels in [0.0, 1.0].
///
/// Uses straight (non-premultiplied) alpha. Conversion to premultiplied
/// happens at the rendering boundary when handing off to the GPU.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const RED: Self = Self::rgb(1.0, 0.0, 0.0);
    pub const GREEN: Self = Self::rgb(0.0, 1.0, 0.0);
    pub const BLUE: Self = Self::rgb(0.0, 0.0, 1.0);

    #[must_use] 
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    #[must_use] 
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Construct from 8-bit per channel values (0–255).
    #[must_use] 
    pub fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    #[must_use] 
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// Construct from a 32-bit hex value: `0xRRGGBB` or `0xRRGGBBAA`.
    #[must_use] 
    pub fn from_hex(hex: u32) -> Self {
        if hex > 0xFFFFFF {
            Self::from_rgba8(
                ((hex >> 24) & 0xFF) as u8,
                ((hex >> 16) & 0xFF) as u8,
                ((hex >> 8) & 0xFF) as u8,
                (hex & 0xFF) as u8,
            )
        } else {
            Self::from_rgb8(
                ((hex >> 16) & 0xFF) as u8,
                ((hex >> 8) & 0xFF) as u8,
                (hex & 0xFF) as u8,
            )
        }
    }

    #[must_use] 
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    #[must_use] 
    pub fn is_opaque(self) -> bool {
        self.a >= 1.0
    }

    #[must_use] 
    pub fn is_transparent(self) -> bool {
        self.a <= 0.0
    }

    /// Linear interpolation between two colors.
    #[must_use] 
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Pack to 32-bit RGBA (8 bits per channel).
    #[must_use] 
    pub fn to_rgba8(self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (self.g.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (self.b.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (self.a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        ]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_rgb8_roundtrip() {
        let c = Color::from_rgb8(128, 64, 255);
        let [r, g, b, a] = c.to_rgba8();
        assert_eq!(r, 128);
        assert_eq!(g, 64);
        assert_eq!(b, 255);
        assert_eq!(a, 255);
    }

    #[test]
    fn from_hex_rgb() {
        let c = Color::from_hex(0xFF8000);
        let [r, g, b, _] = c.to_rgba8();
        assert_eq!(r, 255);
        assert_eq!(g, 128);
        assert_eq!(b, 0);
    }

    #[test]
    fn from_hex_rgba() {
        let c = Color::from_hex(0xFF800080);
        let [r, g, b, a] = c.to_rgba8();
        assert_eq!(r, 255);
        assert_eq!(g, 128);
        assert_eq!(b, 0);
        assert_eq!(a, 128);
    }

    #[test]
    fn with_alpha() {
        let c = Color::RED.with_alpha(0.5);
        assert_eq!(c.r, 1.0);
        assert!((c.a - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn lerp_midpoint() {
        let mid = Color::BLACK.lerp(Color::WHITE, 0.5);
        assert!((mid.r - 0.5).abs() < f32::EPSILON);
        assert!((mid.g - 0.5).abs() < f32::EPSILON);
        assert!((mid.b - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn opaque_and_transparent() {
        assert!(Color::RED.is_opaque());
        assert!(!Color::RED.is_transparent());
        assert!(Color::TRANSPARENT.is_transparent());
        assert!(!Color::TRANSPARENT.is_opaque());
    }
}
