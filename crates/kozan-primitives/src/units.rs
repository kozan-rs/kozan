/// A length value that may be absolute, relative, or intrinsic.
///
/// This is the building block of the style system — widths, heights,
/// margins, paddings, font sizes, and many other properties are all
/// expressed as `Dimension` values that get resolved to concrete pixels
/// during style resolution and layout.
///
/// Absolute values ([`Px`](Dimension::Px)) are ready to use immediately.
/// Relative values ([`Percent`](Dimension::Percent), [`Em`](Dimension::Em),
/// [`Rem`](Dimension::Rem)) need a reference value from the parent or root.
/// Viewport values ([`Vw`](Dimension::Vw), [`Vh`](Dimension::Vh)) need
/// the viewport size. Intrinsic values ([`Auto`](Dimension::Auto),
/// [`MinContent`](Dimension::MinContent), etc.) are resolved by the
/// layout algorithm itself.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Dimension {
    /// Absolute pixels (after DPI scaling).
    Px(f32),
    /// Percentage of the parent's corresponding dimension.
    Percent(f32),
    /// Relative to the element's computed `font-size`.
    Em(f32),
    /// Relative to the root element's computed `font-size`.
    Rem(f32),
    /// Percentage of the viewport width.
    Vw(f32),
    /// Percentage of the viewport height.
    Vh(f32),
    /// The smaller of `vw` and `vh`.
    Vmin(f32),
    /// The larger of `vw` and `vh`.
    Vmax(f32),
    /// Size determined by the layout algorithm.
    #[default]
    Auto,
    /// The smallest size that fits the content without overflow.
    MinContent,
    /// The largest size the content can fill without wrapping.
    MaxContent,
    /// Clamp between min-content and max-content, or the available
    /// space if it's between those bounds.
    FitContent,
}

impl Dimension {
    #[must_use] 
    pub fn is_auto(self) -> bool {
        matches!(self, Dimension::Auto)
    }

    /// True for any value that resolves to a concrete number (not
    /// auto/min-content/max-content/fit-content).
    #[must_use] 
    pub fn is_definite(self) -> bool {
        matches!(
            self,
            Dimension::Px(_)
                | Dimension::Percent(_)
                | Dimension::Em(_)
                | Dimension::Rem(_)
                | Dimension::Vw(_)
                | Dimension::Vh(_)
                | Dimension::Vmin(_)
                | Dimension::Vmax(_)
        )
    }

    /// True for values that the layout algorithm determines.
    #[must_use] 
    pub fn is_intrinsic(self) -> bool {
        matches!(
            self,
            Dimension::Auto | Dimension::MinContent | Dimension::MaxContent | Dimension::FitContent
        )
    }

    /// Resolve an absolute or parent-relative value to pixels.
    ///
    /// Only resolves [`Px`](Dimension::Px) and [`Percent`](Dimension::Percent).
    /// For font-relative and viewport-relative values, use
    /// [`resolve_full`](Self::resolve_full).
    #[must_use] 
    pub fn resolve(self, parent: f32) -> Option<f32> {
        match self {
            Dimension::Px(v) => Some(v),
            Dimension::Percent(pct) => Some(parent * pct / 100.0),
            _ => None,
        }
    }

    /// Resolve with all context values available.
    #[must_use] 
    pub fn resolve_full(self, ctx: &ResolveContext) -> Option<f32> {
        match self {
            Dimension::Px(v) => Some(v),
            Dimension::Percent(pct) => Some(ctx.parent * pct / 100.0),
            Dimension::Em(v) => Some(v * ctx.font_size),
            Dimension::Rem(v) => Some(v * ctx.root_font_size),
            Dimension::Vw(v) => Some(v * ctx.viewport_width / 100.0),
            Dimension::Vh(v) => Some(v * ctx.viewport_height / 100.0),
            Dimension::Vmin(v) => Some(v * ctx.viewport_width.min(ctx.viewport_height) / 100.0),
            Dimension::Vmax(v) => Some(v * ctx.viewport_width.max(ctx.viewport_height) / 100.0),
            Dimension::Auto
            | Dimension::MinContent
            | Dimension::MaxContent
            | Dimension::FitContent => None,
        }
    }

    /// Resolve, falling back to a default for unresolvable values.
    #[must_use] 
    pub fn resolve_or(self, parent: f32, fallback: f32) -> f32 {
        self.resolve(parent).unwrap_or(fallback)
    }
}

/// All the context needed to resolve any [`Dimension`] variant to pixels.
#[derive(Clone, Copy, Debug)]
pub struct ResolveContext {
    /// The parent element's resolved value for the same property.
    pub parent: f32,
    /// The element's computed font-size (for `em` units).
    pub font_size: f32,
    /// The root element's computed font-size (for `rem` units).
    pub root_font_size: f32,
    /// Viewport width in pixels.
    pub viewport_width: f32,
    /// Viewport height in pixels.
    pub viewport_height: f32,
}

/// Shorthand: absolute pixels.
#[must_use] 
pub fn px(value: f32) -> Dimension {
    Dimension::Px(value)
}

/// Shorthand: percentage of parent.
#[must_use] 
pub fn pct(value: f32) -> Dimension {
    Dimension::Percent(value)
}

/// Shorthand: relative to element's font-size.
#[must_use] 
pub fn em(value: f32) -> Dimension {
    Dimension::Em(value)
}

/// Shorthand: relative to root font-size.
#[must_use] 
pub fn rem(value: f32) -> Dimension {
    Dimension::Rem(value)
}

/// Shorthand: percentage of viewport width.
#[must_use] 
pub fn vw(value: f32) -> Dimension {
    Dimension::Vw(value)
}

/// Shorthand: percentage of viewport height.
#[must_use] 
pub fn vh(value: f32) -> Dimension {
    Dimension::Vh(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_resolves_to_itself() {
        assert_eq!(px(100.0).resolve(500.0), Some(100.0));
    }

    #[test]
    fn percent_resolves_against_parent() {
        assert_eq!(pct(50.0).resolve(200.0), Some(100.0));
    }

    #[test]
    fn auto_resolves_to_none() {
        assert_eq!(Dimension::Auto.resolve(500.0), None);
    }

    #[test]
    fn resolve_or_with_fallback() {
        assert_eq!(Dimension::Auto.resolve_or(500.0, 0.0), 0.0);
        assert_eq!(px(42.0).resolve_or(500.0, 0.0), 42.0);
    }

    #[test]
    fn default_is_auto() {
        assert_eq!(Dimension::default(), Dimension::Auto);
    }

    #[test]
    fn is_definite() {
        assert!(px(10.0).is_definite());
        assert!(pct(50.0).is_definite());
        assert!(em(1.5).is_definite());
        assert!(vw(100.0).is_definite());
        assert!(!Dimension::Auto.is_definite());
        assert!(!Dimension::MinContent.is_definite());
    }

    #[test]
    fn is_intrinsic() {
        assert!(Dimension::Auto.is_intrinsic());
        assert!(Dimension::MinContent.is_intrinsic());
        assert!(Dimension::MaxContent.is_intrinsic());
        assert!(Dimension::FitContent.is_intrinsic());
        assert!(!px(10.0).is_intrinsic());
    }

    #[test]
    fn resolve_full_em_and_rem() {
        let ctx = ResolveContext {
            parent: 500.0,
            font_size: 16.0,
            root_font_size: 18.0,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
        };

        assert_eq!(em(2.0).resolve_full(&ctx), Some(32.0));
        assert_eq!(rem(2.0).resolve_full(&ctx), Some(36.0));
    }

    #[test]
    fn resolve_full_viewport_units() {
        let ctx = ResolveContext {
            parent: 0.0,
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
        };

        assert_eq!(vw(50.0).resolve_full(&ctx), Some(960.0));
        assert_eq!(vh(100.0).resolve_full(&ctx), Some(1080.0));
        assert_eq!(Dimension::Vmin(50.0).resolve_full(&ctx), Some(540.0));
        assert_eq!(Dimension::Vmax(50.0).resolve_full(&ctx), Some(960.0));
    }

    #[test]
    fn intrinsic_values_dont_resolve() {
        let ctx = ResolveContext {
            parent: 500.0,
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
        };

        assert!(Dimension::Auto.resolve_full(&ctx).is_none());
        assert!(Dimension::MinContent.resolve_full(&ctx).is_none());
    }
}
