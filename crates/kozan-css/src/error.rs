//! CSS parse error types.

/// Kozan-specific parse errors beyond what cssparser provides.
#[derive(Clone, Debug, PartialEq)]
pub enum CustomError {
    /// Property name not recognized.
    UnknownProperty,
    /// Value didn't match the property's grammar.
    InvalidValue,
    /// Shorthand value missing required components.
    IncompleteShorthand,
    /// Custom property name invalid (must start with `--`).
    InvalidCustomPropertyName,
    /// Selector parsing failed.
    InvalidSelector,
    /// Unknown at-rule name.
    UnknownAtRule,
}

/// The error type used throughout kozan-css.
///
/// Wraps cssparser's `ParseError` with our custom error variants.
pub type Error<'i> = cssparser::ParseError<'i, CustomError>;

/// Source location in the original CSS text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: u32,
    pub column: u32,
}

impl From<cssparser::SourceLocation> for SourceLocation {
    fn from(loc: cssparser::SourceLocation) -> Self {
        Self { line: loc.line, column: loc.column }
    }
}


impl core::fmt::Display for CustomError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownProperty => f.write_str("unknown property"),
            Self::InvalidValue => f.write_str("invalid value"),
            Self::IncompleteShorthand => f.write_str("incomplete shorthand"),
            Self::InvalidCustomPropertyName => f.write_str("invalid custom property name"),
            Self::InvalidSelector => f.write_str("invalid selector"),
            Self::UnknownAtRule => f.write_str("unknown at-rule"),
        }
    }
}
