//! Interned CSS strings — zero-cost clone, O(1) equality.
//!
//! Chrome-style four-tier lookup for maximum multi-thread performance:
//!
//! ```text
//! Atom::new("div")
//!   ↓
//! 0. Static atom table  → HIT → return (zero work, just Arc clone)
//!   ↓ MISS
//! 1. Thread-local cache  → HIT → return (zero sync, zero atomics)
//!   ↓ MISS
//! 2. Global read lock    → HIT → return + cache locally (concurrent readers)
//!   ↓ MISS
//! 3. Global write lock   → insert + cache locally (rare, only first occurrence)
//! ```
//!
//! After warmup, 99%+ of lookups hit tier 0 or 1.
//!
//! # Memory Layout
//!
//! `Atom` is 8 bytes — a single thin pointer via `ThinArc<(), u8>`.
//! The heap layout is: `[refcount | () | length | UTF-8 bytes...]`.
//! This is half the size of `Arc<str>` (which is a fat pointer: ptr + len).

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, RwLock};

use triomphe::ThinArc;

/// Wrapper around `ThinArc<(), u8>` that hashes/compares by UTF-8 content.
/// Used only inside the global and thread-local interning tables.
#[derive(Clone)]
struct InternKey(ThinArc<(), u8>);

impl InternKey {
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.0.slice
    }
}

impl PartialEq for InternKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0.as_ptr(), other.0.as_ptr()) || self.as_bytes() == other.as_bytes()
    }
}

impl Eq for InternKey {}

impl Hash for InternKey {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

impl Borrow<[u8]> for InternKey {
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

static GLOBAL: LazyLock<RwLock<HashSet<InternKey>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

thread_local! {
    static LOCAL: RefCell<HashSet<InternKey>> = RefCell::new(HashSet::new());
}

/// Interned CSS string.
///
/// **8 bytes** on stack (single `ThinArc<(), u8>` thin pointer).
/// Same string → same pointer across all threads.
///
/// - `clone()` → refcount bump, O(1)
/// - `==` → pointer comparison first, O(1) for interned strings
/// - `Hash` → hashes the interned pointer (O(1), not string content)
#[derive(Clone, Debug)]
pub struct Atom(ThinArc<(), u8>);

impl Default for Atom {
    fn default() -> Self { Self::new("") }
}

impl Atom {
    /// Interns a string with four-tier lookup.
    ///
    /// 0. Static atom table (css-atoms feature)
    /// 1. Thread-local cache (zero sync)
    /// 2. Global read lock (concurrent)
    /// 3. Global write lock (rare — only for new strings)
    pub fn new(s: &str) -> Self {
        // Tier 0: static atoms — zero overhead, just ThinArc clone.
        #[cfg(feature = "css-atoms")]
        if let Some(atom) = css_atoms::lookup(s) {
            return atom;
        }

        let key = s.as_bytes();

        // Tier 1: thread-local — zero synchronization.
        let found = LOCAL.with_borrow(|cache| cache.get(key).map(|k| k.0.clone()));
        if let Some(thin) = found {
            return Self(thin);
        }

        // Tier 2: global read lock — concurrent, no blocking between readers.
        {
            let set = GLOBAL.read().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = set.get(key) {
                let thin = existing.0.clone();
                LOCAL.with_borrow_mut(|cache| { cache.insert(InternKey(thin.clone())); });
                return Self(thin);
            }
        }

        // Tier 3: global write lock — only for genuinely new strings.
        let mut set = GLOBAL.write().unwrap_or_else(|e| e.into_inner());

        // Double-check: another thread may have inserted while we waited.
        if let Some(existing) = set.get(key) {
            let thin = existing.0.clone();
            LOCAL.with_borrow_mut(|cache| { cache.insert(InternKey(thin.clone())); });
            return Self(thin);
        }

        let thin = ThinArc::from_header_and_slice((), s.as_bytes());
        let intern = InternKey(thin.clone());
        set.insert(intern.clone());
        LOCAL.with_borrow_mut(|cache| { cache.insert(intern); });
        Self(thin)
    }

    /// Returns the string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        // Safety: Atom is only constructed from valid UTF-8 in Atom::new().
        // The bytes stored in ThinArc are exactly the input &str bytes.
        #[allow(unsafe_code)]
        unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// Returns the raw UTF-8 bytes.
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.0.slice
    }

    /// Pointer identity — `true` if both atoms share the same allocation.
    #[inline]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0.as_ptr(), other.0.as_ptr())
    }
}

impl PartialEq for Atom {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Fast path: same interned pointer → equal (common case).
        self.ptr_eq(other) || self.as_bytes() == other.as_bytes()
    }
}

impl Eq for Atom {}

impl Hash for Atom {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the POINTER, not the string content.
        // Atom is interned: same string → same ThinArc → same pointer.
        // This makes HashMap<Atom, _> lookups O(1) instead of O(string_len).
        (self.0.as_ptr() as usize).hash(state);
    }
}

impl core::ops::Deref for Atom {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Atom {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

// NOTE: Borrow<str> intentionally NOT implemented.
// Atom::hash uses pointer identity (O(1)), not string content.
// Borrow<str> requires hash(atom) == hash(str), which would break
// HashMap invariants. Use Atom::from("x") for lookups instead.

impl core::fmt::Display for Atom {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Atom {
    #[inline]
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Atom {
    #[inline]
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

impl PartialEq<str> for Atom {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Atom {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialOrd for Atom {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Atom {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

#[cfg(feature = "css-atoms")]
mod css_atoms {
    use super::*;

    macro_rules! define_css_atoms {
        ($($name:ident = $val:expr),* $(,)?) => {
            $(
                static $name: LazyLock<ThinArc<(), u8>> = LazyLock::new(|| {
                    let thin = ThinArc::from_header_and_slice((), $val.as_bytes());
                    let key = InternKey(thin.clone());
                    let mut set = GLOBAL.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(existing) = set.get($val.as_bytes()) {
                        existing.0.clone()
                    } else {
                        set.insert(key);
                        thin
                    }
                });
            )*

            #[inline]
            pub fn lookup(s: &str) -> Option<Atom> {
                match s {
                    $( $val => Some(Atom($name.clone())), )*
                    _ => None,
                }
            }
        };
    }

    define_css_atoms! {
        // HTML tags
        ATOM_A = "a",
        ATOM_B = "b",
        ATOM_I = "i",
        ATOM_P = "p",
        ATOM_S = "s",
        ATOM_U = "u",
        ATOM_BR = "br",
        ATOM_DD = "dd",
        ATOM_DL = "dl",
        ATOM_DT = "dt",
        ATOM_EM = "em",
        ATOM_H1 = "h1",
        ATOM_H2 = "h2",
        ATOM_H3 = "h3",
        ATOM_H4 = "h4",
        ATOM_H5 = "h5",
        ATOM_H6 = "h6",
        ATOM_HR = "hr",
        ATOM_LI = "li",
        ATOM_OL = "ol",
        ATOM_TD = "td",
        ATOM_TH = "th",
        ATOM_TR = "tr",
        ATOM_UL = "ul",
        ATOM_COL = "col",
        ATOM_DEL = "del",
        ATOM_DIV = "div",
        ATOM_IMG = "img",
        ATOM_INS = "ins",
        ATOM_KBD = "kbd",
        ATOM_MAP = "map",
        ATOM_NAV = "nav",
        ATOM_PRE = "pre",
        ATOM_SUB = "sub",
        ATOM_SUP = "sup",
        ATOM_SVG = "svg",
        ATOM_VAR = "var",
        ATOM_WBR = "wbr",
        ATOM_BTN = "btn",
        ATOM_ROW = "row",
        ATOM_ABBR = "abbr",
        ATOM_AREA = "area",
        ATOM_BASE = "base",
        ATOM_BODY = "body",
        ATOM_CITE = "cite",
        ATOM_CODE = "code",
        ATOM_DATA = "data",
        ATOM_FORM = "form",
        ATOM_HEAD = "head",
        ATOM_HTML = "html",
        ATOM_LINK = "link",
        ATOM_MAIN = "main",
        ATOM_MARK = "mark",
        ATOM_META = "meta",
        ATOM_RUBY = "ruby",
        ATOM_SAMP = "samp",
        ATOM_SLOT = "slot",
        ATOM_SPAN = "span",
        ATOM_TIME = "time",
        ATOM_BOLD = "bold",
        ATOM_AUTO = "auto",
        ATOM_FLEX = "flex",
        ATOM_GRID = "grid",
        ATOM_NONE = "none",
        ATOM_OPEN = "open",
        ATOM_SHOW = "show",
        ATOM_HIDE = "hide",
        ATOM_ICON = "icon",
        ATOM_CARD = "card",
        ATOM_MENU = "menu",
        ATOM_LIST = "list",
        ATOM_ITEM = "item",
        ATOM_TEXT = "text",
        ATOM_PAGE = "page",
        ATOM_ASIDE = "aside",
        ATOM_AUDIO = "audio",
        ATOM_EMBED = "embed",
        ATOM_METER = "meter",
        ATOM_SMALL = "small",
        ATOM_STYLE = "style",
        ATOM_TABLE = "table",
        ATOM_TBODY = "tbody",
        ATOM_TFOOT = "tfoot",
        ATOM_THEAD = "thead",
        ATOM_TITLE = "title",
        ATOM_TRACK = "track",
        ATOM_VIDEO = "video",
        ATOM_INPUT = "input",
        ATOM_LABEL = "label",
        ATOM_ALIGN = "align",
        ATOM_CLASS = "class",
        ATOM_DEFER = "defer",
        ATOM_MEDIA = "media",
        ATOM_SCOPE = "scope",
        ATOM_VALUE = "value",
        ATOM_WIDTH = "width",
        ATOM_BLOCK = "block",
        ATOM_FIXED = "fixed",
        ATOM_SOLID = "solid",
        ATOM_FOCUS = "focus",
        ATOM_ERROR = "error",
        ATOM_CLOSE = "close",
        ATOM_BADGE = "badge",
        ATOM_ALERT = "alert",
        ATOM_MODAL = "modal",
        ATOM_IMAGE = "image",
        ATOM_BUTTON = "button",
        ATOM_CANVAS = "canvas",
        ATOM_DIALOG = "dialog",
        ATOM_FIGURE = "figure",
        ATOM_FOOTER = "footer",
        ATOM_HEADER = "header",
        ATOM_IFRAME = "iframe",
        ATOM_LEGEND = "legend",
        ATOM_OBJECT = "object",
        ATOM_OPTION = "option",
        ATOM_OUTPUT = "output",
        ATOM_SCRIPT = "script",
        ATOM_SELECT = "select",
        ATOM_SOURCE = "source",
        ATOM_STRONG = "strong",
        ATOM_ACTION = "action",
        ATOM_HEIGHT = "height",
        ATOM_HIDDEN = "hidden",
        ATOM_METHOD = "method",
        ATOM_TARGET = "target",
        ATOM_INLINE = "inline",
        ATOM_CENTER = "center",
        ATOM_NORMAL = "normal",
        ATOM_UNSET = "unset",
        ATOM_STICKY = "sticky",
        ATOM_STATIC = "static",
        ATOM_NOWRAP = "nowrap",
        ATOM_REVERT = "revert",
        ATOM_ACTIVE = "active",
        ATOM_ARTICLE = "article",
        ATOM_CAPTION = "caption",
        ATOM_DETAILS = "details",
        ATOM_PICTURE = "picture",
        ATOM_SECTION = "section",
        ATOM_SUMMARY = "summary",
        ATOM_CHECKED = "checked",
        ATOM_CONTENT = "content",
        ATOM_CHARSET = "charset",
        ATOM_COLSPAN = "colspan",
        ATOM_ROWSPAN = "rowspan",
        ATOM_PATTERN = "pattern",
        ATOM_VISIBLE = "visible",
        ATOM_INHERIT = "inherit",
        ATOM_INITIAL = "initial",
        ATOM_POINTER = "pointer",
        ATOM_LOADING = "loading",
        ATOM_WRAPPER = "wrapper",
        ATOM_TEMPLATE = "template",
        ATOM_TEXTAREA = "textarea",
        ATOM_COLGROUP = "colgroup",
        ATOM_DATALIST = "datalist",
        ATOM_FIELDSET = "fieldset",
        ATOM_NOSCRIPT = "noscript",
        ATOM_OPTGROUP = "optgroup",
        ATOM_PROGRESS = "progress",
        ATOM_DISABLED = "disabled",
        ATOM_DOWNLOAD = "download",
        ATOM_MULTIPLE = "multiple",
        ATOM_READONLY = "readonly",
        ATOM_REQUIRED = "required",
        ATOM_SELECTED = "selected",
        ATOM_TABINDEX = "tabindex",
        ATOM_ABSOLUTE = "absolute",
        ATOM_RELATIVE = "relative",
        ATOM_CONTAINER = "container",
        ATOM_FIGCAPTION = "figcaption",
        ATOM_BLOCKQUOTE = "blockquote",
        ATOM_AUTOFOCUS = "autofocus",
        ATOM_MAXLENGTH = "maxlength",
        ATOM_MINLENGTH = "minlength",
        ATOM_IMPORTANT = "important",
        ATOM_PLACEHOLDER = "placeholder",
        ATOM_AUTOCOMPLETE = "autocomplete",
        ATOM_TRANSPARENT = "transparent",
        ATOM_CURRENTCOLOR = "currentcolor",
        ATOM_BORDER_BOX = "border-box",
        ATOM_CONTENT_BOX = "content-box",

        // HTML attributes (unique strings not already above)
        ATOM_ID = "id",
        ATOM_ALT = "alt",
        ATOM_DIR = "dir",
        ATOM_FOR = "for",
        ATOM_REL = "rel",
        ATOM_SRC = "src",
        ATOM_TOP = "top",
        ATOM_COLS = "cols",
        ATOM_HREF = "href",
        ATOM_LANG = "lang",
        ATOM_LEFT = "left",
        ATOM_NAME = "name",
        ATOM_ROLE = "role",
        ATOM_ROWS = "rows",
        ATOM_SIZE = "size",
        ATOM_STEP = "step",
        ATOM_TYPE = "type",
        ATOM_WRAP = "wrap",
        ATOM_RIGHT = "right",
        ATOM_BOTTOM = "bottom"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_string_same_pointer() {
        let a = Atom::new("--gap");
        let b = Atom::new("--gap");
        assert!(a.ptr_eq(&b));
        assert_eq!(a, b);
    }

    #[test]
    fn different_strings_different_pointers() {
        let a = Atom::new("--gap");
        let b = Atom::new("--color");
        assert!(!a.ptr_eq(&b));
        assert_ne!(a, b);
    }

    #[test]
    fn deref_and_display() {
        let a = Atom::new("hello");
        assert_eq!(&*a, "hello");
        assert_eq!(a.as_str(), "hello");
        assert_eq!(format!("{a}"), "hello");
    }

    #[test]
    fn clone_is_same_pointer() {
        let a = Atom::new("test");
        let b = a.clone();
        assert!(a.ptr_eq(&b));
    }

    #[test]
    fn eq_with_str() {
        let a = Atom::new("flex");
        assert_eq!(a, "flex");
        assert_eq!(a, *"flex");
    }

    #[test]
    fn from_string() {
        let s = String::from("dynamic");
        let a = Atom::from(s);
        let b = Atom::new("dynamic");
        assert!(a.ptr_eq(&b));
    }

    #[test]
    fn usable_as_hashmap_key() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(Atom::new("--x"), 42);
        assert_eq!(map.get(&Atom::new("--x")), Some(&42));
    }

    #[test]
    fn atom_is_8_bytes() {
        assert_eq!(std::mem::size_of::<Atom>(), 8);
    }
}
