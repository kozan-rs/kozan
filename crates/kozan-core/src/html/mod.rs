//! HTML element types — like Chrome's `core/html/`.
//!
//! # Architecture
//!
//! ## Trait hierarchy (mirrors Chrome)
//!
//! ```text
//! HtmlElement                         ← Global HTML attributes, actions
//!   ├── FormControlElement            ← Disabled, name, form, validity
//!   │     └── TextControlElement      ← Value, placeholder, selection
//!   ├── MediaElement                  ← Src, play, pause, muted, controls
//!   └── ReplacedElement              ← Intrinsic dimensions
//! ```
//!
//! ## Element categories
//!
//! | Category | Trait | Elements |
//! |----------|-------|----------|
//! | Simple | `HtmlElement` | div, span, p, h1-h6, section, article, ... |
//! | Form control | `FormControlElement` | button, input, select, textarea |
//! | Text control | `TextControlElement` | input (text types), textarea |
//! | Media | `MediaElement` | audio, video |
//! | Replaced | `ReplacedElement` | img, canvas, video |

// ---- Category traits ----
pub mod form_control;
pub mod html_element;
pub mod media_element;
pub mod replaced;

// ---- Concrete elements ----
mod html_anchor_element;
mod html_body_element;
mod html_div_element;
mod html_heading_element;
mod html_list_elements;
mod html_paragraph_element;
mod html_section_elements;
mod html_span_element;
mod html_text_elements;

// Form controls.
mod html_button_element;
mod html_form_element;
mod html_input_element;
mod html_label_element;
mod html_select_element;
mod html_textarea_element;

// Replaced + media elements.
mod html_audio_element;
pub(crate) mod html_canvas_element;
mod html_image_element;
mod html_video_element;

// ---- Re-exports: traits ----
pub use form_control::{FormControlElement, TextControlElement};
pub use html_element::HtmlElement;
pub use media_element::MediaElement;
pub use replaced::{IntrinsicSizing, ReplacedElement};

// ---- Re-exports: simple elements ----
pub use html_anchor_element::{AnchorData, HtmlAnchorElement};
pub use html_body_element::HtmlBodyElement;
pub use html_div_element::HtmlDivElement;
pub use html_heading_element::{HeadingData, HtmlHeadingElement};
pub use html_paragraph_element::HtmlParagraphElement;
pub use html_span_element::HtmlSpanElement;

// Section elements.
pub use html_section_elements::{
    HtmlAddressElement, HtmlArticleElement, HtmlAsideElement, HtmlDetailsElement,
    HtmlFigCaptionElement, HtmlFigureElement, HtmlFooterElement, HtmlHeaderElement,
    HtmlMainElement, HtmlNavElement, HtmlSectionElement, HtmlSummaryElement,
};

// List elements.
pub use html_list_elements::{
    HtmlDListElement, HtmlDdElement, HtmlDtElement, HtmlLiElement, HtmlOListElement,
    HtmlUListElement, OListData,
};

// Text-level elements.
pub use html_text_elements::{
    HtmlAbbrElement, HtmlBElement, HtmlBlockquoteElement, HtmlBrElement, HtmlCiteElement,
    HtmlCodeElement, HtmlEmElement, HtmlHrElement, HtmlIElement, HtmlKbdElement, HtmlMarkElement,
    HtmlPreElement, HtmlQElement, HtmlSElement, HtmlSmallElement, HtmlStrongElement,
    HtmlSubElement, HtmlSupElement, HtmlTimeElement, HtmlUElement, HtmlVarElement, HtmlWbrElement,
};

// ---- Re-exports: form controls ----
pub use html_button_element::{ButtonData, HtmlButtonElement};
pub use html_form_element::{FormData, HtmlFormElement};
pub use html_input_element::{HtmlInputElement, InputData, InputType};
pub use html_label_element::{HtmlLabelElement, LabelData};
pub use html_select_element::{HtmlSelectElement, SelectData};
pub use html_textarea_element::{HtmlTextAreaElement, TextAreaData};

// ---- Re-exports: replaced + media elements ----
pub use html_audio_element::HtmlAudioElement;
pub use html_canvas_element::{Canvas2D, CanvasData, HtmlCanvasElement};
pub use html_image_element::{HtmlImageElement, ImageData};
pub use html_video_element::{HtmlVideoElement, VideoData};
