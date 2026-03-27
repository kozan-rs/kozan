//! Line cap and join styles for stroke operations.
//!
//! Chrome equivalent: `SkPaint::Cap` and `SkPaint::Join`.

/// How the ends of lines are drawn.
///
/// Chrome equivalent: `SkPaint::Cap` / Canvas `lineCap` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// How corners where lines meet are drawn.
///
/// Chrome equivalent: `SkPaint::Join` / Canvas `lineJoin` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}
