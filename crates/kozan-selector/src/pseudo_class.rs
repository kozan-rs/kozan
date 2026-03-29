//! CSS pseudo-class types and element state bitflags.
//!
//! Pseudo-classes select elements based on information that lies outside the
//! document tree, or that cannot be expressed using simple selectors.
//! CSS Selectors Level 4 defines ~50 pseudo-classes; we support 42.
//!
//! # Three Categories
//!
//! 1. **State-based** (27 variants): Matched via `ElementState` bitflags.
//!    The DOM sets these flags; matching is a single AND instruction — O(1)
//!    regardless of how many states are checked. This is our key performance
//!    advantage over Stylo, which uses per-pseudo-class method calls.
//!
//! 2. **Structural** (8 variants): Require DOM tree queries (parent/sibling
//!    traversal). Examples: `:first-child`, `:empty`, `:root`.
//!    Performance depends on DOM implementation caching.
//!
//! 3. **Context-dependent** (1 variant): Require `MatchingContext` to resolve.
//!    Example: `:scope` — depends on which element is the scoping root.
//!
//! # Functional Pseudo-classes
//!
//! Functional pseudo-classes that take arguments (`:not()`, `:is()`, `:where()`,
//! `:has()`, `:nth-child()`, `:lang()`, `:dir()`) are NOT in this enum.
//! They're represented as `Component` variants in `types.rs` because they
//! contain nested data (selector lists, An+B formulas, etc.).
//!
//! # Spec Reference
//!
//! <https://drafts.csswg.org/selectors-4/#pseudo-classes>

use bitflags::bitflags;

bitflags! {
    /// Element interaction and form state — one bit per state-based pseudo-class.
    ///
    /// The DOM layer sets these flags on elements (e.g., set `HOVER` when the
    /// mouse enters, clear it when it leaves). The selector engine only reads
    /// them via `Element::state()`.
    ///
    /// Matching `:hover` is `state.contains(HOVER)` — a single AND instruction.
    /// Matching `:hover:focus` is `state.contains(HOVER | FOCUS)` — still one AND.
    ///
    /// State flags use `u64` for headroom — 40 flags defined, 24 bits spare
    /// for future CSS additions (e.g., `:user-drag`, custom states).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ElementState: u64 {
        const HOVER             = 1 << 0;
        const ACTIVE            = 1 << 1;
        const FOCUS             = 1 << 2;
        const FOCUS_WITHIN      = 1 << 3;
        const FOCUS_VISIBLE     = 1 << 4;
        const ENABLED           = 1 << 5;
        const DISABLED          = 1 << 6;
        const CHECKED           = 1 << 7;
        const INDETERMINATE     = 1 << 8;
        const REQUIRED          = 1 << 9;
        const OPTIONAL          = 1 << 10;
        const VALID             = 1 << 11;
        const INVALID           = 1 << 12;
        const READ_ONLY         = 1 << 13;
        const READ_WRITE        = 1 << 14;
        const PLACEHOLDER_SHOWN = 1 << 15;
        const DEFAULT           = 1 << 16;
        const TARGET            = 1 << 17;
        const VISITED           = 1 << 18;
        const LINK              = 1 << 19;
        const FULLSCREEN        = 1 << 20;
        const MODAL             = 1 << 21;
        const POPOVER_OPEN      = 1 << 22;
        const DEFINED           = 1 << 23;
        const AUTOFILL          = 1 << 24;
        const USER_VALID        = 1 << 25;
        const USER_INVALID      = 1 << 26;
        const PLAYING           = 1 << 27;
        const PAUSED            = 1 << 28;
        const SEEKING           = 1 << 29;
        const BUFFERING         = 1 << 30;
        const STALLED           = 1 << 31;
        const MUTED             = 1 << 32;
        const VOLUME_LOCKED     = 1 << 33;
        const BLANK             = 1 << 34;
        const IN_RANGE          = 1 << 35;
        const OUT_OF_RANGE      = 1 << 36;
        const OPEN              = 1 << 37;
        const CLOSED            = 1 << 38;
        const PICTURE_IN_PICTURE = 1 << 39;
        const TARGET_WITHIN     = 1 << 40;
        const LOCAL_LINK        = 1 << 41;
        const CURRENT           = 1 << 42;
        const PAST              = 1 << 43;
        const FUTURE            = 1 << 44;
    }
}

/// Non-functional CSS pseudo-classes.
///
/// These pseudo-classes take no arguments (`:hover`, `:first-child`, etc.).
/// Functional pseudo-classes (`:not()`, `:nth-child()`, etc.) are represented
/// as `Component` variants because they contain nested data.
///
/// Each state-based variant has a corresponding `ElementState` flag accessible
/// via `state_flag()`. Structural variants return `None` from `state_flag()`
/// and are matched via DOM tree queries in `matching.rs`.
///
/// # Matching Performance
///
/// ```text
/// state_flag() returns Some(flag)
///   → element.state().contains(flag)  // 1 AND instruction
///
/// state_flag() returns None
///   → tree query (parent/sibling walk)  // O(1) to O(n)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoClass {
    // -- State-based (matched via ElementState — 1 AND instruction) --
    Hover,
    Active,
    Focus,
    FocusWithin,
    FocusVisible,
    Enabled,
    Disabled,
    Checked,
    Indeterminate,
    Required,
    Optional,
    Valid,
    Invalid,
    ReadOnly,
    ReadWrite,
    PlaceholderShown,
    Default,
    Target,
    Visited,
    Link,
    AnyLink,
    Fullscreen,
    Modal,
    PopoverOpen,
    Defined,
    Autofill,
    UserValid,
    UserInvalid,

    // -- Structural (require tree queries) --
    Root,
    Empty,
    FirstChild,
    LastChild,
    OnlyChild,
    FirstOfType,
    LastOfType,
    OnlyOfType,

    // -- Context-dependent (require MatchingContext) --
    /// `:scope` — matches the scoping element. In querySelector, it's the
    /// element the query was called on. In @scope rules, it's the scope root.
    /// Without context, falls back to `:root` (per spec).
    Scope,

    // -- Media state (CSS Selectors 4 — audio/video elements) --
    /// `:playing` — matches a media element that is playing.
    Playing,
    /// `:paused` — matches a media element that is paused.
    Paused,
    /// `:seeking` — matches a media element that is seeking.
    Seeking,
    /// `:buffering` — matches a media element that is buffering.
    Buffering,
    /// `:stalled` — matches a media element that is stalled.
    Stalled,
    /// `:muted` — matches a media element that is muted.
    Muted,
    /// `:volume-locked` — matches a media element whose volume is locked by user agent.
    VolumeLocked,

    // -- Additional commonly-used pseudo-classes --
    /// `:blank` — matches empty input fields (no user input or value).
    /// Differs from `:empty` which is about element children.
    Blank,

    // -- Range validation (form elements) --
    /// `:in-range` — matches when element value is within min/max constraints.
    InRange,
    /// `:out-of-range` — matches when element value is outside min/max constraints.
    OutOfRange,

    // -- Open/Closed state (CSS Open UI) --
    /// `:open` — matches open `<details>`, `<dialog>`, `<select>`, popovers.
    Open,
    /// `:closed` — matches closed `<details>`, `<dialog>`, `<select>`, popovers.
    Closed,

    // -- Media viewport state --
    /// `:picture-in-picture` — matches element displayed in PiP viewport.
    PictureInPicture,

    // -- Location pseudo-classes (CSS Selectors Level 4) --
    /// `:target-within` — matches elements that contain or are the `:target`.
    TargetWithin,
    /// `:local-link` — matches links pointing to the same document URL.
    LocalLink,

    // -- Time-dimensional pseudo-classes (CSS Selectors Level 4 § 12) --
    /// `:current` — matches the element currently being presented in a
    /// time-based media presentation (e.g., WebVTT captions).
    Current,
    /// `:past` — matches elements that have already been presented.
    Past,
    /// `:future` — matches elements that have not yet been presented.
    Future,
}

impl PseudoClass {
    /// Returns the `ElementState` flag for state-based pseudo-classes,
    /// or `None` for structural pseudo-classes that need tree queries.
    pub const fn state_flag(self) -> Option<ElementState> {
        match self {
            Self::Hover => Some(ElementState::HOVER),
            Self::Active => Some(ElementState::ACTIVE),
            Self::Focus => Some(ElementState::FOCUS),
            Self::FocusWithin => Some(ElementState::FOCUS_WITHIN),
            Self::FocusVisible => Some(ElementState::FOCUS_VISIBLE),
            Self::Enabled => Some(ElementState::ENABLED),
            Self::Disabled => Some(ElementState::DISABLED),
            Self::Checked => Some(ElementState::CHECKED),
            Self::Indeterminate => Some(ElementState::INDETERMINATE),
            Self::Required => Some(ElementState::REQUIRED),
            Self::Optional => Some(ElementState::OPTIONAL),
            Self::Valid => Some(ElementState::VALID),
            Self::Invalid => Some(ElementState::INVALID),
            Self::ReadOnly => Some(ElementState::READ_ONLY),
            Self::ReadWrite => Some(ElementState::READ_WRITE),
            Self::PlaceholderShown => Some(ElementState::PLACEHOLDER_SHOWN),
            Self::Default => Some(ElementState::DEFAULT),
            Self::Target => Some(ElementState::TARGET),
            Self::Visited => Some(ElementState::VISITED),
            Self::Link => Some(ElementState::LINK),
            Self::Fullscreen => Some(ElementState::FULLSCREEN),
            Self::Modal => Some(ElementState::MODAL),
            Self::PopoverOpen => Some(ElementState::POPOVER_OPEN),
            Self::Defined => Some(ElementState::DEFINED),
            Self::Autofill => Some(ElementState::AUTOFILL),
            Self::UserValid => Some(ElementState::USER_VALID),
            Self::UserInvalid => Some(ElementState::USER_INVALID),
            Self::Playing => Some(ElementState::PLAYING),
            Self::Paused => Some(ElementState::PAUSED),
            Self::Seeking => Some(ElementState::SEEKING),
            Self::Buffering => Some(ElementState::BUFFERING),
            Self::Stalled => Some(ElementState::STALLED),
            Self::Muted => Some(ElementState::MUTED),
            Self::VolumeLocked => Some(ElementState::VOLUME_LOCKED),
            Self::Blank => Some(ElementState::BLANK),
            Self::InRange => Some(ElementState::IN_RANGE),
            Self::OutOfRange => Some(ElementState::OUT_OF_RANGE),
            Self::Open => Some(ElementState::OPEN),
            Self::Closed => Some(ElementState::CLOSED),
            Self::PictureInPicture => Some(ElementState::PICTURE_IN_PICTURE),
            Self::TargetWithin => Some(ElementState::TARGET_WITHIN),
            Self::LocalLink => Some(ElementState::LOCAL_LINK),
            Self::Current => Some(ElementState::CURRENT),
            Self::Past => Some(ElementState::PAST),
            Self::Future => Some(ElementState::FUTURE),
            Self::AnyLink => None, // matches LINK | VISITED — handled separately
            _ => None,
        }
    }

    /// Whether this is a structural pseudo-class that depends on tree position.
    ///
    /// Structural pseudo-classes require DOM tree queries and their results
    /// can change when children are added/removed/reordered. The invalidation
    /// system uses this to track which selectors need re-evaluation on
    /// structural mutations.
    pub const fn is_structural(self) -> bool {
        matches!(
            self,
            Self::Root
                | Self::Empty
                | Self::FirstChild
                | Self::LastChild
                | Self::OnlyChild
                | Self::FirstOfType
                | Self::LastOfType
                | Self::OnlyOfType
        )
    }
}
