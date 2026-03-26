//! Scrollbar theme — platform colors and timing.
//!
//! Chrome: `ui::ColorProvider` + `ScrollbarThemeOverlay` + `overlay_scrollbar_constants`.

use kozan_primitives::color::Color;

use super::scrollbar_layer::ScrollbarState;

/// Chrome: `ui/native_theme/overlay_scrollbar_constants`.
pub(crate) struct ScrollbarTheme {
    /// Chrome: `kColorOverlayScrollbarFill` — 71% alpha on foreground.
    pub fill: Color,
    /// Chrome: `kColorOverlayScrollbarFillHovered` — 86% alpha on foreground.
    pub fill_hovered: Color,
    /// Chrome: `kColorOverlayScrollbarFill` when pressed.
    pub fill_active: Color,
    /// Chrome: `ui::GetOverlayScrollbarFadeDelay()`.
    pub fade_delay_ms: u32,
    /// Chrome: `ui::GetOverlayScrollbarFadeDuration()`.
    pub fade_duration_ms: u32,
}

impl ScrollbarTheme {
    /// Chrome: dark mode — foreground is white, background is dark.
    /// Alpha values from `ui/color/ui_color_mixer.cc`:
    /// `kGoogleGreyAlpha700 = 0xB5 (71%)`, `kGoogleGreyAlpha800 = 0xDB (86%)`.
    const DARK: Self = Self {
        fill: Color::rgba(1.0, 1.0, 1.0, 0.71),
        fill_hovered: Color::rgba(1.0, 1.0, 1.0, 0.86),
        fill_active: Color::rgba(1.0, 1.0, 1.0, 0.86),
        fade_delay_ms: 500,
        fade_duration_ms: 200,
    };

    /// Chrome: light mode — foreground is black.
    #[allow(dead_code)]
    const LIGHT: Self = Self {
        fill: Color::rgba(0.0, 0.0, 0.0, 0.71),
        fill_hovered: Color::rgba(0.0, 0.0, 0.0, 0.86),
        fill_active: Color::rgba(0.0, 0.0, 0.0, 0.86),
        fade_delay_ms: 500,
        fade_duration_ms: 200,
    };

    pub(crate) fn get() -> &'static Self {
        // Future: select based on CSS `color-scheme` from the scroll container.
        &Self::DARK
    }

    pub(crate) fn thumb_color(&self, state: ScrollbarState) -> Color {
        match state {
            ScrollbarState::Idle => self.fill,
            ScrollbarState::Hovered => self.fill_hovered,
            ScrollbarState::Active => self.fill_active,
        }
    }

    pub(crate) fn fade_delay_secs(&self) -> f32 {
        self.fade_delay_ms as f32 / 1000.0
    }

    pub(crate) fn fade_duration_secs(&self) -> f32 {
        self.fade_duration_ms as f32 / 1000.0
    }
}
