//! Container query evaluation — bridges style and layout.
//!
//! Container queries create a circular dependency: style needs container sizes
//! (from layout), layout needs computed styles. The solution (same as Chrome):
//!
//! **Container sizes come from the PREVIOUS layout.**
//!
//! ```text
//! Frame N:
//!   1. Style — evaluate @container using sizes from frame N-1
//!   2. Layout — compute actual container sizes, cache them
//!   3. If any container size changed → mark descendants dirty → repeat
//! ```
//!
//! At most 2 passes per frame. No infinite loop.
//!
//! The `ContainerLookup` trait is the bridge: the DOM/layout layer implements
//! it, the style resolver calls it during `@container` evaluation.

use kozan_atom::Atom;
use kozan_css::rules::container::{ContainerCondition, ContainerSizeFeature};
use kozan_css::rules::media::{MediaFeatureValue, RangeOp};
use kozan_css::LengthUnit;

/// Container dimensions provided by the layout layer.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContainerSize {
    /// Container's width in px (for `width` / `inline-size` in horizontal-tb).
    pub width: f32,
    /// Container's height in px (for `height` / `block-size` in horizontal-tb).
    pub height: f32,
}

/// Trait for looking up container sizes during style resolution.
///
/// The DOM/layout layer implements this. It returns cached sizes from the
/// previous layout pass. On first render, all containers return `None`
/// (size unknown → `@container` conditions don't match → conservative).
pub trait ContainerLookup {
    /// Find the nearest container ancestor for an element.
    ///
    /// `name`: optional container name from `@container name (...)`.
    /// If `None`, find the nearest ancestor with `container-type` set.
    ///
    /// Returns the container's cached size, or `None` if no container found
    /// or container hasn't been laid out yet.
    fn find_container(&self, element_id: u64, name: Option<&Atom>) -> Option<ContainerSize>;
}

impl PartialEq for ContainerSize {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height
    }
}

/// Cache of container sizes from the previous layout pass.
///
/// After each layout pass, the DOM/layout layer updates this cache with the
/// actual sizes of all container elements. Before the next style pass,
/// `check_changes()` compares against the new sizes and returns the IDs of
/// containers whose size changed — their descendants need restyle.
///
/// ```text
/// Frame N:
///   1. Style: ContainerLookup reads from ContainerSizeCache (frame N-1 sizes)
///   2. Layout: compute actual sizes, call cache.update(id, new_size)
///   3. cache.check_changes() → list of changed container IDs
///   4. Mark descendants of changed containers for restyle
///   5. If any changed → repeat style+layout (at most once more)
/// ```
pub struct ContainerSizeCache {
    /// Current sizes (from this frame's layout).
    current: kozan_selector::fxhash::FxHashMap<u64, ContainerSize>,
    /// Previous sizes (from last frame's layout — used during style resolution).
    previous: kozan_selector::fxhash::FxHashMap<u64, ContainerSize>,
}

impl ContainerSizeCache {
    pub fn new() -> Self {
        Self {
            current: kozan_selector::fxhash::FxHashMap::default(),
            previous: kozan_selector::fxhash::FxHashMap::default(),
        }
    }

    /// Record a container's size after layout. Called by the layout layer.
    pub fn update(&mut self, container_id: u64, size: ContainerSize) {
        self.current.insert(container_id, size);
    }

    /// Get a container's size from the previous layout pass.
    /// Used by `ContainerLookup` during style resolution.
    pub fn get_previous(&self, container_id: u64) -> Option<ContainerSize> {
        self.previous.get(&container_id).copied()
    }

    /// Compare current sizes with previous. Returns IDs of containers whose
    /// size changed — their descendants need restyle.
    ///
    /// Also includes containers that are new (didn't exist last frame) or
    /// removed (existed last frame but not this frame).
    pub fn changed_containers(&self) -> Vec<u64> {
        let mut changed = Vec::new();

        // Check for changed or new containers.
        for (&id, &current_size) in &self.current {
            match self.previous.get(&id) {
                Some(&prev_size) if prev_size == current_size => {}
                _ => changed.push(id), // new or changed
            }
        }

        // Check for removed containers.
        for &id in self.previous.keys() {
            if !self.current.contains_key(&id) {
                changed.push(id);
            }
        }

        changed
    }

    /// Advance to next frame: current becomes previous, current is cleared.
    /// Call after processing all container changes for this frame.
    pub fn advance_frame(&mut self) {
        std::mem::swap(&mut self.previous, &mut self.current);
        self.current.clear();
    }

    /// Whether any container size changed between frames.
    pub fn has_changes(&self) -> bool {
        if self.current.len() != self.previous.len() {
            return true;
        }
        self.current.iter().any(|(id, size)| {
            self.previous.get(id).map_or(true, |prev| prev != size)
        })
    }
}

impl Default for ContainerSizeCache {
    fn default() -> Self {
        Self::new()
    }
}

/// No-op container lookup — all container queries evaluate to false.
/// Used when no layout has run yet (first render) or containers aren't supported.
pub struct NoContainers;

impl ContainerLookup for NoContainers {
    fn find_container(&self, _element_id: u64, _name: Option<&Atom>) -> Option<ContainerSize> {
        None
    }
}

/// Context for resolving units in container query conditions.
///
/// Provides the actual font-size, root font-size, viewport dimensions,
/// and **container dimensions** needed to resolve em/rem/vw/vh/cqw/cqh
/// in `@container (min-width: 48em)`.
#[derive(Clone, Copy, Debug)]
pub struct ContainerEvalContext {
    pub font_size: f32,
    pub root_font_size: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// The container's own width in px — used for `cqw`/`cqi` units.
    /// Set to the container being queried, NOT the viewport.
    pub container_width: f32,
    /// The container's own height in px — used for `cqh`/`cqb` units.
    pub container_height: f32,
}

impl Default for ContainerEvalContext {
    fn default() -> Self {
        use crate::resolver::INITIAL_FONT_SIZE_PX;
        Self {
            font_size: INITIAL_FONT_SIZE_PX,
            root_font_size: INITIAL_FONT_SIZE_PX,
            viewport_width: 0.0,
            viewport_height: 0.0,
            container_width: 0.0,
            container_height: 0.0,
        }
    }
}

/// Evaluate a `@container` condition against a container's size.
///
/// Returns `true` if the condition is met, `false` if not or if no container found.
pub fn evaluate_container_condition(
    condition: &ContainerCondition,
    size: &ContainerSize,
    ctx: &ContainerEvalContext,
) -> bool {
    match condition {
        ContainerCondition::Feature(feature) => evaluate_feature(feature, size, ctx),
        ContainerCondition::Not(inner) => !evaluate_container_condition(inner, size, ctx),
        ContainerCondition::And(conditions) => {
            conditions.iter().all(|c| evaluate_container_condition(c, size, ctx))
        }
        ContainerCondition::Or(conditions) => {
            conditions.iter().any(|c| evaluate_container_condition(c, size, ctx))
        }
    }
}

/// Evaluate a single container size feature: `(width >= 768px)`.
fn evaluate_feature(feature: &ContainerSizeFeature, size: &ContainerSize, ctx: &ContainerEvalContext) -> bool {
    let actual = match feature.name.as_ref() {
        "width" | "inline-size" => size.width,
        "height" | "block-size" => size.height,
        "min-width" => return compare(size.width, RangeOp::Ge, &feature.value, ctx),
        "max-width" => return compare(size.width, RangeOp::Le, &feature.value, ctx),
        "min-height" => return compare(size.height, RangeOp::Ge, &feature.value, ctx),
        "max-height" => return compare(size.height, RangeOp::Le, &feature.value, ctx),
        "min-inline-size" => return compare(size.width, RangeOp::Ge, &feature.value, ctx),
        "max-inline-size" => return compare(size.width, RangeOp::Le, &feature.value, ctx),
        "min-block-size" => return compare(size.height, RangeOp::Ge, &feature.value, ctx),
        "max-block-size" => return compare(size.height, RangeOp::Le, &feature.value, ctx),
        // aspect-ratio: width / height compared as a ratio
        "aspect-ratio" => {
            if size.height == 0.0 { return false; }
            return compare_ratio(size.width / size.height, feature.op, &feature.value);
        }
        "min-aspect-ratio" => {
            if size.height == 0.0 { return false; }
            return compare_ratio(size.width / size.height, RangeOp::Ge, &feature.value);
        }
        "max-aspect-ratio" => {
            if size.height == 0.0 { return false; }
            return compare_ratio(size.width / size.height, RangeOp::Le, &feature.value);
        }
        // orientation: portrait | landscape
        "orientation" => return evaluate_orientation(size, &feature.value),
        _ => return false,
    };
    compare(actual, feature.op, &feature.value, ctx)
}

/// Compare an actual ratio against a feature value (number or ratio `w/h`).
fn compare_ratio(actual: f32, op: RangeOp, value: &MediaFeatureValue) -> bool {
    let target = match value {
        MediaFeatureValue::Number(n) => *n,
        MediaFeatureValue::Ratio(w, h) => {
            if *h == 0 { return false; }
            *w as f32 / *h as f32
        }
        _ => return false,
    };
    match op {
        RangeOp::Eq => (actual - target).abs() < 0.001,
        RangeOp::Gt => actual > target,
        RangeOp::Ge => actual >= target,
        RangeOp::Lt => actual < target,
        RangeOp::Le => actual <= target,
    }
}

/// Evaluate `orientation: portrait | landscape`.
fn evaluate_orientation(size: &ContainerSize, value: &MediaFeatureValue) -> bool {
    match value {
        MediaFeatureValue::Ident(ident) => match ident.as_ref() {
            "portrait" => size.height >= size.width,
            "landscape" => size.width > size.height,
            _ => false,
        },
        _ => false,
    }
}

/// Compare an actual px value against a feature value using a range operator.
fn compare(actual: f32, op: RangeOp, value: &MediaFeatureValue, ctx: &ContainerEvalContext) -> bool {
    let target = match value {
        MediaFeatureValue::Length(v, unit) => resolve_length(*v, unit, ctx),
        MediaFeatureValue::Number(n) => *n,
        _ => return false,
    };
    match op {
        RangeOp::Eq => (actual - target).abs() < 0.01,
        RangeOp::Gt => actual > target,
        RangeOp::Ge => actual >= target,
        RangeOp::Lt => actual < target,
        RangeOp::Le => actual <= target,
    }
}

/// Resolve a CSS length value to px using the evaluation context.
fn resolve_length(value: f32, unit: &LengthUnit, ctx: &ContainerEvalContext) -> f32 {
    let vw = ctx.viewport_width;
    let vh = ctx.viewport_height;
    match unit {
        // Absolute
        LengthUnit::Px => value,
        LengthUnit::Cm => value * 96.0 / 2.54,
        LengthUnit::Mm => value * 96.0 / 25.4,
        LengthUnit::In => value * 96.0,
        LengthUnit::Pt => value * 96.0 / 72.0,
        LengthUnit::Pc => value * 96.0 / 6.0,
        // Font-relative
        LengthUnit::Em | LengthUnit::Ch | LengthUnit::Ex => value * ctx.font_size,
        LengthUnit::Rem => value * ctx.root_font_size,
        // Viewport
        LengthUnit::Vw | LengthUnit::Svw | LengthUnit::Lvw | LengthUnit::Dvw => value * vw / 100.0,
        LengthUnit::Vh | LengthUnit::Svh | LengthUnit::Lvh | LengthUnit::Dvh => value * vh / 100.0,
        LengthUnit::Vmin => value * vw.min(vh) / 100.0,
        LengthUnit::Vmax => value * vw.max(vh) / 100.0,
        LengthUnit::Vi => value * vw / 100.0,
        LengthUnit::Vb => value * vh / 100.0,
        // Container query units — resolve against the container being queried.
        LengthUnit::Cqw | LengthUnit::Cqi => value * ctx.container_width / 100.0,
        LengthUnit::Cqh | LengthUnit::Cqb => value * ctx.container_height / 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(w: f32, h: f32) -> ContainerSize {
        ContainerSize { width: w, height: h }
    }

    fn ctx() -> ContainerEvalContext {
        ContainerEvalContext {
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 1024.0,
            viewport_height: 768.0,
            container_width: 0.0,
            container_height: 0.0,
        }
    }

    fn feature(name: &str, op: RangeOp, px: f32) -> ContainerCondition {
        ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from(name),
            op,
            value: MediaFeatureValue::Length(px, LengthUnit::Px),
        })
    }

    #[test]
    fn width_ge_matches() {
        let s = size(800.0, 600.0);
        assert!(evaluate_container_condition(&feature("width", RangeOp::Ge, 768.0), &s, &ctx()));
    }

    #[test]
    fn width_ge_fails() {
        let s = size(500.0, 600.0);
        assert!(!evaluate_container_condition(&feature("width", RangeOp::Ge, 768.0), &s, &ctx()));
    }

    #[test]
    fn height_lt_matches() {
        let s = size(800.0, 400.0);
        assert!(evaluate_container_condition(&feature("height", RangeOp::Lt, 600.0), &s, &ctx()));
    }

    #[test]
    fn min_width_alias() {
        let s = size(800.0, 600.0);
        assert!(evaluate_container_condition(&feature("min-width", RangeOp::Ge, 768.0), &s, &ctx()));
    }

    #[test]
    fn not_condition() {
        let s = size(500.0, 600.0);
        let cond = ContainerCondition::Not(Box::new(feature("width", RangeOp::Ge, 768.0)));
        assert!(evaluate_container_condition(&cond, &s, &ctx()));
    }

    #[test]
    fn and_both_true() {
        let s = size(800.0, 600.0);
        let cond = ContainerCondition::And(smallvec::smallvec![
            Box::new(feature("width", RangeOp::Ge, 768.0)),
            Box::new(feature("height", RangeOp::Ge, 400.0)),
        ]);
        assert!(evaluate_container_condition(&cond, &s, &ctx()));
    }

    #[test]
    fn and_one_false() {
        let s = size(800.0, 300.0);
        let cond = ContainerCondition::And(smallvec::smallvec![
            Box::new(feature("width", RangeOp::Ge, 768.0)),
            Box::new(feature("height", RangeOp::Ge, 400.0)),
        ]);
        assert!(!evaluate_container_condition(&cond, &s, &ctx()));
    }

    #[test]
    fn or_one_true() {
        let s = size(500.0, 600.0);
        let cond = ContainerCondition::Or(smallvec::smallvec![
            Box::new(feature("width", RangeOp::Ge, 768.0)),
            Box::new(feature("height", RangeOp::Ge, 400.0)),
        ]);
        assert!(evaluate_container_condition(&cond, &s, &ctx()));
    }

    #[test]
    fn cqw_resolves_against_container_not_viewport() {
        let s = size(800.0, 600.0);
        let c = ContainerEvalContext {
            container_width: 400.0,
            container_height: 300.0,
            viewport_width: 1024.0,
            viewport_height: 768.0,
            ..ctx()
        };
        // 50cqw = 400 * 50/100 = 200px. Container is 800px → matches (800 >= 200).
        let cond = ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from("width"),
            op: RangeOp::Ge,
            value: MediaFeatureValue::Length(50.0, LengthUnit::Cqw),
        });
        assert!(evaluate_container_condition(&cond, &s, &c));

        // 250cqw = 400 * 250/100 = 1000px. Container is 800px → doesn't match.
        let cond2 = ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from("width"),
            op: RangeOp::Ge,
            value: MediaFeatureValue::Length(250.0, LengthUnit::Cqw),
        });
        assert!(!evaluate_container_condition(&cond2, &s, &c));
    }

    #[test]
    fn cqh_resolves_against_container_not_viewport() {
        let s = size(800.0, 600.0);
        let c = ContainerEvalContext {
            container_width: 400.0,
            container_height: 300.0,
            ..ctx()
        };
        // 100cqh = 300 * 100/100 = 300px. Container is 600px → matches.
        let cond = ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from("height"),
            op: RangeOp::Ge,
            value: MediaFeatureValue::Length(100.0, LengthUnit::Cqh),
        });
        assert!(evaluate_container_condition(&cond, &s, &c));
    }

    #[test]
    fn no_containers_returns_none() {
        let lookup = NoContainers;
        assert!(lookup.find_container(1, None).is_none());
    }

    #[test]
    fn eq_comparison() {
        let s = size(768.0, 600.0);
        assert!(evaluate_container_condition(&feature("width", RangeOp::Eq, 768.0), &s, &ctx()));
        assert!(!evaluate_container_condition(&feature("width", RangeOp::Eq, 769.0), &s, &ctx()));
    }

    #[test]
    fn em_unit_resolves_with_font_size() {
        let s = size(800.0, 600.0);
        // 48em * 16px = 768px. Container is 800px wide → matches.
        let cond = ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from("width"),
            op: RangeOp::Ge,
            value: MediaFeatureValue::Length(48.0, LengthUnit::Em),
        });
        assert!(evaluate_container_condition(&cond, &s, &ctx()));
    }

    #[test]
    fn em_unit_uses_actual_font_size() {
        let s = size(800.0, 600.0);
        let big_font = ContainerEvalContext { font_size: 32.0, ..ctx() };
        // 48em * 32px = 1536px. Container is 800px → does NOT match.
        let cond = ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from("width"),
            op: RangeOp::Ge,
            value: MediaFeatureValue::Length(48.0, LengthUnit::Em),
        });
        assert!(!evaluate_container_condition(&cond, &s, &big_font));
    }

    #[test]
    fn vw_unit_resolves_with_viewport() {
        let s = size(800.0, 600.0);
        // 50vw = 1024 * 50/100 = 512px. Container is 800px → matches.
        let cond = ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from("width"),
            op: RangeOp::Ge,
            value: MediaFeatureValue::Length(50.0, LengthUnit::Vw),
        });
        assert!(evaluate_container_condition(&cond, &s, &ctx()));
    }

    // ─── aspect-ratio ────────────────────────────────────

    fn ratio_feature(name: &str, op: RangeOp, w: u32, h: u32) -> ContainerCondition {
        ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from(name),
            op,
            value: MediaFeatureValue::Ratio(w, h),
        })
    }

    #[test]
    fn aspect_ratio_matches_landscape() {
        // 800/600 = 1.333, 16/9 = 1.777 → not >=
        let s = size(800.0, 600.0);
        assert!(!evaluate_container_condition(
            &ratio_feature("aspect-ratio", RangeOp::Ge, 16, 9), &s, &ctx()
        ));
        // 4/3 = 1.333 → eq
        assert!(evaluate_container_condition(
            &ratio_feature("aspect-ratio", RangeOp::Eq, 4, 3), &s, &ctx()
        ));
    }

    #[test]
    fn aspect_ratio_min_max() {
        let s = size(1920.0, 1080.0); // 16:9 = 1.777
        // min-aspect-ratio: 16/10 (1.6) → 1.777 >= 1.6 → true
        assert!(evaluate_container_condition(
            &ratio_feature("min-aspect-ratio", RangeOp::Ge, 16, 10), &s, &ctx()
        ));
        // max-aspect-ratio: 4/3 (1.333) → 1.777 <= 1.333 → false
        assert!(!evaluate_container_condition(
            &ratio_feature("max-aspect-ratio", RangeOp::Ge, 4, 3), &s, &ctx()
        ));
    }

    #[test]
    fn aspect_ratio_zero_height() {
        let s = size(800.0, 0.0);
        assert!(!evaluate_container_condition(
            &ratio_feature("aspect-ratio", RangeOp::Ge, 1, 1), &s, &ctx()
        ));
    }

    // ─── orientation ─────────────────────────────────────

    fn orientation_feature(value: &str) -> ContainerCondition {
        ContainerCondition::Feature(ContainerSizeFeature {
            name: Atom::from("orientation"),
            op: RangeOp::Eq,
            value: MediaFeatureValue::Ident(Atom::from(value)),
        })
    }

    #[test]
    fn orientation_landscape() {
        let s = size(800.0, 600.0);
        assert!(evaluate_container_condition(&orientation_feature("landscape"), &s, &ctx()));
        assert!(!evaluate_container_condition(&orientation_feature("portrait"), &s, &ctx()));
    }

    #[test]
    fn orientation_portrait() {
        let s = size(600.0, 800.0);
        assert!(evaluate_container_condition(&orientation_feature("portrait"), &s, &ctx()));
        assert!(!evaluate_container_condition(&orientation_feature("landscape"), &s, &ctx()));
    }

    #[test]
    fn orientation_square_is_portrait() {
        // CSS spec: square is portrait (height >= width)
        let s = size(600.0, 600.0);
        assert!(evaluate_container_condition(&orientation_feature("portrait"), &s, &ctx()));
        assert!(!evaluate_container_condition(&orientation_feature("landscape"), &s, &ctx()));
    }
}
