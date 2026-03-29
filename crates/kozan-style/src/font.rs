//! CSS font value types.
//! Chrome: blink/renderer/platform/fonts/

use crate::Atom;
use kozan_style_macros::ToComputedValue;

/// CSS `font-family` computed value — ordered list of family names.
/// Chrome: blink/renderer/platform/fonts/font_family.h
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct FontFamily(pub Box<[FamilyEntry]>);

impl FontFamily {
    /// Returns the ordered list of family entries.
    pub fn entries(&self) -> &[FamilyEntry] {
        &self.0
    }
}

impl Default for FontFamily {
    fn default() -> Self {
        Self(Box::from([FamilyEntry::Generic(GenericFamily::SansSerif)]))
    }
}

/// A single entry in a font-family list.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum FamilyEntry {
    Named(Atom),
    Generic(GenericFamily),
}

/// CSS generic font family keywords.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ToComputedValue)]
#[repr(u8)]
pub enum GenericFamily {
    Serif = 0,
    SansSerif = 1,
    Monospace = 2,
    Cursive = 3,
    Fantasy = 4,
    SystemUi = 5,
    UiSerif = 6,
    UiSansSerif = 7,
    UiMonospace = 8,
    UiRounded = 9,
    Emoji = 10,
    Math = 11,
    Fangsong = 12,
}

impl core::fmt::Display for GenericFamily {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Serif => "serif",
            Self::SansSerif => "sans-serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
            Self::SystemUi => "system-ui",
            Self::UiSerif => "ui-serif",
            Self::UiSansSerif => "ui-sans-serif",
            Self::UiMonospace => "ui-monospace",
            Self::UiRounded => "ui-rounded",
            Self::Emoji => "emoji",
            Self::Math => "math",
            Self::Fangsong => "fangsong",
        })
    }
}

/// CSS `font-weight` — numeric (1-1000).
/// `normal` = 400, `bold` = 700 at computed time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ToComputedValue)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: Self = Self(100);
    pub const EXTRA_LIGHT: Self = Self(200);
    pub const LIGHT: Self = Self(300);
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMI_BOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
    pub const EXTRA_BOLD: Self = Self(800);
    pub const BLACK: Self = Self(900);

    /// Returns the numeric weight (1..1000).
    pub const fn value(self) -> u16 { self.0 }
    /// Returns `true` if this weight is bold (>= 700).
    pub const fn is_bold(self) -> bool { self.0 >= 700 }
}

impl Default for FontWeight {
    fn default() -> Self { Self::NORMAL }
}

impl From<u16> for FontWeight {
    fn from(v: u16) -> Self { Self(v.clamp(1, 1000)) }
}

/// CSS `font-feature-settings` — `normal` or OpenType feature tags.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum FontFeatureSettings {
    Normal,
    Features(Box<[FontFeature]>),
}

impl Default for FontFeatureSettings {
    fn default() -> Self { Self::Normal }
}

/// A single OpenType font feature tag and value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct FontFeature {
    pub tag: Atom,
    pub value: u32,
}

/// CSS `font-variation-settings` — `normal` or variation axis values.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum FontVariationSettings {
    Normal,
    Variations(Box<[FontVariation]>),
}

impl Default for FontVariationSettings {
    fn default() -> Self { Self::Normal }
}

/// A single font variation axis tag and value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct FontVariation {
    pub tag: Atom,
    pub value: f32,
}
