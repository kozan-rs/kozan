//! Derive macros for Kozan node types.
//!
//! - [`Node`] — for non-element nodes (`TextNode`, `CommentNode`)
//! - [`Element`] — for element nodes (Button, Div, Input)
//! - [`Props`] — generates getters/setters from data structs

mod crate_path;
mod derive_element;
mod derive_event;
mod derive_node;
mod derive_props;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derive the [`Node`] trait for a newtype wrapper around `Handle`.
///
/// Use this for **non-element** node types that cannot have children
/// or attributes (e.g., `TextNode`, `CommentNode`).
///
/// # Example
///
/// ```ignore
/// #[derive(Copy, Clone, Node)]
/// pub struct TextNode(Handle);
/// ```
///
/// Generates `impl Node for TextNode` and `impl Debug for TextNode`.
#[proc_macro_derive(Node)]
pub fn derive_node(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_node::expand(&input).into()
}

/// Derive `Node`, `ContainerNode`, and `Element` traits for an element type.
///
/// Use this for **element** node types that can have children, attributes,
/// and element-specific data.
///
/// # Attributes
///
/// - `tag = "..."` — **(required)** the HTML tag name
/// - `focusable` — mark the element as focusable by default
/// - `data = TypeName` — the element-specific data type (default: `()`)
///
/// # Example
///
/// ```ignore
/// #[derive(Copy, Clone, Element)]
/// #[element(tag = "button", focusable, data = ButtonData)]
/// pub struct Button(Handle);
/// ```
#[proc_macro_derive(Element, attributes(element))]
pub fn derive_element(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_element::expand(&input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Generate getters and setters on an element type from its data struct.
///
/// Fields marked with `#[prop]` get a getter (`fn field(&self) -> T`)
/// and a setter (`fn set_field(&self, value: impl Into<T>)`).
///
/// # Attributes
///
/// - `element = TypeName` — **(required)** the element handle type
///
/// # Example
///
/// ```ignore
/// #[derive(Default, Props)]
/// #[props(element = Button)]
/// pub struct ButtonData {
///     #[prop]
///     pub label: String,
///     #[prop]
///     pub disabled: bool,
/// }
/// ```
/// Derive the [`Event`] trait for an event struct.
///
/// # Attributes
///
/// - `bubbles` — the event bubbles up through the tree
/// - `cancelable` — the event can be cancelled via `prevent_default()`
///
/// # Example
///
/// ```ignore
/// #[derive(Event)]
/// #[event(bubbles, cancelable)]
/// pub struct ClickEvent {
///     pub x: f32,
///     pub y: f32,
/// }
/// ```
#[proc_macro_derive(Event, attributes(event))]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_event::expand(&input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(Props, attributes(props, prop))]
pub fn derive_props(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_props::expand(&input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
