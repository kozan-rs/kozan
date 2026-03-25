//! Focus controller — focus policy and navigation algorithms.
//!
//! Chrome: `FocusController` on `Page`, not on `Document`.

use crate::dom::document::Document;
use crate::id::INVALID;
use crate::scroll::ScrollTree;

/// HTML §6.6 focus management. Stateless about which element is
/// focused — reads `doc.focused_element()` instead. Provides focus
/// queries (is_focusable, tab_order) and Tab navigation (advance).
pub(crate) struct FocusController {
    is_active: bool,
    is_focused: bool,
}

impl FocusController {
    pub fn new() -> Self {
        Self {
            is_active: false,
            is_focused: false,
        }
    }

    #[allow(dead_code)] // Platform reads this when routing activation events.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    #[allow(dead_code)] // Platform reads this when routing keyboard events.
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    /// On blur: clears element focus via Document.
    pub fn set_focused(&mut self, doc: &Document, focused: bool) {
        self.is_focused = focused;
        if !focused {
            doc.set_focused_element(None, false);
        }
    }

    /// Tab/Shift+Tab navigation: builds tab order, finds next, moves focus.
    pub fn advance(&self, doc: &Document, forward: bool) {
        let tab_order = Self::tab_order(doc);
        if tab_order.is_empty() {
            return;
        }

        let current_pos = doc
            .focused_element()
            .and_then(|id| tab_order.iter().position(|&idx| idx == id.index()));

        let next_idx = match current_pos {
            Some(pos) => {
                if forward {
                    (pos + 1) % tab_order.len()
                } else {
                    (pos + tab_order.len() - 1) % tab_order.len()
                }
            }
            None => {
                if forward {
                    0
                } else {
                    tab_order.len() - 1
                }
            }
        };

        doc.set_focused_element(Some(tab_order[next_idx]), true);
    }

    /// HTML §6.6.3 — focusable when natively interactive,
    /// has tabindex, OR is a scrollable overflow region.
    pub fn is_focusable(doc: &Document, index: u32) -> bool {
        let has_tabindex = doc
            .element_data
            .get(index)
            .is_some_and(|ed| ed.attributes.get("tabindex").is_some());
        let native = doc
            .meta
            .get(index)
            .is_some_and(|m| m.flags().is_focusable());
        let scrollable = Self::is_scrollable_region(doc, index);

        if !native && !has_tabindex && !scrollable {
            return false;
        }

        if native {
            if let Some(ed) = doc.element_data.get(index) {
                if !ed.element_state.contains(style_dom::ElementState::ENABLED) {
                    return false;
                }
            }
        }

        if let Some(style) = doc.computed_style(index) {
            if style.get_box().display.is_none() {
                return false;
            }
        }

        true
    }

    /// Explicit tabindex wins; natively focusable and scrollable default to 0.
    pub fn effective_tab_index(doc: &Document, index: u32) -> i32 {
        if let Some(ed) = doc.element_data.get(index) {
            if let Some(val) = ed.attributes.get("tabindex") {
                return val.parse().unwrap_or(0);
            }
        }
        let native = doc
            .meta
            .get(index)
            .is_some_and(|m| m.flags().is_focusable());
        if native || Self::is_scrollable_region(doc, index) {
            0
        } else {
            -1
        }
    }

    /// Walk from `index` up ancestors, return first focusable node.
    pub fn find_focusable_ancestor(doc: &Document, index: u32) -> Option<u32> {
        let mut current = index;
        loop {
            if Self::is_focusable(doc, current) {
                return Some(current);
            }
            match doc.tree.get(current) {
                Some(td) if td.parent != INVALID => current = td.parent,
                _ => return None,
            }
        }
    }

    /// W3C sequential focus navigation order.
    pub fn tab_order(doc: &Document) -> Vec<u32> {
        let mut positive: Vec<(i32, usize, u32)> = Vec::new();
        let mut zero: Vec<u32> = Vec::new();
        let mut order = 0usize;

        let mut stack = vec![doc.root];
        while let Some(index) = stack.pop() {
            if Self::is_focusable(doc, index) {
                let ti = Self::effective_tab_index(doc, index);
                if ti > 0 {
                    positive.push((ti, order, index));
                } else if ti == 0 {
                    zero.push(index);
                }
                order += 1;
            }

            if let Some(td) = doc.tree.get(index) {
                let mut child = td.last_child;
                while child != INVALID {
                    stack.push(child);
                    child = doc.tree.get(child).map_or(INVALID, |t| t.prev_sibling);
                }
            }
        }

        positive.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut result: Vec<u32> = positive.into_iter().map(|(_, _, idx)| idx).collect();
        result.extend(zero);
        result
    }

    /// Keyboard scroll target: walk from focused element up to find scrollable ancestor.
    pub fn scroll_target(doc: &Document, scroll_tree: &ScrollTree) -> Option<u32> {
        let focused_idx = doc.focused_element()?.index();
        let mut current = focused_idx;
        loop {
            if scroll_tree.contains(current) {
                return Some(current);
            }
            let handle = doc.handle_for_index(current)?;
            current = handle.parent()?.raw().index();
        }
    }

    fn is_scrollable_region(doc: &Document, index: u32) -> bool {
        use style::computed_values::overflow_x::T;
        let Some(style) = doc.computed_style(index) else {
            return false;
        };
        matches!(style.clone_overflow_x(), T::Scroll | T::Auto)
            || matches!(style.clone_overflow_y(), T::Scroll | T::Auto)
    }
}
