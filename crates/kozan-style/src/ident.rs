//! CSS identifiers and URL references.

use crate::Atom;
use kozan_style_macros::ToComputedValue;

/// CSS custom identifier (container-name, animation-name references, etc.).
#[derive(Clone, Debug, PartialEq, Default, ToComputedValue)]
pub struct Ident(pub Atom);

impl Ident {
    /// Returns the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Ident {
    fn from(s: &str) -> Self {
        Self(Atom::new(s))
    }
}

/// CSS `url()` reference.
#[derive(Clone, Debug, PartialEq, Default, ToComputedValue)]
pub struct Url(pub Atom);

impl Url {
    /// Returns the URL as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Url {
    fn from(s: &str) -> Self {
        Self(Atom::new(s))
    }
}
