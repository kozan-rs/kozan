//! `DomTraversal` implementation for Stylo style recalculation.

use style::context::{SharedStyleContext, StyleContext};
use style::dom::{TElement, TNode};
use style::traversal::{DomTraversal, PerLevelTraversalData, recalc_style_at};

/// Drives Stylo's style recalculation over the DOM.
pub(crate) struct RecalcStyle<'a> {
    context: SharedStyleContext<'a>,
}

impl<'a> RecalcStyle<'a> {
    pub fn new(context: SharedStyleContext<'a>) -> Self {
        Self { context }
    }
}

impl<E: TElement> DomTraversal<E> for RecalcStyle<'_> {
    fn process_preorder<F: FnMut(E::ConcreteNode)>(
        &self,
        traversal_data: &PerLevelTraversalData,
        context: &mut StyleContext<E>,
        node: E::ConcreteNode,
        note_child: F,
    ) {
        if let Some(el) = node.as_element() {
            let mut data = unsafe { el.ensure_data() };
            recalc_style_at(self, traversal_data, context, el, &mut data, note_child);
            unsafe { el.unset_dirty_descendants() };
        }
    }

    fn process_postorder(&self, _: &mut StyleContext<E>, _: E::ConcreteNode) {
        // No postorder processing needed.
    }

    fn needs_postorder_traversal() -> bool {
        false
    }

    fn shared_context(&self) -> &SharedStyleContext<'_> {
        &self.context
    }
}
