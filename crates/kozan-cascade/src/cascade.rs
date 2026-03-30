//! Cascade algorithm — sort matched declarations by priority and apply winners.
//!
//! The CSS cascade determines which declarations take effect when multiple
//! rules target the same element and property. Priority order:
//!
//! 1. **Origin + importance** (`CascadeLevel`):
//!    UA normal < User normal < Author normal < Author !important < User !important < UA !important
//! 2. **Layer order** (encoded in `CascadeLevel`):
//!    Earlier layers < later layers for normal; reversed for !important
//! 3. **Specificity**: ID > class/attr/pseudo > type > universal
//! 4. **Source order**: later in source wins
//!
//! Steps 1-2 are packed into a single `u32` comparison via `CascadeLevel`.
//! Steps 3-4 are encoded in `ApplicableDeclaration`'s sort key.

use kozan_style::Importance;
use smallvec::SmallVec;

use crate::origin::{CascadeLevel, CascadeOrigin};
use crate::stylist::IndexedRule;

/// A matched declaration ready for cascade sorting.
///
/// One entry per matched selector (not per declaration — a single style rule
/// may produce multiple entries if it has multiple selectors that match).
/// The `rule_index` points into `Stylist::rules` to access the actual
/// `DeclarationBlock`.
#[derive(Clone, Copy, Debug)]
pub struct ApplicableDeclaration {
    /// Index into `Stylist::rules`.
    pub rule_index: u32,
    /// Specificity of the matched selector (from `RuleEntry`).
    pub specificity: u32,
    /// Source order of the rule (from `RuleEntry`).
    pub source_order: u32,
    /// Cascade origin of this rule (needed for `revert` keyword).
    pub origin: CascadeOrigin,
    /// Layer order within the origin (needed for `revert-layer` keyword).
    pub layer_order: u16,
    /// @scope proximity — number of DOM tree hops from the scope root to this
    /// element. Smaller = closer = higher cascade priority (CSS Cascading L6 §6.3).
    /// `0` for rules without @scope (treated as least-proximate per spec).
    /// Set by the selector-matching phase when the rule comes from an @scope block.
    pub scope_depth: u16,
}

/// Converts `scope_depth` into an ascending sort key where:
/// - Unscoped rules (`scope_depth = 0`) have key `0` → lowest priority.
/// - Scoped rules have key `u16::MAX + 1 - depth` → closer (smaller depth) = higher key = sorts later = wins.
///
/// CSS Cascading Level 6 §6.3: scoped > unscoped; closer scope > farther scope.
#[inline]
fn scope_proximity_key(depth: u16) -> u32 {
    if depth == 0 {
        0 // unscoped: lowest priority
    } else {
        u32::MAX - depth as u32 + 1 // closer = higher key
    }
}

/// Sort applicable declarations by **normal** cascade priority.
///
/// Priority order (ascending — last applied wins):
/// 1. `CascadeLevel` (origin + importance + layer, packed u32)
/// 2. Specificity
/// 3. @scope proximity (CSS Cascading Level 6 §6.3):
///    closer scope root (smaller `scope_depth`) wins over farther one.
///    Unscoped rules have `scope_depth = 0` (minimum proximity = lowest priority).
/// 4. Source order
///
/// `!important` declarations are NOT handled here — they get a separate sort
/// inside `cascade_apply()` using the reversed important cascade level.
pub fn sort(decls: &mut [ApplicableDeclaration], rules: &[IndexedRule]) {
    decls.sort_by(|a, b| {
        let level_a = rules[a.rule_index as usize].level(Importance::Normal);
        let level_b = rules[b.rule_index as usize].level(Importance::Normal);

        level_a
            .as_u32()
            .cmp(&level_b.as_u32())
            .then(a.specificity.cmp(&b.specificity))
            .then(scope_proximity_key(a.scope_depth).cmp(&scope_proximity_key(b.scope_depth)))
            .then(a.source_order.cmp(&b.source_order))
    });
}

/// Two-pass cascade application.
///
/// CSS cascade handles `!important` by running two passes:
///
/// **Pass 1 (Normal):** Apply normal declarations in ascending cascade order.
/// Later declarations overwrite earlier ones for the same property.
///
/// **Pass 2 (Important):** Apply `!important` declarations in ascending
/// `!important` cascade order (which is the reverse of normal origin order).
///
/// The callback `apply_fn` is called for each winning declaration. It receives:
/// - The `IndexedRule` containing the declarations
/// - The `CascadeLevel` for the current pass
/// - The importance filter for the current pass
///
/// The caller is responsible for actually applying declarations to a style builder.
pub fn cascade_apply<F>(
    sorted: &[ApplicableDeclaration],
    rules: &[IndexedRule],
    mut apply_fn: F,
)
where
    F: FnMut(&IndexedRule, CascadeLevel, Importance),
{
    // Pass 1: Normal declarations in ascending order.
    for decl in sorted {
        let rule = &rules[decl.rule_index as usize];
        let level = rule.level(Importance::Normal);
        apply_fn(rule, level, Importance::Normal);
    }

    // Pass 2: Important declarations — re-sort by important cascade level.
    // Important declarations reverse origin and layer order, so we need
    // a separate sort. We collect indices of rules that have important
    // declarations, sort by important level, then apply.
    //
    // Single-pass collect: builds the important list and implicitly detects
    // whether any !important declarations exist (empty vec = none).
    {
        // Tuple: (rule_index, specificity, scope_depth, source_order)
        let mut important: SmallVec<[(u32, u32, u16, u32); 16]> = sorted
            .iter()
            .filter(|d| {
                let rule = &rules[d.rule_index as usize];
                rule.declarations
                    .entries()
                    .iter()
                    .any(|(_, imp)| *imp == Importance::Important)
            })
            .map(|d| (d.rule_index, d.specificity, d.scope_depth, d.source_order))
            .collect();

        if !important.is_empty() {
            important.sort_by(|a, b| {
                let level_a = rules[a.0 as usize].level(Importance::Important);
                let level_b = rules[b.0 as usize].level(Importance::Important);

                level_a
                    .as_u32()
                    .cmp(&level_b.as_u32())
                    .then(a.1.cmp(&b.1))
                    .then(scope_proximity_key(a.2).cmp(&scope_proximity_key(b.2)))
                    .then(a.3.cmp(&b.3))
            });

            for &(rule_index, _, _, _) in important.iter() {
                let rule = &rules[rule_index as usize];
                let level = rule.level(Importance::Important);
                apply_fn(rule, level, Importance::Important);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::origin::{CascadeOrigin, Importance};
    use crate::layer::UNLAYERED;
    use kozan_style::DeclarationBlock;

    fn make_rule(origin: CascadeOrigin, layer: u16) -> IndexedRule {
        IndexedRule {
            declarations: triomphe::Arc::new(DeclarationBlock::new()),
            origin,
            layer_order: layer,
            container: None,
            scope: None,
            starting_style: false,
        }
    }

    fn make_decl(rule_index: u32, specificity: u32, source_order: u32) -> ApplicableDeclaration {
        ApplicableDeclaration {
            rule_index,
            specificity,
            source_order,
            origin: CascadeOrigin::Author,
            layer_order: crate::layer::UNLAYERED,
            scope_depth: 0,
        }
    }

    #[test]
    fn sort_by_origin() {
        let rules = vec![
            make_rule(CascadeOrigin::UserAgent, UNLAYERED),
            make_rule(CascadeOrigin::Author, UNLAYERED),
        ];
        let mut decls = vec![
            make_decl(1, 0, 0), // author
            make_decl(0, 0, 0), // ua
        ];
        sort(&mut decls, &rules);
        // UA sorts before Author
        assert_eq!(decls[0].rule_index, 0); // UA
        assert_eq!(decls[1].rule_index, 1); // Author
    }

    #[test]
    fn sort_by_specificity() {
        let rules = vec![
            make_rule(CascadeOrigin::Author, UNLAYERED),
            make_rule(CascadeOrigin::Author, UNLAYERED),
        ];
        let mut decls = vec![
            make_decl(0, 100, 0), // higher specificity
            make_decl(1, 10, 1),  // lower specificity
        ];
        sort(&mut decls, &rules);
        assert_eq!(decls[0].rule_index, 1); // lower specificity first
        assert_eq!(decls[1].rule_index, 0); // higher specificity last (wins)
    }

    #[test]
    fn sort_by_source_order() {
        let rules = vec![
            make_rule(CascadeOrigin::Author, UNLAYERED),
            make_rule(CascadeOrigin::Author, UNLAYERED),
        ];
        let mut decls = vec![
            make_decl(0, 10, 5), // later source order
            make_decl(1, 10, 2), // earlier source order
        ];
        sort(&mut decls, &rules);
        assert_eq!(decls[0].rule_index, 1); // earlier first
        assert_eq!(decls[1].rule_index, 0); // later last (wins)
    }

    #[test]
    fn sort_layer_beats_specificity() {
        let rules = vec![
            make_rule(CascadeOrigin::Author, 0), // layer 0 (lower priority)
            make_rule(CascadeOrigin::Author, 1), // layer 1 (higher priority)
        ];
        let mut decls = vec![
            make_decl(0, 1000, 0), // layer 0, high specificity
            make_decl(1, 1, 1),    // layer 1, low specificity
        ];
        sort(&mut decls, &rules);
        // Layer 1 beats layer 0 regardless of specificity
        assert_eq!(decls[0].rule_index, 0); // layer 0 first
        assert_eq!(decls[1].rule_index, 1); // layer 1 last (wins)
    }

    #[test]
    fn sort_origin_beats_layer() {
        let rules = vec![
            make_rule(CascadeOrigin::UserAgent, UNLAYERED),
            make_rule(CascadeOrigin::Author, 0),
        ];
        let mut decls = vec![
            make_decl(0, 0, 0), // UA unlayered
            make_decl(1, 0, 1), // Author layer 0
        ];
        sort(&mut decls, &rules);
        // Author origin beats UA regardless of layer
        assert_eq!(decls[0].rule_index, 0); // UA first
        assert_eq!(decls[1].rule_index, 1); // Author last (wins)
    }

    #[test]
    fn cascade_apply_normal_pass() {
        let rules = vec![
            make_rule(CascadeOrigin::UserAgent, UNLAYERED),
            make_rule(CascadeOrigin::Author, UNLAYERED),
        ];
        let sorted = vec![
            make_decl(0, 0, 0),
            make_decl(1, 10, 1),
        ];
        let mut applied = Vec::new();
        cascade_apply(&sorted, &rules, |_rule, _level, importance| {
            applied.push(importance);
        });
        // Normal pass applies both rules
        assert!(applied.iter().filter(|i| **i == Importance::Normal).count() >= 2);
    }
}
