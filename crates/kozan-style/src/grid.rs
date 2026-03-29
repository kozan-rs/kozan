//! CSS Grid layout value types.

use crate::Atom;
use crate::specified::LengthPercentage;
use kozan_style_macros::ToComputedValue;

/// CSS `grid-template-columns` / `grid-template-rows`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TrackList {
    None,
    Tracks(Box<[TrackEntry]>),
}

impl Default for TrackList {
    fn default() -> Self { Self::None }
}

/// A single entry in a grid track list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TrackEntry {
    Size(TrackSize),
    Repeat(TrackRepeat),
    LineName(Atom),
}

/// CSS grid track size.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TrackSize {
    Auto,
    Length(LengthPercentage),
    Fr(f32),
    MinMax(Box<TrackSize>, Box<TrackSize>),
    FitContent(LengthPercentage),
    MinContent,
    MaxContent,
}

impl Default for TrackSize {
    fn default() -> Self { Self::Auto }
}

/// CSS `repeat()` in grid track definitions.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct TrackRepeat {
    pub count: RepeatCount,
    pub tracks: Box<[TrackEntry]>,
}

/// CSS `repeat()` count: fixed number, `auto-fill`, or `auto-fit`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum RepeatCount { Number(u32), AutoFill, AutoFit }

/// CSS `grid-template-areas`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum GridTemplateAreas {
    None,
    Areas(Box<[Box<[Option<Atom>]>]>),
}

impl Default for GridTemplateAreas {
    fn default() -> Self { Self::None }
}

/// CSS `grid-column-start`, `grid-row-end`, etc.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum GridLine {
    Auto,
    Line(i32),
    Named(Atom),
    Span(i32),
    SpanNamed(Atom),
}

impl Default for GridLine {
    fn default() -> Self { Self::Auto }
}
