//! Shared box model utilities — stacking context detection.
//!
//! Chrome equivalent: helpers in `PaintLayer` and `ComputedStyleUtils`.

use style::properties::ComputedValues;

/// Check if an element establishes a stacking context.
///
/// Chrome: `PaintLayer::ComputeStackingContext()`.
pub(crate) fn is_stacking_context(style: &ComputedValues) -> bool {
    use style::computed_values::position::T as Position;

    let position = style.clone_position();
    if matches!(
        position,
        Position::Relative | Position::Absolute | Position::Fixed
    ) {
        // z-index: auto does NOT establish a stacking context for positioned elements
        // (only an integer value does). Stylo's clone_z_index returns ZIndex.
        let z_index = style.clone_z_index();
        if !z_index.is_auto() {
            return true;
        }
    }
    if style.get_effects().opacity != 1.0 {
        return true;
    }
    if !style.get_box().transform.0.is_empty() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacking_context_default_is_false() {
        let style = crate::styling::initial_values_arc();
        assert!(!is_stacking_context(&style));
    }
}
