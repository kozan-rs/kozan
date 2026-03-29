//! Attribute selector types and matching.
//!
//! CSS attribute selectors test the presence or value of an element's HTML
//! attributes. Seven matching operations are defined by the CSS Selectors
//! Level 4 specification (§6: Attribute Selectors):
//!
//! | Syntax       | Name      | Matches when value…                    |
//! |-------------|-----------|----------------------------------------|
//! | `[attr]`    | Exists    | Attribute is present (any value)        |
//! | `[attr=v]`  | Equals    | Exactly equals `v`                     |
//! | `[attr~=v]` | Includes  | Is a whitespace-separated list containing `v` |
//! | `[attr\|=v]` | DashMatch | Equals `v` or starts with `v-`         |
//! | `[attr^=v]` | Prefix    | Starts with `v`                        |
//! | `[attr$=v]` | Suffix    | Ends with `v`                          |
//! | `[attr*=v]` | Substring | Contains `v`                           |
//!
//! Each value-matching operation supports an optional case-sensitivity flag:
//! `[attr=val i]` for case-insensitive, `[attr=val s]` for case-sensitive
//! (default). In HTML, certain attributes (like `type`) are case-insensitive
//! by default per the HTML spec, but this is handled by the parser, not here.
//!
//! # Performance
//!
//! Attribute values are compared via `Atom::as_str()` (string comparison),
//! not pointer equality, because attribute values are not interned — only
//! attribute *names* are Atoms. The `matches()` method is `#[inline]` to
//! allow the optimizer to monomorphize away the match dispatch.
//!
//! # Spec Reference
//!
//! <https://drafts.csswg.org/selectors-4/#attribute-selectors>

use kozan_atom::Atom;

/// Case sensitivity mode for attribute value matching.
///
/// Controlled by the `i` (insensitive) or `s` (sensitive) flag at the end
/// of an attribute selector: `[type=text i]`. Without a flag, the default
/// is case-sensitive (per CSS Selectors Level 4 §6.3).
///
/// Note: HTML defines some attributes as case-insensitive by default
/// (e.g., `type` on `<input>`). This is a parsing concern, not a matching
/// concern — the parser chooses the right `CaseSensitivity` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseSensitivity {
    /// Default — byte-exact comparison.
    CaseSensitive,
    /// The `i` flag — ASCII case-insensitive comparison.
    /// Only ASCII letters (a-z, A-Z) are folded; Unicode case folding
    /// is NOT performed (per spec).
    AsciiCaseInsensitive,
}

impl CaseSensitivity {
    /// Compare two strings according to this case sensitivity mode.
    #[inline]
    pub fn eq_str(self, a: &str, b: &str) -> bool {
        match self {
            Self::CaseSensitive => a == b,
            Self::AsciiCaseInsensitive => a.eq_ignore_ascii_case(b),
        }
    }

    /// Check if `haystack` starts with `needle` according to case sensitivity.
    #[inline]
    pub fn starts_with(self, haystack: &str, needle: &str) -> bool {
        match self {
            Self::CaseSensitive => haystack.starts_with(needle),
            Self::AsciiCaseInsensitive => {
                haystack.len() >= needle.len()
                    && haystack.as_bytes()[..needle.len()]
                        .eq_ignore_ascii_case(needle.as_bytes())
            }
        }
    }

    /// Check if `haystack` ends with `needle` according to case sensitivity.
    #[inline]
    pub fn ends_with(self, haystack: &str, needle: &str) -> bool {
        match self {
            Self::CaseSensitive => haystack.ends_with(needle),
            Self::AsciiCaseInsensitive => {
                haystack.len() >= needle.len()
                    && haystack.as_bytes()[haystack.len() - needle.len()..]
                        .eq_ignore_ascii_case(needle.as_bytes())
            }
        }
    }

    /// Check if `haystack` contains `needle` according to case sensitivity.
    #[inline]
    pub fn contains(self, haystack: &str, needle: &str) -> bool {
        match self {
            Self::CaseSensitive => haystack.contains(needle),
            Self::AsciiCaseInsensitive => {
                if needle.is_empty() { return true; }
                haystack
                    .as_bytes()
                    .windows(needle.len())
                    .any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
            }
        }
    }
}

/// A parsed CSS attribute selector: `[name op "value" flags]`.
///
/// The attribute name is stored as an interned `Atom` for O(1) name lookup
/// via the `Element::attr()` trait method. The operation determines how
/// the attribute's value (if present) is tested.
///
/// # Examples
///
/// - `[disabled]` → `AttrSelector { name: "disabled", operation: Exists }`
/// - `[type=text]` → `AttrSelector { name: "type", operation: Equals("text", CaseSensitive) }`
/// - `[class~=active i]` → `AttrSelector { name: "class", operation: Includes("active", AsciiCaseInsensitive) }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrSelector {
    /// Interned attribute name — used to query `Element::attr(name)`.
    pub name: Atom,
    /// How to test the attribute's value (or just its presence).
    pub operation: AttrOperation,
}

/// How an attribute selector tests the attribute's value.
///
/// Each variant corresponds to a CSS attribute selector operator.
/// The `Atom` holds the value to test against; `CaseSensitivity`
/// determines comparison mode.
///
/// Note: `Prefix`, `Suffix`, and `Substring` with empty values never match
/// (per spec: "if the value is the empty string then the selector does not
/// represent anything").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrOperation {
    /// `[attr]` — attribute exists.
    Exists,
    /// `[attr=val]` — exact match.
    Equals(Atom, CaseSensitivity),
    /// `[attr~=val]` — value is a whitespace-separated word list containing `val`.
    Includes(Atom, CaseSensitivity),
    /// `[attr|=val]` — value is exactly `val` or starts with `val-`.
    DashMatch(Atom, CaseSensitivity),
    /// `[attr^=val]` — value starts with `val`.
    Prefix(Atom, CaseSensitivity),
    /// `[attr$=val]` — value ends with `val`.
    Suffix(Atom, CaseSensitivity),
    /// `[attr*=val]` — value contains `val`.
    Substring(Atom, CaseSensitivity),
}

impl AttrSelector {
    /// Tests whether an element's attribute value matches this selector.
    #[inline]
    pub fn matches(&self, attr_value: Option<&str>) -> bool {
        match &self.operation {
            AttrOperation::Exists => attr_value.is_some(),
            AttrOperation::Equals(val, cs) => {
                attr_value.is_some_and(|v| cs.eq_str(v, val.as_str()))
            }
            AttrOperation::Includes(val, cs) => {
                attr_value.is_some_and(|v| {
                    v.split_ascii_whitespace().any(|word| cs.eq_str(word, val.as_str()))
                })
            }
            AttrOperation::DashMatch(val, cs) => {
                attr_value.is_some_and(|v| {
                    cs.eq_str(v, val.as_str())
                        || (v.len() > val.len()
                            && v.as_bytes()[val.len()] == b'-'
                            && cs.starts_with(v, val.as_str()))
                })
            }
            AttrOperation::Prefix(val, cs) => {
                !val.is_empty()
                    && attr_value.is_some_and(|v| cs.starts_with(v, val.as_str()))
            }
            AttrOperation::Suffix(val, cs) => {
                !val.is_empty()
                    && attr_value.is_some_and(|v| cs.ends_with(v, val.as_str()))
            }
            AttrOperation::Substring(val, cs) => {
                !val.is_empty()
                    && attr_value.is_some_and(|v| cs.contains(v, val.as_str()))
            }
        }
    }
}
