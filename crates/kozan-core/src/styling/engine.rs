//! `StyleEngine` — concrete CSS engine backed by Mozilla's Stylo.
//!
//! No per-element storage here. All element data lives on `ElementData`
//! in `Storage<ElementData>`. `StyleEngine` owns only document-level state:
//! Stylist, `SharedRwLock`, snapshots, animations.

use style::context::{
    RegisteredSpeculativePainter, RegisteredSpeculativePainters, SharedStyleContext,
};
use style::global_style_data::GLOBAL_STYLE_DATA;
use style::media_queries::{MediaList, MediaType};
use style::properties::style_structs::Font;
use style::properties::ComputedValues;
use style::selector_parser::SnapshotMap;
use style::servo::animation::DocumentAnimationSet;
use style::shared_lock::{SharedRwLock, StylesheetGuards};
use style::stylesheets::{AllowImportRules, DocumentStyleSheet, Origin, Stylesheet};
use style::stylist::Stylist;
use style::thread_state::ThreadState;
use style::traversal::DomTraversal;
use style::traversal_flags::TraversalFlags;
use style::Atom;

use euclid::{Scale, Size2D};
use selectors::matching::QuirksMode;
use servo_arc::Arc;

use crate::dom::document_cell::DocumentCell;

use super::font_metrics::KozanFontMetricsProvider;
use super::node::KozanNode;
use super::traversal::RecalcStyle;

// ── Dummy speculative painters (required by SharedStyleContext) ──

struct RegisteredPaintersImpl;
impl RegisteredSpeculativePainters for RegisteredPaintersImpl {
    fn get(&self, _name: &Atom) -> Option<&dyn RegisteredSpeculativePainter> {
        None
    }
}

/// The CSS engine. Owns document-level Stylo state only.
/// Per-element data lives on `ElementData` in `Storage<ElementData>`.
pub(crate) struct StyleEngine {
    /// Stylo's shared lock (one per document).
    pub(crate) guard: SharedRwLock,

    /// The Stylist — Stylo's main style computation engine.
    stylist: Stylist,

    /// Snapshot map for invalidation.
    snapshots: SnapshotMap,

    /// Animation set (required by `SharedStyleContext`).
    animations: DocumentAnimationSet,
}

/// UA stylesheet CSS source — included at compile time.
static UA_CSS: &str = include_str!("../../assets/ua.css");

/// Global Stylo config — set once.
static STYLO_CONFIGURED: std::sync::Once = std::sync::Once::new();

impl StyleEngine {
    pub fn new() -> Self {
        // Global config — runs only once across all documents.
        // NOTE: Stylo uses TWO pref systems:
        //   - `style_config` (stylo_config) — runtime prefs, NOT read by Stylo's parser
        //   - `static_prefs` (stylo_static_prefs) — compile-time defaults + runtime overrides,
        //     used by Stylo's `pref!()` macro for feature-gating CSS properties
        // We MUST use `static_prefs::set_pref!` for prefs that gate CSS parsing (display: grid,
        // columns, etc.), otherwise Stylo silently ignores those CSS values.
        STYLO_CONFIGURED.call_once(|| {
            static_prefs::set_pref!("layout.grid.enabled", true);
            static_prefs::set_pref!("layout.columns.enabled", true);
        });

        let guard = SharedRwLock::new();

        let device = style::device::Device::new(
            MediaType::screen(),
            QuirksMode::NoQuirks,
            Size2D::new(1920.0, 1080.0),
            Scale::new(1.0),
            Box::new(KozanFontMetricsProvider::new()),
            ComputedValues::initial_values_with_font_override(Font::initial_values()),
            style::queries::values::PrefersColorScheme::Light,
        );

        let stylist = Stylist::new(device, QuirksMode::NoQuirks);

        let mut engine = Self {
            guard,
            stylist,
            snapshots: SnapshotMap::new(),
            animations: DocumentAnimationSet::default(),
        };

        // Load UA stylesheet. Each document gets its own copy
        // (SharedRwLock is per-document, can't share locked stylesheets).
        engine.add_ua_stylesheet(UA_CSS);
        engine
    }

    pub fn shared_lock(&self) -> &SharedRwLock {
        &self.guard
    }

    // ── Stylesheet management ──

    pub fn add_stylesheet(&mut self, css: &str) {
        let sheet = self.make_stylesheet(css, Origin::Author);
        let read_guard = self.guard.read();
        self.stylist.append_stylesheet(sheet, &read_guard);
    }

    pub fn add_ua_stylesheet(&mut self, css: &str) {
        let sheet = self.make_stylesheet(css, Origin::UserAgent);
        let read_guard = self.guard.read();
        self.stylist.append_stylesheet(sheet, &read_guard);
    }

    fn make_stylesheet(&self, css: &str, origin: Origin) -> DocumentStyleSheet {
        let url = url::Url::parse("kozan://stylesheet")
            .expect("hardcoded URL is always valid");
        let url_data = style::stylesheets::UrlExtraData(Arc::new(url));

        let data = Stylesheet::from_str(
            css,
            url_data,
            origin,
            Arc::new(self.guard.wrap(MediaList::empty())),
            self.guard.clone(),
            None,
            None,
            QuirksMode::NoQuirks,
            AllowImportRules::Yes,
        );
        DocumentStyleSheet(Arc::new(data))
    }

    #[allow(dead_code)]
    pub fn clear_stylesheets(&mut self) {
        let viewport = self.stylist.device().viewport_size();
        let device = style::device::Device::new(
            MediaType::screen(),
            QuirksMode::NoQuirks,
            viewport,
            Scale::new(1.0),
            Box::new(KozanFontMetricsProvider::new()),
            ComputedValues::initial_values_with_font_override(Font::initial_values()),
            style::queries::values::PrefersColorScheme::Light,
        );
        self.stylist = Stylist::new(device, QuirksMode::NoQuirks);
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.stylist
            .device_mut()
            .set_viewport_size(Size2D::new(width, height));
    }

    /// Flush all dirty inline styles to Arc<Locked<PDB>> cache.
    /// Called once before traversal. Only touches elements with `inline_dirty=true`.
    fn flush_inline_styles(&self, cell: DocumentCell) {
        cell.write(|doc| {
            let guard = &self.guard;
            // Walk ALL slots (capacity) — count() returns alive entries
            // which may be less than max index if nodes were freed.
            let capacity = doc.ids.capacity() as u32;
            for i in 0..capacity {
                if let Some(ed) = doc.element_data.get_mut(i) {
                    ed.flush_inline_styles(guard);
                }
            }
        });
    }

    // ── Style recalculation ──

    pub fn recalc_styles(&mut self, cell: DocumentCell, root: u32) {
        style::thread_state::enter(ThreadState::LAYOUT);
        super::node::enter(cell);

        // Find root element (first element child of document node).
        let root_node = KozanNode::new(root);
        let root_element = {
            let mut child = root_node.first_child();
            loop {
                match child {
                    Some(n) if n.is_element() => break Some(n),
                    Some(n) => child = n.next_sibling(),
                    None => break None,
                }
            }
        };

        let Some(root_element) = root_element else {
            super::node::exit();
            style::thread_state::exit(ThreadState::LAYOUT);
            return;
        };

        // Flush dirty inline styles → Arc<Locked<PDB>> for all elements.
        self.flush_inline_styles(cell);

        let guards = StylesheetGuards {
            author: &self.guard.read(),
            ua_or_user: &self.guard.read(),
        };

        // Flush pending stylesheet changes and process invalidations.
        self.stylist
            .flush(&guards)
            .process_style(root_element, Some(&self.snapshots));

        // Build shared style context.
        let context = SharedStyleContext {
            traversal_flags: TraversalFlags::empty(),
            stylist: &self.stylist,
            options: GLOBAL_STYLE_DATA.options.clone(),
            guards,
            visited_styles_enabled: false,
            animations: self.animations.clone(),
            current_time_for_animations: 0.0,
            snapshot_map: &self.snapshots,
            registered_speculative_painters: &RegisteredPaintersImpl,
        };

        // Pre-traverse and run Stylo's style computation.
        let token = <RecalcStyle as DomTraversal<KozanNode>>::pre_traverse(
            root_element,
            &context,
        );
        if token.should_traverse() {
            let traversal = RecalcStyle::new(context);
            style::driver::traverse_dom::<KozanNode, RecalcStyle>(&traversal, token, None);
        }

        self.snapshots.clear();
        self.stylist.rule_tree().maybe_gc();
        super::node::exit();
        style::thread_state::exit(ThreadState::LAYOUT);
    }
}
