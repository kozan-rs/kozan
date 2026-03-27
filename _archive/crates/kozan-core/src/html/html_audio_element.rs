//! `HTMLAudioElement` — an audio playback element.
//!
//! Chrome equivalent: `HTMLAudioElement` (inherits from `HTMLMediaElement`).
//! Not a replaced element (no visual content unless controls are shown).

use super::media_element::MediaElement;
use crate::Handle;
use kozan_macros::Element;

/// An audio playback element (`<audio>`).
///
/// Chrome equivalent: `HTMLAudioElement`.
/// Inherits all playback behavior from `MediaElement`.
#[derive(Copy, Clone, Element)]
#[element(tag = "audio")]
pub struct HtmlAudioElement(Handle);

impl MediaElement for HtmlAudioElement {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;
    use crate::dom::traits::Element;
    use crate::html::media_element::{MediaNetworkState, MediaReadyState};

    #[test]
    fn audio_tag_name() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();
        assert_eq!(audio.tag_name(), "audio");
    }

    #[test]
    fn audio_src_roundtrip() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        assert_eq!(audio.src(), "");
        audio.set_src("song.mp3");
        assert_eq!(audio.src(), "song.mp3");
    }

    #[test]
    fn audio_autoplay_toggle() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        assert!(!audio.autoplay());
        audio.set_autoplay(true);
        assert!(audio.autoplay());
        audio.set_autoplay(false);
        assert!(!audio.autoplay());
    }

    #[test]
    fn audio_loop_toggle() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        assert!(!audio.loop_playback());
        audio.set_loop_playback(true);
        assert!(audio.loop_playback());
        audio.set_loop_playback(false);
        assert!(!audio.loop_playback());
    }

    #[test]
    fn audio_muted_toggle() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        assert!(!audio.muted());
        audio.set_muted(true);
        assert!(audio.muted());
        audio.set_muted(false);
        assert!(!audio.muted());
    }

    #[test]
    fn audio_controls_toggle() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        assert!(!audio.controls());
        audio.set_controls(true);
        assert!(audio.controls());
        audio.set_controls(false);
        assert!(!audio.controls());
    }

    #[test]
    fn audio_preload_default_and_set() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        // Default preload is "auto".
        assert_eq!(audio.preload(), "auto");
        audio.set_preload("none");
        assert_eq!(audio.preload(), "none");
        audio.set_preload("metadata");
        assert_eq!(audio.preload(), "metadata");
    }

    #[test]
    fn audio_default_ready_and_network_state() {
        let doc = Document::new();
        let audio = doc.create::<HtmlAudioElement>();

        // current_time/duration/paused are unimplemented — no backend yet.
        assert_eq!(audio.ready_state(), MediaReadyState::HaveNothing);
        assert_eq!(audio.network_state(), MediaNetworkState::Empty);
    }
}
