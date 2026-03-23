//! Sectioning and grouping HTML elements.
//!
//! Chrome has separate classes for each, but they're all trivial
//! (no additions over `HTMLElement`). Grouped here for organization.
//!
//! # Elements
//!
//! - `<section>` — thematic grouping
//! - `<article>` — self-contained content
//! - `<nav>` — navigation links
//! - `<header>` — introductory content
//! - `<footer>` — footer content
//! - `<main>` — dominant content
//! - `<aside>` — tangentially related content
//! - `<figure>` — self-contained figure
//! - `<figcaption>` — figure caption
//! - `<details>` — disclosure widget (future: toggle behavior)
//! - `<summary>` — details summary (future: click to toggle)
//! - `<address>` — contact information

use crate::Handle;
use kozan_macros::Element;

/// `<section>` — a thematic grouping of content.
#[derive(Copy, Clone, Element)]
#[element(tag = "section")]
pub struct HtmlSectionElement(Handle);

/// `<article>` — self-contained content.
#[derive(Copy, Clone, Element)]
#[element(tag = "article")]
pub struct HtmlArticleElement(Handle);

/// `<nav>` — navigation links.
#[derive(Copy, Clone, Element)]
#[element(tag = "nav")]
pub struct HtmlNavElement(Handle);

/// `<header>` — introductory content for a section or page.
#[derive(Copy, Clone, Element)]
#[element(tag = "header")]
pub struct HtmlHeaderElement(Handle);

/// `<footer>` — footer for a section or page.
#[derive(Copy, Clone, Element)]
#[element(tag = "footer")]
pub struct HtmlFooterElement(Handle);

/// `<main>` — the dominant content of the document.
#[derive(Copy, Clone, Element)]
#[element(tag = "main")]
pub struct HtmlMainElement(Handle);

/// `<aside>` — tangentially related content.
#[derive(Copy, Clone, Element)]
#[element(tag = "aside")]
pub struct HtmlAsideElement(Handle);

/// `<figure>` — self-contained figure with optional caption.
#[derive(Copy, Clone, Element)]
#[element(tag = "figure")]
pub struct HtmlFigureElement(Handle);

/// `<figcaption>` — caption for a `<figure>`.
#[derive(Copy, Clone, Element)]
#[element(tag = "figcaption")]
pub struct HtmlFigCaptionElement(Handle);

/// `<details>` — disclosure widget.
///
/// Future: toggle behavior via click on `<summary>`.
#[derive(Copy, Clone, Element)]
#[element(tag = "details")]
pub struct HtmlDetailsElement(Handle);

/// `<summary>` — summary for a `<details>` element.
#[derive(Copy, Clone, Element)]
#[element(tag = "summary")]
pub struct HtmlSummaryElement(Handle);

/// `<address>` — contact information.
#[derive(Copy, Clone, Element)]
#[element(tag = "address")]
pub struct HtmlAddressElement(Handle);
