//! Scrollbar fade animation — compositor-side opacity transitions.
//!
//! Chrome: `cc::ScrollbarAnimationController`.
//! Opacity is state-driven: Hovered/Active = 1.0, Idle = animated fade.

use std::time::Instant;

use super::scrollbar_layer::ScrollbarState;
use super::scrollbar_theme::ScrollbarTheme;

enum FadePhase {
    Hidden,
    Visible,
    WaitingToFade { since: Instant },
}

/// Chrome: `cc::ScrollbarAnimationController`.
pub(crate) struct ScrollbarAnimation {
    fade: FadePhase,
    state: ScrollbarState,
}

impl ScrollbarAnimation {
    pub(crate) fn new() -> Self {
        Self {
            fade: FadePhase::Hidden,
            state: ScrollbarState::Idle,
        }
    }

    pub(crate) fn set_state(&mut self, state: ScrollbarState) {
        let prev = self.state;
        self.state = state;

        match (prev, state) {
            (_, ScrollbarState::Hovered | ScrollbarState::Active) => {
                self.fade = FadePhase::Visible;
            }
            (ScrollbarState::Hovered | ScrollbarState::Active, ScrollbarState::Idle) => {
                self.fade = FadePhase::WaitingToFade {
                    since: Instant::now(),
                };
            }
            _ => {}
        }
    }

    pub(crate) fn on_scroll(&mut self) {
        if self.state == ScrollbarState::Idle {
            self.fade = FadePhase::WaitingToFade {
                since: Instant::now(),
            };
        }
    }

    pub(crate) fn opacity(&self, theme: &ScrollbarTheme) -> f32 {
        match self.state {
            ScrollbarState::Hovered | ScrollbarState::Active => 1.0,
            ScrollbarState::Idle => self.idle_opacity(theme),
        }
    }

    fn idle_opacity(&self, theme: &ScrollbarTheme) -> f32 {
        let now = Instant::now();
        match self.fade {
            FadePhase::Hidden => 0.0,
            FadePhase::Visible => 1.0,
            FadePhase::WaitingToFade { since } => {
                let elapsed = now.duration_since(since).as_secs_f32();
                if elapsed < theme.fade_delay_secs() {
                    1.0
                } else {
                    let t = (elapsed - theme.fade_delay_secs()) / theme.fade_duration_secs();
                    (1.0 - t).max(0.0)
                }
            }
        }
    }

    pub(crate) fn is_animating(&self) -> bool {
        matches!(
            self.fade,
            FadePhase::WaitingToFade { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_hidden() {
        let anim = ScrollbarAnimation::new();
        let theme = ScrollbarTheme::get();
        assert_eq!(anim.opacity(theme), 0.0);
    }

    #[test]
    fn scroll_shows() {
        let mut anim = ScrollbarAnimation::new();
        anim.on_scroll();
        let theme = ScrollbarTheme::get();
        assert_eq!(anim.opacity(theme), 1.0);
    }

    #[test]
    fn hover_always_visible() {
        let mut anim = ScrollbarAnimation::new();
        anim.set_state(ScrollbarState::Hovered);
        let theme = ScrollbarTheme::get();
        assert_eq!(anim.opacity(theme), 1.0);
    }

    #[test]
    fn active_always_visible() {
        let mut anim = ScrollbarAnimation::new();
        anim.set_state(ScrollbarState::Active);
        let theme = ScrollbarTheme::get();
        assert_eq!(anim.opacity(theme), 1.0);
    }

    #[test]
    fn scroll_during_drag_stays_visible() {
        let mut anim = ScrollbarAnimation::new();
        anim.set_state(ScrollbarState::Active);
        anim.on_scroll();
        let theme = ScrollbarTheme::get();
        assert_eq!(anim.opacity(theme), 1.0);
    }
}
