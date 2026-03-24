//! The Kozan prelude — `use kozan::prelude::*;`
//!
//! Imports everything needed to build a Kozan UI in one line.
//!
//! # What's included
//!
//! - **App**: `App`, `ViewContext`, `WindowConfig`
//! - **DOM traits**: `Node`, `ContainerNode`, `Element`, `HtmlElement`, `EventTarget`
//! - **HTML elements**: `HtmlDivElement`, `HtmlButtonElement`, … (all 20+)
//! - **Style**: `px()`, `em()`, `pct()`, `Color`
//! - **Events**: `Event`, `EventContext`, `ListenerOptions`
//!
//! ```ignore
//! use kozan::prelude::*;
//!
//! fn main() -> kozan::Result<()> {
//!     App::new()
//!         .window(WindowConfig::default(), |ctx| { /* build DOM */ })
//!         .run()
//! }
//! ```

// DOM traits — method calls on elements won't work without these in scope.
pub use kozan_core::{ContainerNode, Element, EventTarget, HasHandle, HtmlElement, Node};

// Document.
pub use kozan_core::Document;

// HTML elements — the types users create.
pub use kozan_core::{
    HtmlAnchorElement, HtmlAudioElement, HtmlButtonElement, HtmlCanvasElement, HtmlDivElement,
    HtmlFormElement, HtmlHeadingElement, HtmlImageElement, HtmlInputElement, HtmlLabelElement,
    HtmlParagraphElement, HtmlSelectElement, HtmlSpanElement, HtmlTextAreaElement,
    HtmlVideoElement,
};

// Category traits.
pub use kozan_core::{
    FormControlElement, IntrinsicSizing, MediaElement, ReplacedElement, TextControlElement,
};

// Style — type-safe property API powered by Stylo.
// div.style()
//     .width(px(200.0))
//     .height(pct(100.0))
//     .background_color(rgb(0.9, 0.3, 0.2));
pub use kozan_core::styling::units::{auto, em, pct, px, rem, vh, vw};
pub use kozan_core::styling::units::{hex, rgb, rgb8, rgba};

// Color — used in almost every style rule.
pub use kozan_primitives::color::Color;

// Input.
pub use kozan_core::{ButtonState, InputEvent, KeyCode, Modifiers, MouseButton};

// Events.
pub use kozan_core::{Event, EventContext, ListenerId, ListenerOptions};

// DOM event types — all 19 typed events for interactive UIs.
pub use kozan_core::events::mouse_event::{MouseEnterEvent, MouseLeaveEvent, MouseMoveEvent};
pub use kozan_core::events::wheel_event::WheelEvent;
pub use kozan_core::{
    BlurEvent, ClickEvent, ContextMenuEvent, DblClickEvent, FocusEvent, FocusInEvent,
    FocusOutEvent, KeyDownEvent, KeyUpEvent, MouseDownEvent, MouseOutEvent, MouseOverEvent,
    MouseUpEvent, ResizeEvent, ScrollEvent,
};

// App + Platform.
pub use crate::App;
pub use kozan_platform::{ViewContext, WindowConfig};

// Scheduler — advanced cross-thread posting (rarely needed directly).
// Normal async work uses ctx.spawn() instead.
pub use kozan_scheduler::WakeSender;
