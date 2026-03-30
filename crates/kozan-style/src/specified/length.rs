//! All CSS `<length>` units at the specified level.
//!
//! Categorized into sub-enums by resolution context:
//! - `AbsoluteLength`: fixed ratios to px (cm, mm, in, pt, pc)
//! - `FontRelativeLength`: needs font-size/metrics (em, rem, ch, ex, cap, ic, lh, rlh)
//! - `ViewportPercentageLength`: needs viewport size (vw, vh, vmin, vmax + s/l/d variants)
//! - `ContainerRelativeLength`: needs container query size (cqw, cqh, cqi, cqb)

use crate::context::ComputeContext;
use crate::computed;

/// Specified CSS `<length>` — preserves original unit.
///
/// Resolved to `computed::Length` (pure px) via `ToComputedValue`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Absolute(AbsoluteLength),
    FontRelative(FontRelativeLength),
    ViewportPercentage(ViewportPercentageLength),
    ContainerRelative(ContainerRelativeLength),
}

impl crate::ToComputedValue for Length {
    type ComputedValue = computed::Length;

    fn to_computed_value(&self, ctx: &ComputeContext) -> computed::Length {
        match self {
            Self::Absolute(l) => l.to_computed_value(ctx),
            Self::FontRelative(l) => l.to_computed_value(ctx),
            Self::ViewportPercentage(l) => l.to_computed_value(ctx),
            Self::ContainerRelative(l) => l.to_computed_value(ctx),
        }
    }

    fn from_computed_value(computed: &computed::Length) -> Self {
        Self::Absolute(AbsoluteLength::Px(computed.px()))
    }
}

// https://developer.mozilla.org/en-US/docs/Web/CSS/length#absolute_length_units

const PX_PER_IN: f32 = 96.0;
const PX_PER_CM: f32 = PX_PER_IN / 2.54;
const PX_PER_MM: f32 = PX_PER_IN / 25.4;
const PX_PER_Q: f32 = PX_PER_MM / 4.0;
const PX_PER_PT: f32 = PX_PER_IN / 72.0;
const PX_PER_PC: f32 = PX_PER_PT * 12.0;

/// CSS absolute length units — fixed ratio to pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AbsoluteLength {
    /// Pixels. 1px = 1/96th of 1in.
    Px(f32),
    /// Centimeters. 1cm = 96px / 2.54.
    Cm(f32),
    /// Millimeters. 1mm = 1/10th of 1cm.
    Mm(f32),
    /// Quarter-millimeters. 1Q = 1/4th of 1mm.
    Q(f32),
    /// Inches. 1in = 96px.
    In(f32),
    /// Points. 1pt = 1/72nd of 1in.
    Pt(f32),
    /// Picas. 1pc = 12pt.
    Pc(f32),
}

impl AbsoluteLength {
    /// Converts to CSS pixels using the standard ratios.
    pub fn to_px(self) -> f32 {
        match self {
            Self::Px(v) => v,
            Self::Cm(v) => v * PX_PER_CM,
            Self::Mm(v) => v * PX_PER_MM,
            Self::Q(v) => v * PX_PER_Q,
            Self::In(v) => v * PX_PER_IN,
            Self::Pt(v) => v * PX_PER_PT,
            Self::Pc(v) => v * PX_PER_PC,
        }
    }

    /// Resolves to computed px applying zoom.
    pub fn to_computed_value(self, ctx: &ComputeContext) -> computed::Length {
        computed::Length::new(self.to_px() * ctx.zoom)
    }
}

// https://developer.mozilla.org/en-US/docs/Web/CSS/length#font-relative_length_units

/// CSS font-relative length units — resolved against font metrics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontRelativeLength {
    /// Relative to element's `font-size`.
    Em(f32),
    /// Relative to root element's `font-size`.
    Rem(f32),
    /// Width of "0" (U+0030) in element's font.
    Ch(f32),
    /// x-height of element's font.
    Ex(f32),
    /// Cap height of element's font.
    Cap(f32),
    /// Width of CJK water ideograph (U+6C34).
    Ic(f32),
    /// Line height of element.
    Lh(f32),
    /// Line height of root element.
    Rlh(f32),
    /// Cap height of root element's font.
    Rcap(f32),
    /// `ch` of root element.
    Rch(f32),
    /// `ex` of root element.
    Rex(f32),
    /// `ic` of root element.
    Ric(f32),
}

impl FontRelativeLength {
    /// Resolves to computed px using font-size and font metrics from context.
    /// Resolves font-relative lengths to computed px.
    ///
    /// When real font metrics are unavailable (e.g., before font load), the
    /// CSS Values Level 4 spec defines these fallback values:
    ///
    /// > If the computed value of `font-size` is used as a fallback for `ch`,
    /// > `ex`, or other font-relative units when the font is unavailable, the
    /// > fallback value is 0.5 × font-size for `ch`/`ex`, 0.7 × font-size for
    /// > `cap`, and 1 × font-size for `ic`.
    ///
    /// Spec: CSS Values and Units Level 4 § 6.1 — Relative lengths
    /// <https://www.w3.org/TR/css-values-4/#font-relative-lengths>
    pub fn to_computed_value(self, ctx: &ComputeContext) -> computed::Length {
        let px = match self {
            Self::Em(v) => v * ctx.font_size,
            Self::Rem(v) => v * ctx.root_font_size,
            // Fallback: 0.5 × font-size (spec §6.1)
            Self::Ch(v) => v * ctx.font_metrics.map_or(ctx.font_size * 0.5, |m| m.zero_advance),
            Self::Ex(v) => v * ctx.font_metrics.map_or(ctx.font_size * 0.5, |m| m.x_height),
            // Fallback: 0.7 × font-size (spec §6.1)
            Self::Cap(v) => v * ctx.font_metrics.map_or(ctx.font_size * 0.7, |m| m.cap_height),
            // Fallback: 1 × font-size (spec §6.1)
            Self::Ic(v) => v * ctx.font_metrics.map_or(ctx.font_size, |m| m.ic_width),
            Self::Lh(v) => v * ctx.line_height,
            Self::Rlh(v) => v * ctx.root_line_height,
            Self::Rcap(v) => v * ctx.root_font_metrics.map_or(ctx.root_font_size * 0.7, |m| m.cap_height),
            Self::Rch(v) => v * ctx.root_font_metrics.map_or(ctx.root_font_size * 0.5, |m| m.zero_advance),
            Self::Rex(v) => v * ctx.root_font_metrics.map_or(ctx.root_font_size * 0.5, |m| m.x_height),
            Self::Ric(v) => v * ctx.root_font_metrics.map_or(ctx.root_font_size, |m| m.ic_width),
        };
        computed::Length::new(px * ctx.zoom)
    }
}

// https://developer.mozilla.org/en-US/docs/Web/CSS/length#viewport-percentage_lengths

/// CSS viewport-percentage length units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewportPercentageLength {
    // Default viewport (UA default)
    Vw(f32), Vh(f32), Vmin(f32), Vmax(f32), Vi(f32), Vb(f32),
    // Small viewport
    Svw(f32), Svh(f32), Svmin(f32), Svmax(f32), Svi(f32), Svb(f32),
    // Large viewport
    Lvw(f32), Lvh(f32), Lvmin(f32), Lvmax(f32), Lvi(f32), Lvb(f32),
    // Dynamic viewport
    Dvw(f32), Dvh(f32), Dvmin(f32), Dvmax(f32), Dvi(f32), Dvb(f32),
}

impl ViewportPercentageLength {
    /// Resolves to computed px using viewport dimensions from context.
    pub fn to_computed_value(self, ctx: &ComputeContext) -> computed::Length {
        let (size, factor) = match self {
            // Default viewport
            Self::Vw(v) => (ctx.viewport_width, v),
            Self::Vh(v) => (ctx.viewport_height, v),
            Self::Vmin(v) => (ctx.viewport_width.min(ctx.viewport_height), v),
            Self::Vmax(v) => (ctx.viewport_width.max(ctx.viewport_height), v),
            // vi/vb: inline/block axis depends on writing-mode.
            // horizontal-tb: inline=width, block=height
            // vertical-*:    inline=height, block=width
            Self::Vi(v) => (if ctx.horizontal_writing_mode { ctx.viewport_width } else { ctx.viewport_height }, v),
            Self::Vb(v) => (if ctx.horizontal_writing_mode { ctx.viewport_height } else { ctx.viewport_width }, v),

            // Small viewport
            Self::Svw(v) => (ctx.small_viewport.width, v),
            Self::Svh(v) => (ctx.small_viewport.height, v),
            Self::Svmin(v) => (ctx.small_viewport.width.min(ctx.small_viewport.height), v),
            Self::Svmax(v) => (ctx.small_viewport.width.max(ctx.small_viewport.height), v),
            Self::Svi(v) => (if ctx.horizontal_writing_mode { ctx.small_viewport.width } else { ctx.small_viewport.height }, v),
            Self::Svb(v) => (if ctx.horizontal_writing_mode { ctx.small_viewport.height } else { ctx.small_viewport.width }, v),

            // Large viewport
            Self::Lvw(v) => (ctx.large_viewport.width, v),
            Self::Lvh(v) => (ctx.large_viewport.height, v),
            Self::Lvmin(v) => (ctx.large_viewport.width.min(ctx.large_viewport.height), v),
            Self::Lvmax(v) => (ctx.large_viewport.width.max(ctx.large_viewport.height), v),
            Self::Lvi(v) => (if ctx.horizontal_writing_mode { ctx.large_viewport.width } else { ctx.large_viewport.height }, v),
            Self::Lvb(v) => (if ctx.horizontal_writing_mode { ctx.large_viewport.height } else { ctx.large_viewport.width }, v),

            // Dynamic viewport
            Self::Dvw(v) => (ctx.dynamic_viewport.width, v),
            Self::Dvh(v) => (ctx.dynamic_viewport.height, v),
            Self::Dvmin(v) => (ctx.dynamic_viewport.width.min(ctx.dynamic_viewport.height), v),
            Self::Dvmax(v) => (ctx.dynamic_viewport.width.max(ctx.dynamic_viewport.height), v),
            Self::Dvi(v) => (if ctx.horizontal_writing_mode { ctx.dynamic_viewport.width } else { ctx.dynamic_viewport.height }, v),
            Self::Dvb(v) => (if ctx.horizontal_writing_mode { ctx.dynamic_viewport.height } else { ctx.dynamic_viewport.width }, v),
        };

        computed::Length::new(size * factor / 100.0)
    }
}

// https://developer.mozilla.org/en-US/docs/Web/CSS/length#container_query_length_units

/// CSS container query length units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContainerRelativeLength {
    /// 1% of query container's width.
    Cqw(f32),
    /// 1% of query container's height.
    Cqh(f32),
    /// 1% of query container's inline size.
    Cqi(f32),
    /// 1% of query container's block size.
    Cqb(f32),
    /// Smaller of cqi and cqb.
    Cqmin(f32),
    /// Larger of cqi and cqb.
    Cqmax(f32),
}

impl ContainerRelativeLength {
    /// Resolves to computed px using container query sizes from context.
    pub fn to_computed_value(self, ctx: &ComputeContext) -> computed::Length {
        let container = ctx.container_size.copied().unwrap_or_default();
        let px = match self {
            Self::Cqw(v) => container.width * v / 100.0,
            Self::Cqh(v) => container.height * v / 100.0,
            Self::Cqi(v) => container.inline_size * v / 100.0,
            Self::Cqb(v) => container.block_size * v / 100.0,
            Self::Cqmin(v) => container.inline_size.min(container.block_size) * v / 100.0,
            Self::Cqmax(v) => container.inline_size.max(container.block_size) * v / 100.0,
        };
        computed::Length::new(px)
    }
}
impl core::fmt::Display for Length {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Absolute(l) => write!(f, "{l}"),
            Self::FontRelative(l) => write!(f, "{l}"),
            Self::ViewportPercentage(l) => write!(f, "{l}"),
            Self::ContainerRelative(l) => write!(f, "{l}"),
        }
    }
}

impl core::fmt::Display for AbsoluteLength {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Px(v) => write!(f, "{v}px"),
            Self::Cm(v) => write!(f, "{v}cm"),
            Self::Mm(v) => write!(f, "{v}mm"),
            Self::Q(v) => write!(f, "{v}Q"),
            Self::In(v) => write!(f, "{v}in"),
            Self::Pt(v) => write!(f, "{v}pt"),
            Self::Pc(v) => write!(f, "{v}pc"),
        }
    }
}

impl core::fmt::Display for FontRelativeLength {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Em(v) => write!(f, "{v}em"),
            Self::Rem(v) => write!(f, "{v}rem"),
            Self::Ch(v) => write!(f, "{v}ch"),
            Self::Ex(v) => write!(f, "{v}ex"),
            Self::Cap(v) => write!(f, "{v}cap"),
            Self::Ic(v) => write!(f, "{v}ic"),
            Self::Lh(v) => write!(f, "{v}lh"),
            Self::Rlh(v) => write!(f, "{v}rlh"),
            Self::Rcap(v) => write!(f, "{v}rcap"),
            Self::Rch(v) => write!(f, "{v}rch"),
            Self::Rex(v) => write!(f, "{v}rex"),
            Self::Ric(v) => write!(f, "{v}ric"),
        }
    }
}

impl core::fmt::Display for ViewportPercentageLength {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Vw(v) => write!(f, "{v}vw"), Self::Vh(v) => write!(f, "{v}vh"),
            Self::Vmin(v) => write!(f, "{v}vmin"), Self::Vmax(v) => write!(f, "{v}vmax"),
            Self::Vi(v) => write!(f, "{v}vi"), Self::Vb(v) => write!(f, "{v}vb"),
            Self::Svw(v) => write!(f, "{v}svw"), Self::Svh(v) => write!(f, "{v}svh"),
            Self::Svmin(v) => write!(f, "{v}svmin"), Self::Svmax(v) => write!(f, "{v}svmax"),
            Self::Svi(v) => write!(f, "{v}svi"), Self::Svb(v) => write!(f, "{v}svb"),
            Self::Lvw(v) => write!(f, "{v}lvw"), Self::Lvh(v) => write!(f, "{v}lvh"),
            Self::Lvmin(v) => write!(f, "{v}lvmin"), Self::Lvmax(v) => write!(f, "{v}lvmax"),
            Self::Lvi(v) => write!(f, "{v}lvi"), Self::Lvb(v) => write!(f, "{v}lvb"),
            Self::Dvw(v) => write!(f, "{v}dvw"), Self::Dvh(v) => write!(f, "{v}dvh"),
            Self::Dvmin(v) => write!(f, "{v}dvmin"), Self::Dvmax(v) => write!(f, "{v}dvmax"),
            Self::Dvi(v) => write!(f, "{v}dvi"), Self::Dvb(v) => write!(f, "{v}dvb"),
        }
    }
}

impl core::fmt::Display for ContainerRelativeLength {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cqw(v) => write!(f, "{v}cqw"),
            Self::Cqh(v) => write!(f, "{v}cqh"),
            Self::Cqi(v) => write!(f, "{v}cqi"),
            Self::Cqb(v) => write!(f, "{v}cqb"),
            Self::Cqmin(v) => write!(f, "{v}cqmin"),
            Self::Cqmax(v) => write!(f, "{v}cqmax"),
        }
    }
}
/// Creates a length in CSS pixels.
pub fn px(v: f32) -> Length { Length::Absolute(AbsoluteLength::Px(v)) }
/// Creates a length in centimeters.
pub fn cm(v: f32) -> Length { Length::Absolute(AbsoluteLength::Cm(v)) }
/// Creates a length in millimeters.
pub fn mm(v: f32) -> Length { Length::Absolute(AbsoluteLength::Mm(v)) }
/// Creates a length in quarter-millimeters.
pub fn q(v: f32) -> Length { Length::Absolute(AbsoluteLength::Q(v)) }
/// Creates a length in inches.
pub fn inches(v: f32) -> Length { Length::Absolute(AbsoluteLength::In(v)) }
/// Creates a length in points (1/72 inch).
pub fn pt(v: f32) -> Length { Length::Absolute(AbsoluteLength::Pt(v)) }
/// Creates a length in picas (12 points).
pub fn pc(v: f32) -> Length { Length::Absolute(AbsoluteLength::Pc(v)) }
/// Creates a length relative to the element's font-size.
pub fn em(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Em(v)) }
/// Creates a length relative to the root font-size.
pub fn rem(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Rem(v)) }
/// Creates a length relative to the "0" glyph width.
pub fn ch(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Ch(v)) }
/// Creates a length relative to the x-height.
pub fn ex(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Ex(v)) }
/// Creates a length relative to the cap height.
pub fn cap(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Cap(v)) }
/// Creates a length relative to the CJK water ideograph width.
pub fn ic(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Ic(v)) }
/// Creates a length relative to the element's line-height.
pub fn lh(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Lh(v)) }
/// Creates a length relative to the root line-height.
pub fn rlh(v: f32) -> Length { Length::FontRelative(FontRelativeLength::Rlh(v)) }
/// Creates a length as a percentage of viewport width.
pub fn vw(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Vw(v)) }
/// Creates a length as a percentage of viewport height.
pub fn vh(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Vh(v)) }
/// Creates a length as a percentage of the smaller viewport dimension.
pub fn vmin(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Vmin(v)) }
/// Creates a length as a percentage of the larger viewport dimension.
pub fn vmax(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Vmax(v)) }
/// Creates a length as a percentage of dynamic viewport width.
pub fn dvw(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Dvw(v)) }
/// Creates a length as a percentage of dynamic viewport height.
pub fn dvh(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Dvh(v)) }
/// Creates a length as a percentage of large viewport width.
pub fn lvw(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Lvw(v)) }
/// Creates a length as a percentage of large viewport height.
pub fn lvh(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Lvh(v)) }
/// Creates a length as a percentage of small viewport width.
pub fn svw(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Svw(v)) }
/// Creates a length as a percentage of small viewport height.
pub fn svh(v: f32) -> Length { Length::ViewportPercentage(ViewportPercentageLength::Svh(v)) }
/// Creates a length as a percentage of container query width.
pub fn cqw(v: f32) -> Length { Length::ContainerRelative(ContainerRelativeLength::Cqw(v)) }
/// Creates a length as a percentage of container query height.
pub fn cqh(v: f32) -> Length { Length::ContainerRelative(ContainerRelativeLength::Cqh(v)) }
/// Creates a length as a percentage of container query inline size.
pub fn cqi(v: f32) -> Length { Length::ContainerRelative(ContainerRelativeLength::Cqi(v)) }
/// Creates a length as a percentage of container query block size.
pub fn cqb(v: f32) -> Length { Length::ContainerRelative(ContainerRelativeLength::Cqb(v)) }
/// Creates a percentage from a human-readable value (50.0 = 50%).
pub fn percent(v: f32) -> crate::computed::Percentage { crate::computed::Percentage::new(v / 100.0) }
