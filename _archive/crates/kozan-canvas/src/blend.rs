//! Blend modes for canvas compositing operations.
//!
//! Chrome equivalent: `SkBlendMode` / Canvas `globalCompositeOperation`.

/// Compositing and blending mode.
///
/// Chrome equivalent: `SkBlendMode` mapped from `globalCompositeOperation`.
/// Covers the full Canvas 2D spec set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    SourceOver,
    SourceIn,
    SourceOut,
    SourceAtop,
    DestinationOver,
    DestinationIn,
    DestinationOut,
    DestinationAtop,
    Lighter,
    Copy,
    Xor,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}
