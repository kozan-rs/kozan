//! CSS color system with full color space support.
//!
//! Colors stay in their original color space throughout the pipeline.
//! Conversion to device space happens at paint time, preserving
//! wide-gamut data for modern displays.

/// Identifies a CSS color space.
///
/// Determines how the three components of an [`AbsoluteColor`] are interpreted.
/// Each color space has different semantics for its component values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ColorSpace {
    /// sRGB: `[r, g, b]` in 0.0..1.0. The default web color space.
    #[default]
    Srgb,
    /// HSL: `[hue (0..360), saturation (0..1), lightness (0..1)]`.
    Hsl,
    /// HWB: `[hue (0..360), whiteness (0..1), blackness (0..1)]`.
    Hwb,
    /// CIE Lab: `[L (0..100), a (-125..125), b (-125..125)]`.
    Lab,
    /// CIE LCH: `[L (0..100), C (0..150), h (0..360)]`.
    Lch,
    /// OK Lab: `[L (0..1), a (-0.4..0.4), b (-0.4..0.4)]`. Perceptually uniform.
    Oklab,
    /// OK LCH: `[L (0..1), C (0..0.4), h (0..360)]`. Perceptually uniform polar.
    Oklch,
    /// Linear sRGB: `[r, g, b]` in 0.0..1.0. Linear-light version of sRGB.
    SrgbLinear,
    /// Display P3: `[r, g, b]` in 0.0..1.0. Wide-gamut space for Apple displays.
    DisplayP3,
    /// Adobe RGB (1998): `[r, g, b]` in 0.0..1.0.
    A98Rgb,
    /// ProPhoto RGB: `[r, g, b]` in 0.0..1.0. Very wide gamut.
    ProphotoRgb,
    /// ITU-R BT.2020: `[r, g, b]` in 0.0..1.0. HDR/wide-gamut broadcast.
    Rec2020,
    /// CIE XYZ with D50 illuminant: `[x, y, z]`.
    XyzD50,
    /// CIE XYZ with D65 illuminant: `[x, y, z]`.
    XyzD65,
}

/// A resolved color in any color space.
///
/// Components are interpreted according to [`ColorSpace`].
/// Alpha is always 0.0..1.0 regardless of space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbsoluteColor {
    /// Three color components. Meaning depends on `color_space`.
    pub components: [f32; 3],
    /// Alpha channel (0.0 = transparent, 1.0 = opaque).
    pub alpha: f32,
    /// Which color space the components are in.
    pub color_space: ColorSpace,
}

impl AbsoluteColor {
    /// Transparent black in sRGB.
    pub const TRANSPARENT: Self = Self::srgb(0.0, 0.0, 0.0, 0.0);
    /// Opaque black in sRGB.
    pub const BLACK: Self = Self::srgb(0.0, 0.0, 0.0, 1.0);
    /// Opaque white in sRGB.
    pub const WHITE: Self = Self::srgb(1.0, 1.0, 1.0, 1.0);

    /// Creates an sRGB color from f32 components (0.0..1.0).
    pub const fn srgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { components: [r, g, b], alpha: a, color_space: ColorSpace::Srgb }
    }

    /// Creates an opaque sRGB color.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::srgb(r, g, b, 1.0)
    }

    /// Creates an sRGB color from u8 components (0..255).
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
    }

    /// Creates an opaque sRGB color from a 24-bit hex value (0xRRGGBB).
    pub fn from_hex(hex: u32) -> Self {
        Self::from_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
    }

    /// Creates an OKLCh color (perceptually uniform, wide-gamut safe).
    pub const fn oklch(l: f32, c: f32, h: f32, a: f32) -> Self {
        Self { components: [l, c, h], alpha: a, color_space: ColorSpace::Oklch }
    }

    /// Creates an OKLab color (perceptually uniform).
    pub const fn oklab(l: f32, a_axis: f32, b_axis: f32, alpha: f32) -> Self {
        Self { components: [l, a_axis, b_axis], alpha, color_space: ColorSpace::Oklab }
    }

    /// Creates a Display P3 color (wide-gamut, Apple displays).
    pub const fn display_p3(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { components: [r, g, b], alpha: a, color_space: ColorSpace::DisplayP3 }
    }

    /// Creates an HSL color.
    pub const fn hsl(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self { components: [h, s, l], alpha: a, color_space: ColorSpace::Hsl }
    }

    /// Creates a color in any color space.
    pub const fn new(c0: f32, c1: f32, c2: f32, alpha: f32, space: ColorSpace) -> Self {
        Self { components: [c0, c1, c2], alpha, color_space: space }
    }

    /// Converts to sRGB u8 array. Clamps to 0..255.
    pub fn to_u8(self) -> [u8; 4] {
        let [c0, c1, c2] = self.components;
        [
            (c0.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (c1.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (c2.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (self.alpha.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        ]
    }

    /// Packs sRGB to u32 as 0xRRGGBBAA.
    pub fn to_u32(self) -> u32 {
        let [r, g, b, a] = self.to_u8();
        (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | a as u32
    }

    /// Returns true if fully transparent.
    pub fn is_transparent(self) -> bool { self.alpha == 0.0 }

    /// Returns true if fully opaque.
    pub fn is_opaque(self) -> bool { self.alpha >= 1.0 }

    /// First component (r in sRGB, L in oklab/oklch, h in hsl).
    pub fn c0(&self) -> f32 { self.components[0] }

    /// Second component (g in sRGB, a in oklab, C in oklch, s in hsl).
    pub fn c1(&self) -> f32 { self.components[1] }

    /// Third component (b in sRGB, b in oklab, h in oklch, l in hsl).
    pub fn c2(&self) -> f32 { self.components[2] }

    /// Mixes this color with another at the given ratio (0.0 = self, 1.0 = other).
    pub fn mix(self, other: Self, ratio: f32) -> Self {
        let inv = 1.0 - ratio;
        Self {
            components: [
                self.components[0] * inv + other.components[0] * ratio,
                self.components[1] * inv + other.components[1] * ratio,
                self.components[2] * inv + other.components[2] * ratio,
            ],
            alpha: self.alpha * inv + other.alpha * ratio,
            color_space: self.color_space,
        }
    }

    /// Returns a copy with the given alpha.
    pub fn with_alpha(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }

    /// Writes hex representation to a `fmt::Write` destination.
    ///
    /// Outputs `#rrggbb` or `#rrggbbaa` if not fully opaque.
    /// Only meaningful for sRGB — other spaces are clamped to 0..1.
    pub fn write_hex(self, dest: &mut impl core::fmt::Write) -> core::fmt::Result {
        let [r, g, b, a] = self.to_u8();
        if a == 255 {
            write!(dest, "#{r:02x}{g:02x}{b:02x}")
        } else {
            write!(dest, "#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }

    /// Returns the hex string representation.
    pub fn to_hex_string(self) -> String {
        let mut s = String::with_capacity(9);
        self.write_hex(&mut s).unwrap();
        s
    }

    /// Lightens the color by the given amount (0.0..1.0).
    ///
    /// Linearly interpolates each component toward 1.0 (white).
    pub fn lighten(self, amount: f32) -> Self {
        let [c0, c1, c2] = self.components;
        Self {
            components: [
                c0 + (1.0 - c0) * amount,
                c1 + (1.0 - c1) * amount,
                c2 + (1.0 - c2) * amount,
            ],
            ..self
        }
    }

    /// Darkens the color by the given amount (0.0..1.0).
    ///
    /// Linearly interpolates each component toward 0.0 (black).
    pub fn darken(self, amount: f32) -> Self {
        let inv = 1.0 - amount;
        let [c0, c1, c2] = self.components;
        Self {
            components: [c0 * inv, c1 * inv, c2 * inv],
            ..self
        }
    }
}

impl Default for AbsoluteColor {
    fn default() -> Self { Self::BLACK }
}

impl crate::Animate for AbsoluteColor {
    fn animate(&self, other: &Self, procedure: crate::Procedure) -> Result<Self, ()> {
        let (wa, wb) = procedure.weights();
        Ok(Self {
            components: [
                (self.components[0] as f64 * wa + other.components[0] as f64 * wb) as f32,
                (self.components[1] as f64 * wa + other.components[1] as f64 * wb) as f32,
                (self.components[2] as f64 * wa + other.components[2] as f64 * wb) as f32,
            ],
            alpha: (self.alpha as f64 * wa + other.alpha as f64 * wb) as f32,
            color_space: self.color_space,
        })
    }
}

impl crate::ToAnimatedZero for AbsoluteColor {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Ok(Self { components: [0.0; 3], alpha: 0.0, color_space: self.color_space })
    }
}

impl crate::ComputeSquaredDistance for AbsoluteColor {
    fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()> {
        let d = |i: usize| (self.components[i] - other.components[i]) as f64;
        let da = (self.alpha - other.alpha) as f64;
        Ok(d(0) * d(0) + d(1) * d(1) + d(2) * d(2) + da * da)
    }
}

impl crate::Zero for AbsoluteColor {
    fn zero() -> Self { Self::TRANSPARENT }
    fn is_zero(&self) -> bool { self.is_transparent() }
}

impl core::fmt::Display for AbsoluteColor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [c0, c1, c2] = self.components;
        match self.color_space {
            ColorSpace::Srgb => {
                let [r, g, b, a] = self.to_u8();
                if a == 255 { write!(f, "rgb({r}, {g}, {b})") }
                else { write!(f, "rgba({r}, {g}, {b}, {:.2})", self.alpha) }
            }
            ColorSpace::Hsl => {
                if self.alpha >= 1.0 { write!(f, "hsl({c0}, {:.0}%, {:.0}%)", c1 * 100.0, c2 * 100.0) }
                else { write!(f, "hsla({c0}, {:.0}%, {:.0}%, {:.2})", c1 * 100.0, c2 * 100.0, self.alpha) }
            }
            ColorSpace::Oklch => write!(f, "oklch({c0:.3} {c1:.3} {c2:.1} / {:.2})", self.alpha),
            ColorSpace::Oklab => write!(f, "oklab({c0:.3} {c1:.4} {c2:.4} / {:.2})", self.alpha),
            ColorSpace::DisplayP3 => write!(f, "color(display-p3 {c0:.4} {c1:.4} {c2:.4} / {:.2})", self.alpha),
            _ => write!(f, "color({:?} {c0:.4} {c1:.4} {c2:.4} / {:.2})", self.color_space, self.alpha),
        }
    }
}

/// CSS system color keyword (platform-dependent UI colors).
///
/// Resolved to [`AbsoluteColor`] based on the active color scheme.
/// See <https://developer.mozilla.org/en-US/docs/Web/CSS/system-color>.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SystemColor {
    /// Background of application content or documents.
    Canvas,
    /// Text in application content or documents.
    CanvasText,
    /// Text in non-active, non-visited links.
    LinkText,
    /// Text in visited links.
    VisitedText,
    /// Text in active links.
    ActiveText,
    /// Background of push buttons.
    ButtonFace,
    /// Text of push buttons.
    ButtonText,
    /// Border of push buttons.
    ButtonBorder,
    /// Background of input fields.
    Field,
    /// Text in input fields.
    FieldText,
    /// Background of selected items.
    Highlight,
    /// Text of selected items.
    HighlightText,
    /// Background of selected items in non-active contexts.
    SelectedItem,
    /// Text of selected items in non-active contexts.
    SelectedItemText,
    /// Background of text highlighted by find-in-page.
    Mark,
    /// Text of find-in-page highlights.
    MarkText,
    /// Disabled text.
    GrayText,
    /// Platform accent color.
    AccentColor,
    /// Text on platform accent color.
    AccentColorText,
}

impl SystemColor {
    /// Resolves to an absolute sRGB color based on the color scheme.
    pub fn resolve(self, dark: bool) -> AbsoluteColor {
        if dark { self.dark() } else { self.light() }
    }

    fn light(self) -> AbsoluteColor {
        match self {
            Self::Canvas | Self::Field => AbsoluteColor::WHITE,
            Self::CanvasText | Self::ButtonText | Self::FieldText
                | Self::MarkText => AbsoluteColor::BLACK,
            Self::LinkText => AbsoluteColor::from_hex(0x0000EE),
            Self::VisitedText => AbsoluteColor::from_hex(0x551A8B),
            Self::ActiveText => AbsoluteColor::from_hex(0xFF0000),
            Self::ButtonFace => AbsoluteColor::from_hex(0xDDDDDD),
            Self::ButtonBorder | Self::GrayText => AbsoluteColor::from_hex(0x767676),
            Self::Highlight | Self::SelectedItem => AbsoluteColor::from_hex(0x3367D6),
            Self::HighlightText | Self::SelectedItemText
                | Self::AccentColorText => AbsoluteColor::WHITE,
            Self::Mark => AbsoluteColor::from_hex(0xFFFF00),
            Self::AccentColor => AbsoluteColor::from_hex(0x0078D4),
        }
    }

    fn dark(self) -> AbsoluteColor {
        match self {
            Self::Canvas => AbsoluteColor::from_hex(0x1E1E1E),
            Self::CanvasText | Self::ButtonText | Self::FieldText
                | Self::MarkText => AbsoluteColor::WHITE,
            Self::LinkText => AbsoluteColor::from_hex(0x9E9EFF),
            Self::VisitedText => AbsoluteColor::from_hex(0xD0ADF0),
            Self::ActiveText => AbsoluteColor::from_hex(0xFF6666),
            Self::ButtonFace => AbsoluteColor::from_hex(0x444444),
            Self::ButtonBorder | Self::GrayText => AbsoluteColor::from_hex(0x8C8C8C),
            Self::Field => AbsoluteColor::from_hex(0x333333),
            Self::Highlight | Self::SelectedItem => AbsoluteColor::from_hex(0x6699FF),
            Self::HighlightText | Self::SelectedItemText
                | Self::AccentColorText => AbsoluteColor::BLACK,
            Self::Mark => AbsoluteColor::from_hex(0x888800),
            Self::AccentColor => AbsoluteColor::from_hex(0x60AAFF),
        }
    }
}

impl core::fmt::Display for SystemColor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

/// CSS `<color>` at specified level.
///
/// Preserves the original color representation (keywords, functions, etc.).
/// Resolved to [`ComputedColor`] or [`AbsoluteColor`] during cascade.
#[derive(Clone, Debug, PartialEq)]
pub enum Color {
    /// A resolved color in any color space.
    Absolute(AbsoluteColor),
    /// The `currentColor` keyword.
    CurrentColor,
    /// A CSS system color keyword.
    System(SystemColor),
    /// `color-mix(in <space>, <color> <pct>, <color> <pct>)`.
    ColorMix(Box<ColorMix>),
    /// `light-dark(<light>, <dark>)`.
    LightDark(Box<Color>, Box<Color>),
}

/// Arguments to `color-mix()`.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorMix {
    /// The color space to interpolate in.
    pub space: ColorSpace,
    /// Left color operand.
    pub left: Color,
    /// Left color percentage (0.0..1.0).
    pub left_pct: f32,
    /// Right color operand.
    pub right: Color,
    /// Right color percentage (0.0..1.0).
    pub right_pct: f32,
}

impl Color {
    /// Transparent black.
    pub const TRANSPARENT: Self = Self::Absolute(AbsoluteColor::TRANSPARENT);
    /// Opaque black.
    pub const BLACK: Self = Self::Absolute(AbsoluteColor::BLACK);
    /// Opaque white.
    pub const WHITE: Self = Self::Absolute(AbsoluteColor::WHITE);

    /// Creates an sRGB color from u8 RGBA components.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::Absolute(AbsoluteColor::srgb(
            r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0,
        ))
    }

    /// Creates an opaque sRGB color from u8 RGB.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self { Self::rgba(r, g, b, 255) }

    /// Creates an opaque sRGB color from hex.
    pub fn from_hex(hex: u32) -> Self { Self::Absolute(AbsoluteColor::from_hex(hex)) }

    /// Creates a `color-mix()` value.
    pub fn color_mix(space: ColorSpace, left: Color, left_pct: f32, right: Color, right_pct: f32) -> Self {
        Self::ColorMix(Box::new(ColorMix { space, left, left_pct, right, right_pct }))
    }

    /// Creates a `light-dark()` value.
    pub fn light_dark(light: Color, dark: Color) -> Self {
        Self::LightDark(Box::new(light), Box::new(dark))
    }

    /// Returns true if this is the `currentColor` keyword.
    pub const fn is_current_color(&self) -> bool { matches!(self, Self::CurrentColor) }
}

impl Default for Color { fn default() -> Self { Self::BLACK } }
impl From<AbsoluteColor> for Color { fn from(c: AbsoluteColor) -> Self { Self::Absolute(c) } }
impl From<SystemColor> for Color { fn from(c: SystemColor) -> Self { Self::System(c) } }

impl core::fmt::Display for Color {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Absolute(c) => write!(f, "{c}"),
            Self::CurrentColor => f.write_str("currentcolor"),
            Self::System(s) => write!(f, "{s}"),
            Self::ColorMix(m) => write!(
                f, "color-mix(in {:?}, {} {:.0}%, {} {:.0}%)",
                m.space, m.left, m.left_pct * 100.0, m.right, m.right_pct * 100.0,
            ),
            Self::LightDark(l, d) => write!(f, "light-dark({l}, {d})"),
        }
    }
}

/// Wrapper for the CSS `color` property.
///
/// Resolves `currentColor` to the inherited color at computed time.
/// Other color properties use [`Color`] directly (computes to [`ComputedColor`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorProperty(pub Color);

impl Default for ColorProperty { fn default() -> Self { Self(Color::BLACK) } }

impl crate::ToComputedValue for ColorProperty {
    type ComputedValue = AbsoluteColor;

    fn to_computed_value(&self, ctx: &crate::ComputeContext) -> AbsoluteColor {
        resolve_to_absolute(&self.0, ctx, true)
    }

    fn from_computed_value(computed: &AbsoluteColor) -> Self {
        Self(Color::Absolute(*computed))
    }
}

impl core::fmt::Display for ColorProperty {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { write!(f, "{}", self.0) }
}

impl crate::IntoDeclared<ColorProperty> for ColorProperty {
    fn into_declared(self) -> crate::Declared<ColorProperty> { crate::Declared::Value(self) }
}
impl crate::IntoDeclared<ColorProperty> for Color {
    fn into_declared(self) -> crate::Declared<ColorProperty> { crate::Declared::Value(ColorProperty(self)) }
}
impl crate::IntoDeclared<ColorProperty> for AbsoluteColor {
    fn into_declared(self) -> crate::Declared<ColorProperty> { crate::Declared::Value(ColorProperty(Color::Absolute(self))) }
}
impl crate::IntoDeclared<ColorProperty> for SystemColor {
    fn into_declared(self) -> crate::Declared<ColorProperty> { crate::Declared::Value(ColorProperty(Color::System(self))) }
}

/// Computed `<color>` — may keep `currentColor` for paint-time resolution.
///
/// The `color` property resolves to [`AbsoluteColor`] at computed time.
/// Other properties (border-color, outline-color, etc.) keep `CurrentColor`
/// so `color` animations propagate without re-cascade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComputedColor {
    /// A fully resolved color.
    Absolute(AbsoluteColor),
    /// Resolved at paint time against the element's computed `color`.
    CurrentColor,
}

impl ComputedColor {
    /// Resolves for paint: replaces `currentColor` with the element's color value.
    pub fn resolve(self, current_color: AbsoluteColor) -> AbsoluteColor {
        match self {
            Self::Absolute(c) => c,
            Self::CurrentColor => current_color,
        }
    }
}

impl Default for ComputedColor { fn default() -> Self { Self::Absolute(AbsoluteColor::BLACK) } }
impl From<AbsoluteColor> for ComputedColor { fn from(c: AbsoluteColor) -> Self { Self::Absolute(c) } }

impl core::fmt::Display for ComputedColor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Absolute(c) => write!(f, "{c}"),
            Self::CurrentColor => f.write_str("currentcolor"),
        }
    }
}

impl crate::Animate for ComputedColor {
    fn animate(&self, other: &Self, procedure: crate::Procedure) -> Result<Self, ()> {
        match (self, other) {
            (Self::Absolute(a), Self::Absolute(b)) => Ok(Self::Absolute(a.animate(b, procedure)?)),
            (Self::CurrentColor, Self::CurrentColor) => Ok(Self::CurrentColor),
            _ => match procedure {
                crate::Procedure::Interpolate { progress } => Ok(if progress < 0.5 { *self } else { *other }),
                _ => Err(()),
            },
        }
    }
}

impl crate::ToAnimatedZero for ComputedColor {
    fn to_animated_zero(&self) -> Result<Self, ()> { Ok(Self::Absolute(AbsoluteColor::TRANSPARENT)) }
}

impl crate::ComputeSquaredDistance for ComputedColor {
    fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()> {
        match (self, other) {
            (Self::Absolute(a), Self::Absolute(b)) => a.compute_squared_distance(b),
            (Self::CurrentColor, Self::CurrentColor) => Ok(0.0),
            _ => Ok(1.0),
        }
    }
}

impl crate::ToComputedValue for Color {
    type ComputedValue = ComputedColor;

    fn to_computed_value(&self, ctx: &crate::ComputeContext) -> ComputedColor {
        match self {
            Self::CurrentColor => ComputedColor::CurrentColor,
            other => ComputedColor::Absolute(resolve_to_absolute(other, ctx, false)),
        }
    }

    fn from_computed_value(computed: &ComputedColor) -> Self {
        match computed {
            ComputedColor::Absolute(c) => Self::Absolute(*c),
            ComputedColor::CurrentColor => Self::CurrentColor,
        }
    }
}

/// Resolves a specified color to an absolute color using the cascade context.
fn resolve_to_absolute(color: &Color, ctx: &crate::ComputeContext, resolve_current: bool) -> AbsoluteColor {
    match color {
        Color::Absolute(c) => *c,
        Color::CurrentColor => {
            if resolve_current { ctx.inherited_color } else { AbsoluteColor::BLACK }
        }
        Color::System(s) => s.resolve(matches!(ctx.color_scheme, crate::ColorScheme::Dark)),
        Color::ColorMix(m) => {
            let left = resolve_to_absolute(&m.left, ctx, resolve_current);
            let right = resolve_to_absolute(&m.right, ctx, resolve_current);
            left.mix(right, m.right_pct)
        }
        Color::LightDark(light, dark) => {
            if matches!(ctx.color_scheme, crate::ColorScheme::Dark) {
                resolve_to_absolute(dark, ctx, resolve_current)
            } else {
                resolve_to_absolute(light, ctx, resolve_current)
            }
        }
    }
}
