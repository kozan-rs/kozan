//! Focus controller — window-level focus state.
//!
//! Chrome: `FocusController` on `Page`. DOM-level focus queries
//! (is_focusable, tab_order) live on `Document`.

use crate::dom::document::Document;

/// Window-level focus state. DOM focus queries and navigation
/// algorithms live on `Document`; this struct only tracks whether
/// the window is active and focused.
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

    /// Tab/Shift+Tab navigation — delegates to Document's tab_order.
    pub fn advance(&self, doc: &Document, forward: bool) {
        let tab_order = doc.tab_order();
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
}
