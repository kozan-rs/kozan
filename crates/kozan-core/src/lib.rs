//! `kozan-core` — Core DOM engine for Kozan.
//!
//! # Module structure (mirrors Chrome)
//!
//! - [`dom`] — DOM types: Node, `EventTarget`, Element, Handle, Document, Text
//! - [`events`] — Event system: Event, dispatch, listeners, path
//! - [`html`] — HTML elements: `HtmlElement`, `HtmlDivElement`, `HtmlButtonElement`

// Allow derive macros to reference `::kozan_core` from within this crate.
extern crate self as kozan_core;

// Internal infrastructure (not part of the public DOM API).
mod data_storage;
pub(crate) mod id;
pub(crate) mod tree;

// Public modules (Chrome-aligned structure).
pub mod dom;
pub mod events;
pub mod html;
pub mod input;
pub mod styling;

pub mod layout;
pub mod paint;
pub mod compositor;
pub mod scroll;
pub mod widget;
pub(crate) mod dirty_phases;
pub mod lifecycle;

// Re-exports: styling (Stylo-backed CSS engine).
pub use styling::{Arc, ComputedValues, PropertyDeclarationBlock};
pub use styling::values;
pub use styling::style_structs;

// Re-exports: traits (most important — users import these).
pub use dom::traits::{ContainerNode, Element, HasHandle, Node};
pub use events::EventTarget;
pub use html::HtmlElement;
pub use html::MediaElement;
pub use html::{FormControlElement, TextControlElement};
pub use html::{IntrinsicSizing, ReplacedElement};

// Re-exports: core types.
pub use dom::attribute::{Attribute, AttributeCollection};
pub use dom::document::Document;
pub use dom::element_data::ElementData;
pub use dom::handle::Handle;
pub use dom::node::{NodeFlags, NodeType};
pub use id::RawId;

// Re-exports: node types.
pub use dom::text::{Text, TextData};
pub use html::HtmlAudioElement;
pub use html::HtmlBodyElement;
pub use html::HtmlDivElement;
pub use html::HtmlParagraphElement;
pub use html::HtmlSpanElement;
pub use html::{AnchorData, HtmlAnchorElement};
pub use html::{ButtonData, HtmlButtonElement};
pub use html::{CanvasData, HtmlCanvasElement};
pub use html::{FormData, HtmlFormElement};
pub use html::{HeadingData, HtmlHeadingElement};
pub use html::{HtmlImageElement, ImageData};
pub use html::{HtmlInputElement, InputData, InputType};
pub use html::{HtmlLabelElement, LabelData};
pub use html::{HtmlSelectElement, SelectData};
pub use html::{HtmlTextAreaElement, TextAreaData};
pub use html::{HtmlVideoElement, VideoData};

// Re-exports: events.
pub use events::{Event, EventContext, ListenerId, ListenerOptions};

// Re-exports: DOM event types (dispatched through the tree).
// Note: MouseMoveEvent, MouseEnterEvent, MouseLeaveEvent, WheelEvent share names
// with platform-level types in `input::`. Module path disambiguates.
pub use events::{
    BlurEvent, ClickEvent, ContextMenuEvent, DblClickEvent, FocusEvent, FocusInEvent,
    FocusOutEvent, KeyDownEvent, KeyUpEvent, MouseDownEvent, MouseOutEvent, MouseOverEvent,
    MouseUpEvent, ResizeEvent, ScrollEvent,
};
// Re-export collision-prone DOM events via their module for explicit access.
pub use events::mouse_event::{MouseEnterEvent, MouseLeaveEvent, MouseMoveEvent};
pub use events::wheel_event::WheelEvent;

// Re-exports: input types (engine's public input API).
pub use input::{ButtonState, InputEvent, KeyCode, Modifiers, MouseButton};

// Re-export derive macros.
pub use kozan_macros::{Element as DeriveElement, Event as DeriveEvent, Node as DeriveNode, Props};
