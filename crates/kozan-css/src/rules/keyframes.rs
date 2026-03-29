//! `@keyframes` rule types — animation keyframe definitions.

use kozan_atom::Atom;
use kozan_style::DeclarationBlock;
use smallvec::SmallVec;

/// `@keyframes name { ... }` — defines named animation keyframes.
pub struct KeyframesRule {
    /// The animation name (e.g. `fadeIn`, `slide-up`).
    pub name: Atom,
    /// The keyframe blocks in source order.
    pub keyframes: Box<[KeyframeBlock]>,
}

/// A single keyframe block: `0%, 50% { declarations }`.
pub struct KeyframeBlock {
    /// One or more keyframe selectors (percentages or from/to).
    pub selectors: SmallVec<[KeyframeSelector; 2]>,
    /// Property declarations for this keyframe.
    pub declarations: DeclarationBlock,
}

/// A keyframe selector: a percentage or `from`/`to` keyword.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyframeSelector {
    /// A percentage (0.0 = 0%, 1.0 = 100%).
    Percentage(f32),
    /// `from` keyword (equivalent to 0%).
    From,
    /// `to` keyword (equivalent to 100%).
    To,
}

impl KeyframeSelector {
    /// Returns the percentage value (0.0..=1.0).
    #[inline]
    pub fn percentage(&self) -> f32 {
        match self {
            Self::Percentage(p) => *p,
            Self::From => 0.0,
            Self::To => 1.0,
        }
    }
}
