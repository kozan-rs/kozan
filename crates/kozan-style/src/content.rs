//! CSS generated content value types.

use crate::Atom;
use kozan_style_macros::ToComputedValue;

/// CSS `content` property value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum Content {
    Normal,
    None,
    Items(Box<[ContentItem]>),
}

impl Default for Content {
    fn default() -> Self { Self::Normal }
}

/// Individual `content` value items.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ContentItem {
    String(Atom),
    Url(Atom),
    Counter(Atom, Option<crate::ListStyleType>),
    Counters(Atom, Atom, Option<crate::ListStyleType>),
    OpenQuote,
    CloseQuote,
    NoOpenQuote,
    NoCloseQuote,
    Attr(Atom),
}

/// CSS `counter-reset` / `counter-increment` / `counter-set`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum CounterList {
    None,
    Counters(Box<[CounterEntry]>),
}

impl Default for CounterList {
    fn default() -> Self { Self::None }
}

/// A single counter name-value pair.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct CounterEntry {
    pub name: Atom,
    pub value: i32,
}

/// CSS `quotes`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum Quotes {
    Auto,
    None,
    Pairs(Box<[(Atom, Atom)]>),
}

impl Default for Quotes {
    fn default() -> Self { Self::Auto }
}
