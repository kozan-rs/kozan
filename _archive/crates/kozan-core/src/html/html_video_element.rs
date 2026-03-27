//! `HTMLVideoElement` — a video playback element.
//!
//! Chrome equivalent: `HTMLVideoElement`.
//! Both a `MediaElement` AND a `ReplacedElement` — it has intrinsic
//! dimensions from the video resolution.
//!
//! # Chrome hierarchy
//!
//! ```text
//! HTMLElement → HTMLMediaElement → HTMLVideoElement
//! LayoutObject → LayoutBox → LayoutReplaced → LayoutImage → LayoutMedia → LayoutVideo
//! ```

use super::media_element::MediaElement;
use super::replaced::{IntrinsicSizing, ReplacedElement};
use crate::Handle;
use kozan_macros::{Element, Props};

/// A video playback element (`<video>`).
///
/// Chrome equivalent: `HTMLVideoElement`.
/// Implements both `MediaElement` (playback) and `ReplacedElement` (intrinsic size).
#[derive(Copy, Clone, Element)]
#[element(tag = "video", data = VideoData)]
pub struct HtmlVideoElement(Handle);

/// Element-specific data for `<video>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlVideoElement)]
#[non_exhaustive]
pub struct VideoData {
    /// The video's natural width in pixels.
    #[prop]
    pub video_width: f32,
    /// The video's natural height in pixels.
    #[prop]
    pub video_height: f32,
    /// The poster image URL (shown before playback starts).
    #[prop]
    pub poster: String,
}

impl MediaElement for HtmlVideoElement {}

impl ReplacedElement for HtmlVideoElement {
    fn intrinsic_sizing(&self) -> IntrinsicSizing {
        let w = self.video_width();
        let h = self.video_height();

        if w > 0.0 && h > 0.0 {
            IntrinsicSizing::from_size(w, h)
        } else {
            // Video metadata not loaded yet — default 300x150 per spec.
            IntrinsicSizing::from_size(300.0, 150.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;

    #[test]
    fn video_default_intrinsic_size() {
        let doc = Document::new();
        let video = doc.create::<HtmlVideoElement>();

        // Default: 300x150 per HTML spec.
        let sizing = video.intrinsic_sizing();
        assert_eq!(sizing.width, Some(300.0));
        assert_eq!(sizing.height, Some(150.0));
    }

    #[test]
    fn video_with_dimensions() {
        let doc = Document::new();
        let video = doc.create::<HtmlVideoElement>();

        video.set_video_width(1920.0);
        video.set_video_height(1080.0);

        let sizing = video.intrinsic_sizing();
        assert_eq!(sizing.width, Some(1920.0));
        assert_eq!(sizing.height, Some(1080.0));
        assert!((sizing.aspect_ratio.unwrap() - 1.777).abs() < 0.01);
    }

    #[test]
    fn video_poster() {
        let doc = Document::new();
        let video = doc.create::<HtmlVideoElement>();

        video.set_poster("thumbnail.jpg");
        assert_eq!(video.poster(), "thumbnail.jpg");
    }
}
