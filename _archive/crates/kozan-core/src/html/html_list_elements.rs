//! List HTML elements — ul, ol, li, dl, dt, dd.
//!
//! Chrome equivalents: `HTMLUListElement`, `HTMLOListElement`, `HTMLLIElement`,
//! `HTMLDListElement`.

use crate::Handle;
use kozan_macros::{Element, Props};

/// `<ul>` — an unordered list.
#[derive(Copy, Clone, Element)]
#[element(tag = "ul")]
pub struct HtmlUListElement(Handle);

/// `<ol>` — an ordered list.
///
/// Chrome equivalent: `HTMLOListElement`. Adds `start`, `reversed`, `type`.
#[derive(Copy, Clone, Element)]
#[element(tag = "ol", data = OListData)]
pub struct HtmlOListElement(Handle);

/// Element-specific data for `<ol>`.
#[derive(Default, Clone, Props)]
#[props(element = HtmlOListElement)]
pub struct OListData {
    /// The starting number for the list.
    #[prop]
    pub start: i32,
    /// Whether the list is reversed.
    #[prop]
    pub reversed: bool,
}

/// `<li>` — a list item.
///
/// Chrome equivalent: `HTMLLIElement`. Adds `value` for ordered lists.
#[derive(Copy, Clone, Element)]
#[element(tag = "li")]
pub struct HtmlLiElement(Handle);

/// `<dl>` — a description list.
#[derive(Copy, Clone, Element)]
#[element(tag = "dl")]
pub struct HtmlDListElement(Handle);

/// `<dt>` — a description term.
#[derive(Copy, Clone, Element)]
#[element(tag = "dt")]
pub struct HtmlDtElement(Handle);

/// `<dd>` — a description detail.
#[derive(Copy, Clone, Element)]
#[element(tag = "dd")]
pub struct HtmlDdElement(Handle);
