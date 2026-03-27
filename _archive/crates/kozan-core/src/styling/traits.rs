//! Stylo trait implementations — bridges `KozanNode` to Stylo's DOM traits.
//!
//! All hot-path methods read from `ElementData` via direct pointer access.
//! `ElementData` lives in `Storage<ElementData>` — one array lookup, all fields.
//! No `doc().read()` closures in selector matching. No `HashMap`.

use selectors::OpaqueElement;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::{BLOOM_HASH_MASK, BloomFilter};
use selectors::matching::{ElementSelectorFlags, MatchingContext, QuirksMode, VisitedHandlingMode};
use selectors::sink::Push;
use servo_arc::ArcBorrow;
use style::applicable_declarations::ApplicableDeclarationBlock;
use style::bloom::each_relevant_element_hash;
use style::context::SharedStyleContext;
use style::data::{ElementDataMut, ElementDataRef};
use style::dom::{
    AttributeProvider, LayoutIterator, NodeInfo, OpaqueNode, TDocument, TElement, TNode,
    TShadowRoot,
};
use style::properties::PropertyDeclarationBlock;
use style::selector_parser::{AttrValue, Lang};
use style::selector_parser::{NonTSPseudoClass, PseudoElement, SelectorImpl};
use style::shared_lock::{Locked, SharedRwLock};
use style::values::AtomIdent;
use style::{Atom, LocalName, Namespace};
use style_dom::ElementState;

use super::node::{KozanNode, doc};
use crate::dom::element_data::ElementData;

/// Child iterator for Stylo's `traversal_children()`.
pub(crate) struct KozanChildIter {
    current: Option<KozanNode>,
}

impl Iterator for KozanChildIter {
    type Item = KozanNode;
    fn next(&mut self) -> Option<KozanNode> {
        let node = self.current?;
        self.current = node.next_sibling();
        Some(node)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helper: direct ElementData access via pointer (no closure overhead)
// ═══════════════════════════════════════════════════════════════════

impl KozanNode {
    /// Direct access to `ElementData`. O(1) array lookup, no closure.
    /// Safe during traversal: single-threaded, document stable.
    #[inline]
    fn ed(&self) -> Option<&ElementData> {
        unsafe { (*doc().as_ptr()).element_data_by_index(self.idx()) }
    }

    /// Mutable access to `ElementData`.
    #[inline]
    #[allow(clippy::mut_from_ref)] // Intentional: interior-mutable doc accessed through shared ref during style traversal
    fn ed_mut(&self) -> Option<&mut ElementData> {
        unsafe { (*doc().as_ptr()).element_data_by_index_mut(self.idx()) }
    }
}

// ═══════════════════════════════════════════════════════════════════
// NodeInfo
// ═══════════════════════════════════════════════════════════════════

impl NodeInfo for KozanNode {
    fn is_element(&self) -> bool {
        self.is_element()
    }
    fn is_text_node(&self) -> bool {
        self.is_text()
    }
}

// ═══════════════════════════════════════════════════════════════════
// TDocument
// ═══════════════════════════════════════════════════════════════════

impl TDocument for KozanNode {
    type ConcreteNode = KozanNode;
    fn as_node(&self) -> Self::ConcreteNode {
        *self
    }
    fn is_html_document(&self) -> bool {
        true
    }
    fn quirks_mode(&self) -> QuirksMode {
        QuirksMode::NoQuirks
    }

    fn shared_lock(&self) -> &SharedRwLock {
        unsafe { (*doc().as_ptr()).shared_lock() }
    }
}

// ═══════════════════════════════════════════════════════════════════
// TShadowRoot
// ═══════════════════════════════════════════════════════════════════

impl TShadowRoot for KozanNode {
    type ConcreteNode = KozanNode;
    fn as_node(&self) -> Self::ConcreteNode {
        *self
    }
    fn host(&self) -> <Self::ConcreteNode as TNode>::ConcreteElement {
        unreachable!("no shadow DOM")
    }
    fn style_data<'a>(&self) -> Option<&'a style::stylist::CascadeData>
    where
        Self: 'a,
    {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════
// TNode
// ═══════════════════════════════════════════════════════════════════

impl TNode for KozanNode {
    type ConcreteElement = KozanNode;
    type ConcreteDocument = KozanNode;
    type ConcreteShadowRoot = KozanNode;

    fn parent_node(&self) -> Option<Self> {
        KozanNode::parent_node(self)
    }
    fn first_child(&self) -> Option<Self> {
        KozanNode::first_child(self)
    }
    fn last_child(&self) -> Option<Self> {
        KozanNode::last_child(self)
    }
    fn prev_sibling(&self) -> Option<Self> {
        KozanNode::prev_sibling(self)
    }
    fn next_sibling(&self) -> Option<Self> {
        KozanNode::next_sibling(self)
    }

    fn owner_doc(&self) -> Self::ConcreteDocument {
        let mut c = *self;
        while let Some(p) = c.parent_node() {
            c = p;
        }
        c
    }

    fn is_in_document(&self) -> bool {
        let mut c = *self;
        loop {
            if c.is_document() {
                return true;
            }
            match c.parent_node() {
                Some(p) => c = p,
                None => return false,
            }
        }
    }

    fn traversal_parent(&self) -> Option<Self::ConcreteElement> {
        self.parent_node().and_then(|n| n.as_element())
    }

    fn opaque(&self) -> OpaqueNode {
        OpaqueNode(self.idx() as usize)
    }
    fn debug_id(self) -> usize {
        self.idx() as usize
    }

    fn as_element(&self) -> Option<Self::ConcreteElement> {
        if self.is_element() { Some(*self) } else { None }
    }
    fn as_document(&self) -> Option<Self::ConcreteDocument> {
        if self.is_document() {
            Some(*self)
        } else {
            None
        }
    }
    fn as_shadow_root(&self) -> Option<Self::ConcreteShadowRoot> {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════
// selectors::Element — all reads from ElementData, no closures
// ═══════════════════════════════════════════════════════════════════

impl selectors::Element for KozanNode {
    type Impl = SelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        // idx()+1 is always ≥1, so the pointer is never null.
        let ptr = std::ptr::NonNull::new((self.idx() as usize + 1) as *mut ())
            .expect("idx() + 1 is always non-zero");
        OpaqueElement::from_non_null_ptr(ptr)
    }

    fn parent_element(&self) -> Option<Self> {
        KozanNode::parent_node(self).filter(|n| n.is_element())
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }
    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        let mut n = KozanNode::prev_sibling(self)?;
        loop {
            if n.is_element() {
                return Some(n);
            }
            n = n.prev_sibling()?;
        }
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let mut n = KozanNode::next_sibling(self)?;
        loop {
            if n.is_element() {
                return Some(n);
            }
            n = n.next_sibling()?;
        }
    }

    fn first_element_child(&self) -> Option<Self> {
        let mut n = KozanNode::first_child(self)?;
        loop {
            if n.is_element() {
                return Some(n);
            }
            n = n.next_sibling()?;
        }
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.is_element()
    }

    fn has_local_name(
        &self,
        name: &<SelectorImpl as selectors::SelectorImpl>::BorrowedLocalName,
    ) -> bool {
        self.ed().is_some_and(|d| *d.local_name == *name)
    }

    fn has_namespace(
        &self,
        ns: &<SelectorImpl as selectors::SelectorImpl>::BorrowedNamespaceUrl,
    ) -> bool {
        self.ed().is_some_and(|d| *d.namespace == *ns)
    }

    fn is_same_type(&self, other: &Self) -> bool {
        match (self.ed(), other.ed()) {
            (Some(a), Some(b)) => a.local_name == b.local_name && a.namespace == b.namespace,
            _ => false,
        }
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&<SelectorImpl as selectors::SelectorImpl>::NamespaceUrl>,
        local_name: &<SelectorImpl as selectors::SelectorImpl>::LocalName,
        operation: &AttrSelectorOperation<&<SelectorImpl as selectors::SelectorImpl>::AttrValue>,
    ) -> bool {
        match *ns {
            NamespaceConstraint::Specific(ns) if !ns.is_empty() => return false,
            _ => {}
        }
        let ed = match self.ed() {
            Some(d) => d,
            None => return false,
        };
        match ed.attributes.get(local_name.as_ref()) {
            Some(v) => operation.eval_str(v),
            None => false,
        }
    }

    fn match_non_ts_pseudo_class(
        &self,
        pc: &NonTSPseudoClass,
        _: &mut MatchingContext<SelectorImpl>,
    ) -> bool {
        let state = self.ed().map_or(ElementState::empty(), |d| d.element_state);
        match *pc {
            NonTSPseudoClass::Hover => state.contains(ElementState::HOVER),
            NonTSPseudoClass::Active => state.contains(ElementState::ACTIVE),
            NonTSPseudoClass::Focus => state.contains(ElementState::FOCUS),
            NonTSPseudoClass::FocusWithin => state.contains(ElementState::FOCUS_WITHIN),
            NonTSPseudoClass::FocusVisible => state.contains(ElementState::FOCUSRING),
            NonTSPseudoClass::Enabled => state.contains(ElementState::ENABLED),
            NonTSPseudoClass::Disabled => state.contains(ElementState::DISABLED),
            NonTSPseudoClass::Checked => state.contains(ElementState::CHECKED),
            NonTSPseudoClass::Link | NonTSPseudoClass::AnyLink => self
                .ed()
                .is_some_and(|d| d.tag_name == "a" && d.attributes.get("href").is_some()),
            NonTSPseudoClass::Visited => false,
            NonTSPseudoClass::Defined => true,
            _ => false,
        }
    }

    fn match_pseudo_element(
        &self,
        _: &PseudoElement,
        _: &mut MatchingContext<SelectorImpl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, flags: ElementSelectorFlags) {
        if let Some(d) = self.ed() {
            d.selector_flags.set(d.selector_flags.get() | flags);
        }
    }

    fn is_link(&self) -> bool {
        self.ed()
            .is_some_and(|d| d.tag_name == "a" && d.attributes.get("href").is_some())
    }
    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(
        &self,
        id: &<SelectorImpl as selectors::SelectorImpl>::Identifier,
        cs: CaseSensitivity,
    ) -> bool {
        self.ed()
            .and_then(|d| d.id.as_ref())
            .is_some_and(|a| cs.eq(a.as_ref().as_bytes(), id.as_ref().as_bytes()))
    }

    fn has_class(
        &self,
        name: &<SelectorImpl as selectors::SelectorImpl>::Identifier,
        cs: CaseSensitivity,
    ) -> bool {
        self.ed().is_some_and(|d| {
            d.classes
                .iter()
                .any(|c| cs.eq(c.as_ref().as_bytes(), name.as_ref().as_bytes()))
        })
    }

    fn has_custom_state(&self, _: &<SelectorImpl as selectors::SelectorImpl>::Identifier) -> bool {
        false
    }
    fn imported_part(
        &self,
        _: &<SelectorImpl as selectors::SelectorImpl>::Identifier,
    ) -> Option<<SelectorImpl as selectors::SelectorImpl>::Identifier> {
        None
    }
    fn is_part(&self, _: &<SelectorImpl as selectors::SelectorImpl>::Identifier) -> bool {
        false
    }
    fn is_empty(&self) -> bool {
        KozanNode::first_child(self).is_none()
    }

    fn is_root(&self) -> bool {
        KozanNode::parent_node(self).is_some_and(|p| p.is_document())
    }

    fn add_element_unique_hashes(&self, filter: &mut BloomFilter) -> bool {
        each_relevant_element_hash(*self, |hash| filter.insert_hash(hash & BLOOM_HASH_MASK));
        true
    }
}

// ═══════════════════════════════════════════════════════════════════
// AttributeProvider
// ═══════════════════════════════════════════════════════════════════

impl AttributeProvider for KozanNode {
    fn get_attr(&self, attr: &LocalName, namespace: &Namespace) -> Option<String> {
        if !namespace.is_empty() {
            return None;
        }
        self.ed()?
            .attributes
            .get(attr.as_ref())
            .map(|v| v.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════
// TElement — reads from ElementData, Stylo data lives on ElementData
// ═══════════════════════════════════════════════════════════════════

impl TElement for KozanNode {
    type ConcreteNode = KozanNode;
    type TraversalChildrenIterator = KozanChildIter;

    fn as_node(&self) -> Self::ConcreteNode {
        *self
    }

    fn traversal_children(&self) -> LayoutIterator<Self::TraversalChildrenIterator> {
        LayoutIterator(KozanChildIter {
            current: KozanNode::first_child(self),
        })
    }

    fn is_html_element(&self) -> bool {
        true
    }
    fn is_mathml_element(&self) -> bool {
        false
    }
    fn is_svg_element(&self) -> bool {
        false
    }

    fn style_attribute(&self) -> Option<ArcBorrow<'_, Locked<PropertyDeclarationBlock>>> {
        self.ed()?
            .style_attribute
            .as_ref()
            .map(|arc| arc.borrow_arc())
    }

    fn animation_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<servo_arc::Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }
    fn transition_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<servo_arc::Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }

    fn state(&self) -> ElementState {
        self.ed().map_or(ElementState::empty(), |d| d.element_state)
    }

    fn has_part_attr(&self) -> bool {
        false
    }
    fn exports_any_part(&self) -> bool {
        false
    }

    fn id(&self) -> Option<&Atom> {
        self.ed()?.id.as_ref()
    }

    fn each_class<F>(&self, mut cb: F)
    where
        F: FnMut(&AtomIdent),
    {
        if let Some(d) = self.ed() {
            for class in &d.classes {
                cb(&AtomIdent::from(&**class));
            }
        }
    }

    fn each_custom_state<F>(&self, _: F)
    where
        F: FnMut(&AtomIdent),
    {
    }

    fn each_attr_name<F>(&self, mut cb: F)
    where
        F: FnMut(&LocalName),
    {
        if let Some(d) = self.ed() {
            for attr in d.attributes.iter() {
                cb(&LocalName::from(attr.name()));
            }
        }
    }

    fn has_dirty_descendants(&self) -> bool {
        self.ed().is_some_and(|d| d.dirty_descendants.get())
    }

    fn has_snapshot(&self) -> bool {
        self.ed().is_some_and(|d| d.has_snapshot.get())
    }

    fn handled_snapshot(&self) -> bool {
        self.ed().is_some_and(|d| d.handled_snapshot.get())
    }

    unsafe fn set_handled_snapshot(&self) {
        if let Some(d) = self.ed() {
            d.handled_snapshot.set(true);
        }
    }

    unsafe fn set_dirty_descendants(&self) {
        if let Some(d) = self.ed_mut() {
            d.dirty_descendants.set(true);
        }
    }

    unsafe fn unset_dirty_descendants(&self) {
        if let Some(d) = self.ed() {
            d.dirty_descendants.set(false);
        }
    }

    fn store_children_to_process(&self, n: isize) {
        if let Some(d) = self.ed() {
            d.children_to_process.set(n);
        }
    }

    fn did_process_child(&self) -> isize {
        if let Some(d) = self.ed() {
            let r = d.children_to_process.get() - 1;
            d.children_to_process.set(r);
            r
        } else {
            0
        }
    }

    unsafe fn ensure_data(&self) -> ElementDataMut<'_> {
        self.ed()
            .expect("ensure_data on non-element")
            .stylo_data
            .borrow_mut()
    }

    unsafe fn clear_data(&self) {
        // Stylo data lives on ElementData — clearing means resetting the wrapper.
        // We don't remove the ElementData itself (it's part of the DOM).
        if let Some(d) = self.ed_mut() {
            d.stylo_data = style::data::ElementDataWrapper::default();
        }
    }

    fn has_data(&self) -> bool {
        self.ed().is_some()
    }

    fn borrow_data(&self) -> Option<ElementDataRef<'_>> {
        Some(self.ed()?.stylo_data.borrow())
    }

    fn mutate_data(&self) -> Option<ElementDataMut<'_>> {
        Some(self.ed()?.stylo_data.borrow_mut())
    }

    fn skip_item_display_fixup(&self) -> bool {
        false
    }
    fn may_have_animations(&self) -> bool {
        false
    }
    fn has_animations(&self, _: &SharedStyleContext) -> bool {
        false
    }
    fn has_css_animations(&self, _: &SharedStyleContext, _: Option<PseudoElement>) -> bool {
        false
    }
    fn has_css_transitions(&self, _: &SharedStyleContext, _: Option<PseudoElement>) -> bool {
        false
    }
    fn shadow_root(&self) -> Option<<Self::ConcreteNode as TNode>::ConcreteShadowRoot> {
        None
    }
    fn containing_shadow(&self) -> Option<<Self::ConcreteNode as TNode>::ConcreteShadowRoot> {
        None
    }

    fn lang_attr(&self) -> Option<AttrValue> {
        let ed = self.ed()?;
        ed.attributes
            .get("lang")
            .map(|v| AttrValue::from(v as &str))
    }

    fn match_element_lang(&self, override_lang: Option<Option<AttrValue>>, value: &Lang) -> bool {
        let lang = match override_lang {
            Some(Some(ref l)) => l.as_ref().to_string(),
            Some(None) => return false,
            None => {
                let ed = match self.ed() {
                    Some(d) => d,
                    None => return false,
                };
                match ed.attributes.get("lang") {
                    Some(v) => v.to_string(),
                    None => return false,
                }
            }
        };
        lang.starts_with(value.as_ref())
    }

    fn is_html_document_body_element(&self) -> bool {
        self.ed().is_some_and(|d| d.tag_name == "body")
    }

    fn synthesize_presentational_hints_for_legacy_attributes<V>(
        &self,
        _: VisitedHandlingMode,
        _: &mut V,
    ) where
        V: Push<ApplicableDeclarationBlock>,
    {
    }

    fn local_name(&self) -> &<SelectorImpl as selectors::parser::SelectorImpl>::BorrowedLocalName {
        let ed = self.ed().expect("local_name on non-element");
        &ed.local_name
    }

    fn namespace(
        &self,
    ) -> &<SelectorImpl as selectors::parser::SelectorImpl>::BorrowedNamespaceUrl {
        let ed = self.ed().expect("namespace on non-element");
        &ed.namespace
    }

    fn query_container_size(
        &self,
        _: &style::values::computed::Display,
    ) -> euclid::default::Size2D<Option<app_units::Au>> {
        euclid::default::Size2D::new(None, None)
    }

    fn has_selector_flags(&self, flags: ElementSelectorFlags) -> bool {
        self.ed()
            .is_some_and(|d| d.selector_flags.get().contains(flags))
    }

    fn relative_selector_search_direction(&self) -> ElementSelectorFlags {
        self.ed()
            .map_or(ElementSelectorFlags::empty(), |d| d.selector_flags.get())
    }
}
