//! Media element trait — shared behavior for audio and video.
//!
//! Chrome equivalent: `HTMLMediaElement`.
//! Intermediate trait between `HtmlElement` and concrete media elements.
//!
//! # Chrome hierarchy
//!
//! ```text
//! HTMLElement
//!   └── HTMLMediaElement         ← THIS TRAIT
//!         ├── HTMLAudioElement
//!         └── HTMLVideoElement   ← also ReplacedElement
//! ```
//!
//! # What it provides
//!
//! - `src` / `set_src` — media source URL
//! - `autoplay`, `loop_playback`, `muted`, `controls` — boolean attributes
//! - `preload` — loading hint
//! - Playback API stubs (play, pause, `current_time` — future)

use super::html_element::HtmlElement;

/// Media element ready states.
///
/// Chrome equivalent: `HTMLMediaElement::ReadyState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaReadyState {
    /// No information about the media resource.
    #[default]
    HaveNothing = 0,
    /// Enough data to know duration and dimensions.
    HaveMetadata = 1,
    /// Data for the current playback position, but not enough for playback.
    HaveCurrentData = 2,
    /// Enough data to play at current position for a short while.
    HaveFutureData = 3,
    /// Enough data to play through to the end without buffering.
    HaveEnoughData = 4,
}

/// Media element network states.
///
/// Chrome equivalent: `HTMLMediaElement::NetworkState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaNetworkState {
    /// No network activity.
    #[default]
    Empty = 0,
    /// Fetching idle (no current fetch).
    Idle = 1,
    /// Actively downloading media data.
    Loading = 2,
    /// No suitable source found.
    NoSource = 3,
}

/// Shared behavior for media elements (audio, video).
///
/// Chrome equivalent: `HTMLMediaElement`.
///
/// # Unimplemented — requires media backend
///
/// `play()`, `pause()`, `current_time()`, `duration()`, and `paused()` return
/// safe defaults per the HTML spec until a media backend is wired in.
pub trait MediaElement: HtmlElement {
    // ---- Source ----

    /// The media source URL.
    fn src(&self) -> String {
        self.attribute("src").unwrap_or_default()
    }

    fn set_src(&self, src: impl Into<String>) {
        self.set_attribute("src", src);
    }

    // ---- Boolean attributes ----

    /// Whether playback should start automatically.
    fn autoplay(&self) -> bool {
        self.attribute("autoplay").is_some()
    }

    fn set_autoplay(&self, autoplay: bool) {
        if autoplay {
            self.set_attribute("autoplay", "");
        } else {
            self.remove_attribute("autoplay");
        }
    }

    /// Whether playback should loop.
    fn loop_playback(&self) -> bool {
        self.attribute("loop").is_some()
    }

    fn set_loop_playback(&self, looping: bool) {
        if looping {
            self.set_attribute("loop", "");
        } else {
            self.remove_attribute("loop");
        }
    }

    /// Whether the audio is muted.
    fn muted(&self) -> bool {
        self.attribute("muted").is_some()
    }

    fn set_muted(&self, muted: bool) {
        if muted {
            self.set_attribute("muted", "");
        } else {
            self.remove_attribute("muted");
        }
    }

    /// Whether the user agent should show controls.
    fn controls(&self) -> bool {
        self.attribute("controls").is_some()
    }

    fn set_controls(&self, controls: bool) {
        if controls {
            self.set_attribute("controls", "");
        } else {
            self.remove_attribute("controls");
        }
    }

    // ---- Preload ----

    /// The preload hint ("none", "metadata", "auto").
    fn preload(&self) -> String {
        self.attribute("preload")
            .unwrap_or_else(|| "auto".to_string())
    }

    fn set_preload(&self, preload: impl Into<String>) {
        self.set_attribute("preload", preload);
    }

    // ---- Playback state (stubs for now) ----

    /// Current playback position in seconds.
    fn current_time(&self) -> f64 {
        0.0
    }

    /// Total duration in seconds. NaN if unknown.
    fn duration(&self) -> f64 {
        f64::NAN
    }

    /// Whether the media is currently paused.
    fn paused(&self) -> bool {
        true
    }

    /// Current ready state.
    fn ready_state(&self) -> MediaReadyState {
        MediaReadyState::HaveNothing
    }

    /// Current network state.
    fn network_state(&self) -> MediaNetworkState {
        MediaNetworkState::Empty
    }

    // ---- Playback actions (stubs for now) ----

    /// Start playback. No-op until a media backend is wired in.
    fn play(&self) {}

    /// Pause playback. No-op until a media backend is wired in.
    fn pause(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;
    use crate::html::HtmlAudioElement;

    #[test]
    fn audio_src() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();
        assert_eq!(audio.src(), "");

        audio.set_src("music.mp3");
        assert_eq!(audio.src(), "music.mp3");
    }

    #[test]
    fn audio_boolean_attrs() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        assert!(!audio.autoplay());
        assert!(!audio.loop_playback());
        assert!(!audio.muted());
        assert!(!audio.controls());

        audio.set_autoplay(true);
        audio.set_controls(true);
        assert!(audio.autoplay());
        assert!(audio.controls());
    }

    #[test]
    fn audio_default_ready_and_network_state() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();
        // paused/duration/current_time are unimplemented — no backend yet.
        assert_eq!(audio.ready_state(), MediaReadyState::HaveNothing);
        assert_eq!(audio.network_state(), MediaNetworkState::Empty);
    }
}
