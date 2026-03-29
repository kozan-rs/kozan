//! CSS style property types for the Kozan UI platform.
//!
//! Three-level value pipeline:
//! - **Specified**: what the user writes (em, rem, vw, calc, var, keywords)
//! - **Computed**: after cascade — relative units resolved, % survives
//! - **Used**: after layout — everything is concrete px
//!
//! All enum/bitflag types and property structs are generated from TOML schema
//! definitions at build time. Hand-written types provide complex CSS values
//! with interned `Atom` strings and `Box<[T]>` frozen slices.

// Allow derive macros to reference `kozan_style::Trait` from within this crate.
extern crate self as kozan_style;

// Core types and context
mod traits;
mod context;

// Value pipeline layers
pub mod computed;
pub mod specified;
pub mod generics;
pub mod calc;
mod declared;

// Generic value wrappers (AutoOr, NoneOr, NormalOr, Edges, etc.)
mod wrappers;

// Hand-written CSS value types
mod animation;
mod border;
mod color;
mod content;
mod filter;
mod font;
mod geometry;
mod grid;
mod helpers;
mod ident;
mod image;
mod shadow;
mod svg;
mod text;
mod transform;
mod ui;

// Re-export core traits
pub use traits::{
    ToCss, ToComputedValue, Zero,
    Animate, Procedure, ToAnimatedZero, ComputeSquaredDistance,
};
pub use context::{ComputeContext, ContainerSize, FontMetrics, ViewportSize};
pub use color::{AbsoluteColor, Color, ColorMix, ColorProperty, ColorSpace, ComputedColor, SystemColor};
// ColorScheme is generated from types.toml

// Re-export calc
pub use calc::{CalcNode, MinMaxOp, var, var_or, env, env_or, attr, unparsed};
pub use declared::{UnparsedValue, SubstitutionRefs};

// Re-export declared
pub use kozan_atom::Atom;
pub use declared::*;

// Re-export generic wrappers
pub use wrappers::*;

// Re-export hand-written types
pub use animation::*;
pub use border::*;
// color types re-exported explicitly above
pub use content::*;
pub use filter::*;
pub use font::*;
pub use geometry::*;
pub use grid::*;
pub use helpers::*;
pub use ident::*;
pub use image::*;
pub use shadow::*;
pub use svg::*;
pub use text::*;
pub use transform::*;
pub use ui::*;

// Re-export specified constructor helpers at top level for ergonomics
pub use specified::length::{
    px, cm, mm, q, inches, pt, pc,
    em, rem, ch, ex, cap, ic, lh, rlh,
    vw, vh, vmin, vmax, dvw, dvh, lvw, lvh, svw, svh,
    cqw, cqh, cqi, cqb, percent,
};

// Generated code
include!(concat!(env!("OUT_DIR"), "/generated_types.rs"));
include!(concat!(env!("OUT_DIR"), "/generated_properties.rs"));
include!(concat!(env!("OUT_DIR"), "/generated_builder.rs"));
