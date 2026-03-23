//! Inline text-level HTML elements.
//!
//! Simple elements that affect text styling/semantics. Most are trivial
//! (no additions over `HTMLElement`). Grouped here for organization.
//!
//! Chrome has separate files for each, but they all inherit `HTMLElement`
//! with minimal additions.

use crate::Handle;
use kozan_macros::Element;

/// `<em>` — emphasis (typically italic).
#[derive(Copy, Clone, Element)]
#[element(tag = "em")]
pub struct HtmlEmElement(Handle);

/// `<strong>` — strong importance (typically bold).
#[derive(Copy, Clone, Element)]
#[element(tag = "strong")]
pub struct HtmlStrongElement(Handle);

/// `<b>` — bring attention (bold).
#[derive(Copy, Clone, Element)]
#[element(tag = "b")]
pub struct HtmlBElement(Handle);

/// `<i>` — alternate voice (italic).
#[derive(Copy, Clone, Element)]
#[element(tag = "i")]
pub struct HtmlIElement(Handle);

/// `<u>` — unarticulated annotation (underline).
#[derive(Copy, Clone, Element)]
#[element(tag = "u")]
pub struct HtmlUElement(Handle);

/// `<s>` — strikethrough (no longer accurate).
#[derive(Copy, Clone, Element)]
#[element(tag = "s")]
pub struct HtmlSElement(Handle);

/// `<small>` — side comment (smaller text).
#[derive(Copy, Clone, Element)]
#[element(tag = "small")]
pub struct HtmlSmallElement(Handle);

/// `<mark>` — highlighted text.
#[derive(Copy, Clone, Element)]
#[element(tag = "mark")]
pub struct HtmlMarkElement(Handle);

/// `<code>` — code fragment (monospace).
#[derive(Copy, Clone, Element)]
#[element(tag = "code")]
pub struct HtmlCodeElement(Handle);

/// `<kbd>` — keyboard input (monospace).
#[derive(Copy, Clone, Element)]
#[element(tag = "kbd")]
pub struct HtmlKbdElement(Handle);

/// `<pre>` — preformatted text (monospace, whitespace preserved).
#[derive(Copy, Clone, Element)]
#[element(tag = "pre")]
pub struct HtmlPreElement(Handle);

/// `<blockquote>` — block quotation.
#[derive(Copy, Clone, Element)]
#[element(tag = "blockquote")]
pub struct HtmlBlockquoteElement(Handle);

/// `<br>` — line break.
#[derive(Copy, Clone, Element)]
#[element(tag = "br")]
pub struct HtmlBrElement(Handle);

/// `<hr>` — thematic break (horizontal rule).
#[derive(Copy, Clone, Element)]
#[element(tag = "hr")]
pub struct HtmlHrElement(Handle);

/// `<abbr>` — abbreviation.
#[derive(Copy, Clone, Element)]
#[element(tag = "abbr")]
pub struct HtmlAbbrElement(Handle);

/// `<cite>` — citation reference.
#[derive(Copy, Clone, Element)]
#[element(tag = "cite")]
pub struct HtmlCiteElement(Handle);

/// `<q>` — inline quotation.
#[derive(Copy, Clone, Element)]
#[element(tag = "q")]
pub struct HtmlQElement(Handle);

/// `<time>` — date/time.
#[derive(Copy, Clone, Element)]
#[element(tag = "time")]
pub struct HtmlTimeElement(Handle);

/// `<sub>` — subscript.
#[derive(Copy, Clone, Element)]
#[element(tag = "sub")]
pub struct HtmlSubElement(Handle);

/// `<sup>` — superscript.
#[derive(Copy, Clone, Element)]
#[element(tag = "sup")]
pub struct HtmlSupElement(Handle);

/// `<var>` — variable.
#[derive(Copy, Clone, Element)]
#[element(tag = "var")]
pub struct HtmlVarElement(Handle);

/// `<wbr>` — word break opportunity.
#[derive(Copy, Clone, Element)]
#[element(tag = "wbr")]
pub struct HtmlWbrElement(Handle);
