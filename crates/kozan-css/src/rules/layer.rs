//! `@layer` rule types — cascade layers.

use kozan_atom::Atom;
use smallvec::SmallVec;
use super::RuleList;

/// `@layer` rule — either a block form or a statement form.
///
/// Block: `@layer name { rules }`
/// Statement: `@layer name1, name2;`
pub enum LayerRule {
    /// Block form with optional name and nested rules.
    Block {
        name: Option<LayerName>,
        rules: RuleList,
    },
    /// Statement form declaring one or more layer names.
    Statement {
        names: SmallVec<[LayerName; 2]>,
    },
}

/// A dotted layer name: `framework.utilities` → `["framework", "utilities"]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerName(pub SmallVec<[Atom; 2]>);
