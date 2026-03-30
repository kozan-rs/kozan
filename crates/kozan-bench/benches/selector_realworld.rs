//! Real-world selector matching benchmarks with a tree-structured DOM.
//!
//! Unlike the cascade benchmarks (flat elements, no tree), these benchmarks
//! use a proper arena-backed DOM tree so descendant/child combinators actually
//! walk ancestors. This gives **pessimistic** (real) numbers.
//!
//! Benchmark groups:
//! - **simple**: `.class`, `#id`, `tag` — hash lookup only
//! - **compound**: `div.card.active` — multiple checks, no tree walk
//! - **descendant**: `.container .item` — ancestor walking
//! - **deep_descendant**: `html body .app .main .content .card .title` — deep chain
//! - **child**: `.nav > .link` — single parent check
//! - **nth_child**: `:nth-child(2n+1)`, `:first-child`, `:last-child` — sibling counting
//! - **mixed_real_world**: realistic CSS from a web app (200 rules, mixed complexity)
//! - **worst_case**: selectors designed to be maximally expensive

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;

use kozan_atom::Atom;
use kozan_cascade::cascade::{self, ApplicableDeclaration};
use kozan_cascade::device::Device;
use kozan_cascade::origin::CascadeOrigin;
use kozan_cascade::resolver::ResolvedStyle;
use kozan_cascade::sharing_cache::{hash_matched, MatchedPropertiesCache, SharingCache, SharingKey};
use kozan_cascade::stylist::Stylist;
use kozan_css::parse_stylesheet;
use kozan_selector::bloom::AncestorBloom;
use kozan_selector::context::{MatchingContext, QuirksMode, SelectorCaches};
use kozan_selector::element::Element;
use kozan_selector::opaque::OpaqueElement;
use kozan_selector::pseudo_class::ElementState;
use kozan_selector::rule_map::RuleMap;
use smallvec::SmallVec;

// ═══════════════════════════════════════════════════
// ARENA-BACKED TREE ELEMENT
// ═══════════════════════════════════════════════════

/// Node data stored in the arena.
#[derive(Clone)]
struct NodeData {
    tag: Atom,
    id: Option<Atom>,
    classes: SmallVec<[Atom; 4]>,
    state: ElementState,
    parent: Option<u32>,
    prev_sibling: Option<u32>,
    next_sibling: Option<u32>,
    first_child: Option<u32>,
    last_child: Option<u32>,
    child_index: u32,   // 1-based
    child_count: u32,   // total siblings including self
    is_root: bool,
}

/// Shared arena holding all nodes. Rc for cheap cloning in Element trait.
type Arena = Arc<Vec<NodeData>>;

/// A tree element backed by a shared arena. Cloning is O(1) (Arc + index).
#[derive(Clone)]
struct TreeElement {
    arena: Arena,
    index: u32,
}

impl TreeElement {
    fn data(&self) -> &NodeData {
        &self.arena[self.index as usize]
    }

    fn at(&self, idx: u32) -> Self {
        Self { arena: self.arena.clone(), index: idx }
    }
}

impl Element for TreeElement {
    fn local_name(&self) -> &Atom { &self.data().tag }
    fn id(&self) -> Option<&Atom> { self.data().id.as_ref() }
    fn has_class(&self, class: &Atom) -> bool {
        self.data().classes.iter().any(|c| c == class)
    }
    fn each_class<F: FnMut(&Atom)>(&self, mut f: F) {
        for c in &self.data().classes { f(c); }
    }
    fn attr(&self, _: &Atom) -> Option<&str> { None }
    fn parent_element(&self) -> Option<Self> {
        self.data().parent.map(|i| self.at(i))
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        self.data().prev_sibling.map(|i| self.at(i))
    }
    fn next_sibling_element(&self) -> Option<Self> {
        self.data().next_sibling.map(|i| self.at(i))
    }
    fn first_child_element(&self) -> Option<Self> {
        self.data().first_child.map(|i| self.at(i))
    }
    fn last_child_element(&self) -> Option<Self> {
        self.data().last_child.map(|i| self.at(i))
    }
    fn state(&self) -> ElementState { self.data().state }
    fn is_root(&self) -> bool { self.data().is_root }
    fn is_empty(&self) -> bool { self.data().first_child.is_none() }
    fn child_index(&self) -> u32 { self.data().child_index }
    fn child_count(&self) -> u32 { self.data().child_count }
    fn child_index_of_type(&self) -> u32 { self.data().child_index }
    fn child_count_of_type(&self) -> u32 { self.data().child_count }
    fn opaque(&self) -> OpaqueElement { OpaqueElement::new(self.index as u64) }
}

// ═══════════════════════════════════════════════════
// DOM TREE BUILDER
// ═══════════════════════════════════════════════════

struct DomBuilder {
    nodes: Vec<NodeData>,
}

impl DomBuilder {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add a node. Returns its index.
    fn add(&mut self, tag: &str, id: Option<&str>, classes: &[&str], parent: Option<u32>) -> u32 {
        let idx = self.nodes.len() as u32;
        let mut child_index = 1u32;
        let mut prev_sibling = None;

        if let Some(p) = parent {
            let parent_node = &self.nodes[p as usize];
            // Count existing children to get child_index
            let mut count = 0u32;
            let mut last = parent_node.first_child;
            while let Some(c) = last {
                count += 1;
                prev_sibling = Some(c);
                last = self.nodes[c as usize].next_sibling;
            }
            child_index = count + 1;
        }

        self.nodes.push(NodeData {
            tag: Atom::from(tag),
            id: id.map(Atom::from),
            classes: classes.iter().map(|c| Atom::from(*c)).collect(),
            state: ElementState::empty(),
            parent,
            prev_sibling,
            next_sibling: None,
            first_child: None,
            last_child: None,
            child_index,
            child_count: 1, // Updated in finalize
            is_root: parent.is_none(),
        });

        // Link to parent
        if let Some(p) = parent {
            if self.nodes[p as usize].first_child.is_none() {
                self.nodes[p as usize].first_child = Some(idx);
            }
            self.nodes[p as usize].last_child = Some(idx);
        }

        // Link prev sibling
        if let Some(prev) = prev_sibling {
            self.nodes[prev as usize].next_sibling = Some(idx);
        }

        idx
    }

    fn set_state(&mut self, idx: u32, state: ElementState) {
        self.nodes[idx as usize].state = state;
    }

    /// Finalize: compute child_count for all siblings.
    fn build(mut self) -> (Arena, Vec<TreeElement>) {
        // Compute child_count per parent group
        let len = self.nodes.len();
        for i in 0..len {
            if let Some(p) = self.nodes[i].parent {
                // Count all children of this parent
                let mut count = 0u32;
                let mut c = self.nodes[p as usize].first_child;
                while let Some(ci) = c {
                    count += 1;
                    c = self.nodes[ci as usize].next_sibling;
                }
                // Set child_count for all children of this parent
                let mut c = self.nodes[p as usize].first_child;
                while let Some(ci) = c {
                    self.nodes[ci as usize].child_count = count;
                    c = self.nodes[ci as usize].next_sibling;
                }
            }
        }

        let arena = Arc::new(self.nodes);
        let elements: Vec<TreeElement> = (0..len as u32)
            .map(|i| TreeElement { arena: arena.clone(), index: i })
            .collect();
        (arena, elements)
    }
}

// ═══════════════════════════════════════════════════
// DOM TREE FIXTURES
// ═══════════════════════════════════════════════════

/// Build a realistic web app DOM tree (depth ~8, ~120 nodes).
///
/// ```text
/// html
///   body
///     div#app.app
///       header.header
///         nav.nav
///           a.link.active (×5)
///       main.main.container
///         section.content
///           div.card (×10)
///             h2.card-title
///             p.card-body
///             div.card-footer
///               button.btn.btn-primary
///         aside.sidebar
///           ul.menu
///             li.menu-item (×8)
///               a.menu-link
///       footer.footer
///         div.footer-content
///           span.copyright
/// ```
fn build_webapp_dom() -> (Arena, Vec<TreeElement>) {
    let mut b = DomBuilder::new();

    let html = b.add("html", None, &[], None);
    let body = b.add("body", None, &[], Some(html));
    let app = b.add("div", Some("app"), &["app"], Some(body));

    // Header + nav
    let header = b.add("header", None, &["header"], Some(app));
    let nav = b.add("nav", None, &["nav"], Some(header));
    let mut nav_links = Vec::new();
    for i in 0..5 {
        let classes: Vec<&str> = if i == 0 {
            vec!["link", "active"]
        } else {
            vec!["link"]
        };
        let link = b.add("a", None, &classes, Some(nav));
        nav_links.push(link);
    }
    // Set hover on third link
    b.set_state(nav_links[2], ElementState::HOVER);

    // Main content
    let main = b.add("main", None, &["main", "container"], Some(app));
    let section = b.add("section", None, &["content"], Some(main));

    // 10 cards
    let mut card_targets = Vec::new();
    for i in 0..10 {
        let card = b.add("div", None, &["card"], Some(section));
        let title = b.add("h2", None, &["card-title"], Some(card));
        let _body = b.add("p", None, &["card-body"], Some(card));
        let footer = b.add("div", None, &["card-footer"], Some(card));
        let btn_classes: Vec<&str> = if i % 2 == 0 {
            vec!["btn", "btn-primary"]
        } else {
            vec!["btn", "btn-secondary"]
        };
        let btn = b.add("button", None, &btn_classes, Some(footer));
        card_targets.push((card, title, btn));
    }

    // Sidebar
    let aside = b.add("aside", None, &["sidebar"], Some(main));
    let ul = b.add("ul", None, &["menu"], Some(aside));
    for _ in 0..8 {
        let li = b.add("li", None, &["menu-item"], Some(ul));
        b.add("a", None, &["menu-link"], Some(li));
    }

    // Footer
    let footer_el = b.add("footer", None, &["footer"], Some(app));
    let footer_content = b.add("div", None, &["footer-content"], Some(footer_el));
    b.add("span", None, &["copyright"], Some(footer_content));

    b.build()
}

/// Deep DOM tree — 30 levels deep, single chain (worst case for descendant walking).
fn build_deep_dom() -> (Arena, Vec<TreeElement>) {
    let mut b = DomBuilder::new();

    let tags = ["div", "section", "article", "main", "aside", "nav"];
    let classes_pool = ["l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9"];

    let mut parent: Option<u32> = None;
    for i in 0..30 {
        let tag = tags[i % tags.len()];
        let cls = classes_pool[i % classes_pool.len()];
        let node = b.add(tag, None, &[cls, "node"], parent);
        parent = Some(node);
    }
    // Leaf element at depth 30
    b.add("span", None, &["leaf", "target"], parent);

    b.build()
}

/// Wide DOM — 1 parent, 100 children (worst case for :nth-child).
fn build_wide_dom() -> (Arena, Vec<TreeElement>) {
    let mut b = DomBuilder::new();

    let parent = b.add("ul", None, &["list"], None);
    for i in 0..100 {
        let classes: Vec<&str> = if i % 2 == 0 {
            vec!["item", "even"]
        } else {
            vec!["item", "odd"]
        };
        b.add("li", None, &classes, Some(parent));
    }

    b.build()
}

fn build_stylist(css: &str) -> Stylist {
    let sheet = parse_stylesheet(css);
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(sheet, CascadeOrigin::Author);
    stylist.rebuild();
    stylist
}

/// Run find_matching on a set of target elements and return total match count.
fn match_elements(map: &RuleMap, targets: &[TreeElement]) -> u32 {
    let mut total = 0u32;
    for el in targets {
        map.for_each_matching(el, |_| total += 1);
    }
    total
}

/// Full pipeline: match + sort + apply on target elements.
fn full_pipeline(stylist: &Stylist, targets: &[TreeElement]) -> u32 {
    let map = stylist.author_map();
    let rules = stylist.rules();
    let mut total = 0u32;
    for el in targets {
        let matches = map.find_matching(el);
        let mut decls: Vec<ApplicableDeclaration> = matches
            .iter()
            .map(|entry| ApplicableDeclaration {
                rule_index: entry.data,
                specificity: entry.specificity.value(),
                source_order: entry.source_order,
                origin: CascadeOrigin::Author,
                layer_order: 0,
                scope_depth: 0,
            })
            .collect();
        cascade::sort(&mut decls, rules);
        let mut applied = 0u32;
        cascade::cascade_apply(&decls, rules, |_, _, _| applied += 1);
        total += applied;
    }
    total
}

// ═══════════════════════════════════════════════════
// BENCHMARK 1: SIMPLE SELECTORS (hash lookup only)
// ═══════════════════════════════════════════════════

fn bench_simple_selectors(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/simple");

    let css = r#"
        .card { display: flex }
        .btn { padding: 8px }
        .link { color: blue }
        .active { font-weight: bold }
        #app { width: 100% }
        div { box-sizing: border-box }
        button { cursor: pointer }
        h2 { font-size: 1.5rem }
        * { margin: 0 }
    "#;
    let stylist = build_stylist(css);
    let (_, elements) = build_webapp_dom();

    // Pick diverse target elements
    let targets: Vec<_> = elements.iter()
        .filter(|e| {
            let tag = e.local_name().as_ref();
            matches!(tag, "div" | "button" | "a" | "h2")
        })
        .cloned()
        .collect();

    group.bench_function("9_rules", |b| {
        b.iter(|| black_box(match_elements(stylist.author_map(), black_box(&targets))))
    });

    // Scale up: 100 simple class rules
    let mut css100 = String::new();
    for i in 0..100 {
        css100.push_str(&format!(".c{i} {{ color: red }}\n"));
    }
    css100.push_str("* { box-sizing: border-box }\n");
    let stylist100 = build_stylist(&css100);

    group.bench_function("101_rules", |b| {
        b.iter(|| black_box(match_elements(stylist100.author_map(), black_box(&targets))))
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 2: COMPOUND SELECTORS (multi-check, no walk)
// ═══════════════════════════════════════════════════

fn bench_compound_selectors(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/compound");

    let css = r#"
        div.card { border: 1px solid }
        button.btn.btn-primary { background: blue }
        button.btn.btn-secondary { background: gray }
        a.link.active { color: red }
        div.card-footer { padding: 16px }
        h2.card-title { font-size: 1.2rem }
        li.menu-item { list-style: none }
        a.menu-link { text-decoration: none }
        div.app { min-height: 100vh }
        section.content { flex: 1 }
    "#;
    let stylist = build_stylist(css);
    let (_, elements) = build_webapp_dom();

    let targets: Vec<_> = elements.iter()
        .filter(|e| !e.data().classes.is_empty())
        .cloned()
        .collect();

    group.bench_function("10_rules", |b| {
        b.iter(|| black_box(match_elements(stylist.author_map(), black_box(&targets))))
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 3: DESCENDANT SELECTORS (ancestor walking)
// ═══════════════════════════════════════════════════

fn bench_descendant_selectors(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/descendant");

    // Shallow descendants (1-2 levels up)
    let css_shallow = r#"
        .nav .link { color: blue }
        .card .card-title { font-size: 1.2rem }
        .card .btn { padding: 8px }
        .sidebar .menu-item { border: none }
        .menu .menu-link { display: block }
        .header .link { font-weight: 500 }
        .content .card { margin: 16px }
        .footer .copyright { font-size: 0.8rem }
    "#;
    let stylist_shallow = build_stylist(css_shallow);
    let (_, elements) = build_webapp_dom();
    let targets: Vec<_> = elements[5..].to_vec(); // Skip html/body/app

    group.bench_function("shallow_8_rules", |b| {
        b.iter(|| black_box(match_elements(stylist_shallow.author_map(), black_box(&targets))))
    });

    // Deep descendants (many levels up)
    let css_deep = r#"
        html .link { color: inherit }
        body .btn { cursor: pointer }
        #app .card-title { color: #333 }
        .app .content .card .btn { background: blue }
        .container .card .card-footer .btn-primary { font-weight: bold }
        html body div main section div h2 { line-height: 1.4 }
    "#;
    let stylist_deep = build_stylist(css_deep);

    group.bench_function("deep_6_rules", |b| {
        b.iter(|| black_box(match_elements(stylist_deep.author_map(), black_box(&targets))))
    });

    // Mix of shallow + deep + simple (realistic)
    let css_mixed = r#"
        .card { display: flex }
        .nav .link { color: blue }
        .card .card-title { font-size: 1.2rem }
        body .btn { cursor: pointer }
        #app .content .card .btn-primary { background: blue }
        button { border: none }
        * { box-sizing: border-box }
        .sidebar .menu .menu-item .menu-link { color: #666 }
    "#;
    let stylist_mixed = build_stylist(css_mixed);

    group.bench_function("mixed_8_rules", |b| {
        b.iter(|| black_box(match_elements(stylist_mixed.author_map(), black_box(&targets))))
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 4: DEEP CHAIN (worst case ancestor walking)
// ═══════════════════════════════════════════════════

fn bench_deep_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/deep_chain");

    let (_, elements) = build_deep_dom();
    let leaf = vec![elements.last().unwrap().clone()]; // depth-30 leaf

    // Selector that matches at root — 30 levels of walking
    let css_worst = ".l0 .target { color: red }";
    let stylist = build_stylist(css_worst);

    group.bench_function("depth30_match_at_root", |b| {
        b.iter(|| black_box(match_elements(stylist.author_map(), black_box(&leaf))))
    });

    // Selector that DOESN'T match — still walks all 30 ancestors before failing
    let css_miss = ".nonexistent .target { color: red }";
    let stylist_miss = build_stylist(css_miss);

    group.bench_function("depth30_miss", |b| {
        b.iter(|| black_box(match_elements(stylist_miss.author_map(), black_box(&leaf))))
    });

    // Multiple deep selectors
    let css_multi = r#"
        .l0 .target { color: red }
        .l1 .target { display: block }
        .l2 .target { margin: 0 }
        .l3 .target { padding: 0 }
        .l9 .target { font-size: 14px }
        .node .leaf { font-weight: bold }
        div .node .target { border: none }
    "#;
    let stylist_multi = build_stylist(css_multi);

    group.bench_function("depth30_7_selectors", |b| {
        b.iter(|| black_box(match_elements(stylist_multi.author_map(), black_box(&leaf))))
    });

    // Full pipeline on the leaf
    group.bench_function("depth30_full_pipeline", |b| {
        b.iter(|| black_box(full_pipeline(&stylist_multi, black_box(&leaf))))
    });

    // ── Bloom filter comparison on deep tree ──
    // Pre-build bloom with all 30 ancestors pushed (simulates DFS walk reaching the leaf).
    let mut bloom = AncestorBloom::new();
    for el in &elements[..elements.len() - 1] {
        bloom.push(el);
    }
    let leaf_el = elements.last().unwrap();

    // Miss with bloom: bloom rejects `.nonexistent` in O(1) — no ancestor walk
    let css_many_miss = r#"
        .nonexistent1 .target { color: red }
        .nonexistent2 .target { color: blue }
        .nonexistent3 .target { color: green }
        .miss1 .miss2 .target { color: purple }
        .fake .target { color: pink }
        .nope1 .nope2 .nope3 .target { color: black }
        .absent .leaf { font-weight: bold }
        .gone .node .target { border: none }
    "#;
    let stylist_miss_many = build_stylist(css_many_miss);

    group.bench_function("depth30_8_miss_NO_bloom", |b| {
        b.iter(|| {
            let map = stylist_miss_many.author_map();
            let mut count = 0u32;
            map.for_each_matching(black_box(leaf_el), |_| count += 1);
            black_box(count)
        })
    });

    group.bench_function("depth30_8_miss_WITH_bloom", |b| {
        b.iter(|| {
            let map = stylist_miss_many.author_map();
            let mut caches = SelectorCaches::new();
            let mut ctx = MatchingContext::for_restyle(
                &bloom,
                QuirksMode::NoQuirks,
                &mut caches,
            );
            let mut count = 0u32;
            map.for_each_matching_in_context(black_box(leaf_el), &mut ctx, |_| count += 1);
            black_box(count)
        })
    });

    // Hit with bloom: `.l0 .target` — bloom says "maybe" (l0 IS in ancestors), still walks
    group.bench_function("depth30_hit_NO_bloom", |b| {
        b.iter(|| {
            let map = stylist.author_map();
            let mut count = 0u32;
            map.for_each_matching(black_box(leaf_el), |_| count += 1);
            black_box(count)
        })
    });

    group.bench_function("depth30_hit_WITH_bloom", |b| {
        b.iter(|| {
            let map = stylist.author_map();
            let mut caches = SelectorCaches::new();
            let mut ctx = MatchingContext::for_restyle(
                &bloom,
                QuirksMode::NoQuirks,
                &mut caches,
            );
            let mut count = 0u32;
            map.for_each_matching_in_context(black_box(leaf_el), &mut ctx, |_| count += 1);
            black_box(count)
        })
    });

    // All 31 elements with bloom DFS walk vs without
    group.bench_function("depth30_all_nodes_NO_bloom", |b| {
        b.iter(|| {
            let map = stylist_multi.author_map();
            let mut total = 0u32;
            for el in &elements {
                map.for_each_matching(black_box(el), |_| total += 1);
            }
            black_box(total)
        })
    });

    group.bench_function("depth30_all_nodes_WITH_bloom_dfs", |b| {
        b.iter(|| {
            let map = stylist_multi.author_map();
            let mut bloom_local = AncestorBloom::new();
            let mut caches = SelectorCaches::new();
            let mut total = 0u32;
            for el in &elements {
                let mut ctx = MatchingContext::for_restyle(
                    &bloom_local,
                    QuirksMode::NoQuirks,
                    &mut caches,
                );
                map.for_each_matching_in_context(black_box(el), &mut ctx, |_| total += 1);
                bloom_local.push(el);
            }
            black_box(total)
        })
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 5: CHILD COMBINATOR (>)
// ═══════════════════════════════════════════════════

fn bench_child_combinator(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/child");

    let css = r#"
        .nav > .link { color: blue }
        .card > .card-title { font-size: 1.2rem }
        .card > .card-body { line-height: 1.6 }
        .card-footer > .btn { padding: 8px 16px }
        .menu > .menu-item { list-style: none }
        .menu-item > .menu-link { display: block }
        #app > header { border-bottom: 1px solid }
        #app > main { flex: 1 }
        #app > footer { border-top: 1px solid }
    "#;
    let stylist = build_stylist(css);
    let (_, elements) = build_webapp_dom();
    let targets: Vec<_> = elements[3..].to_vec();

    group.bench_function("9_rules", |b| {
        b.iter(|| black_box(match_elements(stylist.author_map(), black_box(&targets))))
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 6: NTH-CHILD (sibling counting)
// ═══════════════════════════════════════════════════

fn bench_nth_child(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/nth_child");

    let (_, elements) = build_wide_dom();
    let targets: Vec<_> = elements[1..].to_vec(); // 100 <li> children

    // :first-child / :last-child (fast — just check index)
    let css_edge = r#"
        .item:first-child { font-weight: bold }
        .item:last-child { margin-bottom: 0 }
    "#;
    let stylist_edge = build_stylist(css_edge);

    group.bench_function("first_last_child/100_items", |b| {
        b.iter(|| black_box(match_elements(stylist_edge.author_map(), black_box(&targets))))
    });

    // :nth-child(2n+1) — needs modular arithmetic
    let css_nth = r#"
        .item:nth-child(2n+1) { background: #f5f5f5 }
        .item:nth-child(3n) { border-bottom: 1px solid }
        .item:nth-child(even) { background: white }
    "#;
    let stylist_nth = build_stylist(css_nth);

    group.bench_function("nth_formulas/100_items", |b| {
        b.iter(|| black_box(match_elements(stylist_nth.author_map(), black_box(&targets))))
    });

    // Full pipeline with nth-child
    group.bench_function("nth_full_pipeline/100_items", |b| {
        b.iter(|| black_box(full_pipeline(&stylist_nth, black_box(&targets))))
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 7: PSEUDO-CLASS STATE
// ═══════════════════════════════════════════════════

fn bench_pseudo_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/pseudo_state");

    let css = r#"
        .link:hover { color: red }
        .btn:hover { background: darkblue }
        .link:active { color: darkred }
        a:visited { color: purple }
        button:focus { outline: 2px solid blue }
        .link:hover:active { transform: scale(0.98) }
    "#;
    let stylist = build_stylist(css);
    let (_, elements) = build_webapp_dom();
    let targets: Vec<_> = elements.iter()
        .filter(|e| {
            let tag = e.local_name().as_ref();
            tag == "a" || tag == "button"
        })
        .cloned()
        .collect();

    group.bench_function("6_rules", |b| {
        b.iter(|| black_box(match_elements(stylist.author_map(), black_box(&targets))))
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 8: REAL-WORLD CSS (mixed complexity, 200 rules)
// ═══════════════════════════════════════════════════

fn gen_realworld_css() -> String {
    let mut css = String::with_capacity(10000);

    // Reset/normalize (simple)
    css.push_str("*, *::before, *::after { box-sizing: border-box }\n");
    css.push_str("body { margin: 0; font-family: sans-serif; line-height: 1.5 }\n");
    css.push_str("h1, h2, h3, h4, h5, h6 { margin: 0 }\n");
    css.push_str("a { color: inherit; text-decoration: none }\n");
    css.push_str("button { cursor: pointer; border: none; background: none }\n");
    css.push_str("ul { list-style: none; padding: 0; margin: 0 }\n");

    // Layout (compound)
    css.push_str(".app { display: flex; flex-direction: column; min-height: 100vh }\n");
    css.push_str(".main { display: flex; flex: 1 }\n");
    css.push_str(".container { max-width: 1200px; margin: 0 auto; padding: 0 16px }\n");
    css.push_str(".content { flex: 1; padding: 24px }\n");
    css.push_str(".sidebar { width: 280px; padding: 24px }\n");

    // Header/nav (descendant)
    css.push_str(".header { background: #fff; border-bottom: 1px solid #e0e0e0 }\n");
    css.push_str(".header .nav { display: flex; gap: 16px; padding: 12px 0 }\n");
    css.push_str(".nav .link { padding: 8px 16px; border-radius: 4px }\n");
    css.push_str(".nav .link.active { background: #e3f2fd; color: #1976d2 }\n");
    css.push_str(".nav .link:hover { background: #f5f5f5 }\n");

    // Card component (descendant + compound)
    css.push_str(".card { background: #fff; border-radius: 8px; overflow: hidden }\n");
    css.push_str(".card .card-title { font-size: 1.25rem; font-weight: 600 }\n");
    css.push_str(".card .card-body { padding: 16px; color: #424242 }\n");
    css.push_str(".card .card-footer { padding: 12px 16px; border-top: 1px solid #e0e0e0 }\n");
    css.push_str(".content .card { margin-bottom: 16px }\n");

    // Buttons (compound + state)
    css.push_str(".btn { padding: 8px 16px; border-radius: 4px; font-weight: 500 }\n");
    css.push_str(".btn.btn-primary { background: #1976d2; color: #fff }\n");
    css.push_str(".btn.btn-secondary { background: #757575; color: #fff }\n");
    css.push_str(".btn:hover { filter: brightness(1.1) }\n");
    css.push_str(".card-footer .btn { font-size: 0.875rem }\n");
    css.push_str(".card-footer > .btn-primary { font-weight: 600 }\n");

    // Menu (descendant chain)
    css.push_str(".sidebar .menu { display: flex; flex-direction: column }\n");
    css.push_str(".menu .menu-item { padding: 0 }\n");
    css.push_str(".menu .menu-item .menu-link { padding: 8px 12px; display: block }\n");
    css.push_str(".menu .menu-link:hover { background: #f5f5f5 }\n");
    css.push_str(".sidebar .menu .menu-item:first-child { padding-top: 0 }\n");

    // Footer
    css.push_str(".footer { background: #fafafa; border-top: 1px solid #e0e0e0; padding: 24px 0 }\n");
    css.push_str(".footer .footer-content { text-align: center; color: #757575 }\n");
    css.push_str(".footer .copyright { font-size: 0.75rem }\n");

    // Utility classes (many simple — like Tailwind)
    for i in 0..50 {
        css.push_str(&format!(".u{i} {{ padding: {i}px }}\n"));
    }

    // Complex selectors (deep descendant + compound + state)
    css.push_str("#app .main .content .card:hover { box-shadow: 0 2px 8px rgba(0,0,0,.1) }\n");
    css.push_str("#app .main .sidebar .menu .menu-item:nth-child(odd) .menu-link { background: #fafafa }\n");
    css.push_str("body .app .header .nav > .link.active:hover { background: #bbdefb }\n");
    css.push_str(".content .card .card-footer > .btn.btn-primary:hover { background: #1565c0 }\n");

    // Additional rules to hit 200
    for i in 0..100 {
        match i % 5 {
            0 => css.push_str(&format!(".x{i} {{ color: red }}\n")),
            1 => css.push_str(&format!(".card .x{i} {{ display: block }}\n")),
            2 => css.push_str(&format!("div.x{i} {{ margin: 0 }}\n")),
            3 => css.push_str(&format!(".nav .x{i} {{ padding: 0 }}\n")),
            _ => css.push_str(&format!(".sidebar .menu .x{i} {{ border: none }}\n")),
        }
    }

    css
}

fn bench_realworld(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/realworld");
    group.sample_size(50);

    let css = gen_realworld_css();
    let stylist = build_stylist(&css);
    let (_, all_elements) = build_webapp_dom();
    let rules = stylist.rules();

    // All elements in the tree (full restyle scenario)
    group.bench_function("match_all_elements", |b| {
        b.iter(|| black_box(match_elements(stylist.author_map(), black_box(&all_elements))))
    });

    group.bench_function("full_pipeline_all_elements", |b| {
        b.iter(|| black_box(full_pipeline(&stylist, black_box(&all_elements))))
    });

    // With bloom filter (real restyle scenario)
    group.bench_function("with_bloom_all_elements", |b| {
        b.iter(|| {
            let map = stylist.author_map();
            let mut bloom = AncestorBloom::new();
            let mut total = 0u32;
            let mut caches = SelectorCaches::new();

            // Simple DFS-like walk — push ancestors, match, pop
            // For benchmarking we just push each element and match the next ones
            for el in &all_elements {
                let mut ctx = MatchingContext::for_restyle(
                    &bloom,
                    QuirksMode::NoQuirks,
                    &mut caches,
                );
                map.for_each_matching_in_context(el, &mut ctx, |_| total += 1);
                bloom.push(el);
            }
            black_box(total)
        })
    });

    // With full cache hierarchy (production scenario)
    group.bench_function("full_pipeline_with_caches", |b| {
        b.iter(|| {
            let map = stylist.author_map();
            let mut sharing = SharingCache::new();
            let mut mpc = MatchedPropertiesCache::new();
            let mut total = 0u32;

            for el in &all_elements {
                // L1: Sharing cache
                let key = SharingKey::new(
                    el.local_name().clone(),
                    el.id().cloned(),
                    el.data().classes.clone(),
                    el.state().bits() as u32,
                    el.data().parent.unwrap_or(u32::MAX) as u64,
                );
                if let Some(_) = sharing.get(&key) {
                    total += 1;
                    continue;
                }

                // L2: Match
                let matches = map.find_matching(el);
                let mut decls: Vec<ApplicableDeclaration> = matches
                    .iter()
                    .map(|entry| ApplicableDeclaration {
                        rule_index: entry.data,
                        specificity: entry.specificity.value(),
                        source_order: entry.source_order,
                        origin: CascadeOrigin::Author,
                        layer_order: 0,
                        scope_depth: 0,
                    })
                    .collect();

                // L3: MPC
                let h = hash_matched(&decls);
                if let Some(idx) = mpc.get(h) {
                    sharing.insert(key, idx);
                    total += 1;
                    continue;
                }

                // L4: Full cascade
                cascade::sort(&mut decls, rules);
                let mut applied = 0u32;
                cascade::cascade_apply(&decls, rules, |_, _, _| applied += 1);
                let dummy_style = Arc::new(ResolvedStyle {
                    style: kozan_style::ComputedStyle::default(),
                    custom_properties: kozan_cascade::CustomPropertyMap::new(),
                });
                mpc.insert(h, dummy_style.clone());
                sharing.insert(key, dummy_style);
                total += applied;
            }
            black_box(total)
        })
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 9: WORST CASE (maximally expensive selectors)
// ═══════════════════════════════════════════════════

fn bench_worst_case(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector/worst_case");

    // Deep descendant miss on deep tree (30 ancestor walks, no match)
    let (_, deep_elements) = build_deep_dom();
    let deep_leaf = vec![deep_elements.last().unwrap().clone()];

    let css_deep_miss = r#"
        .nonexistent1 .target { color: red }
        .nonexistent2 .target { color: blue }
        .nonexistent3 .target { color: green }
        .nonexistent4 .target { color: yellow }
        .nonexistent5 .target { color: orange }
        .miss1 .miss2 .miss3 .target { color: purple }
        html body .miss .target { color: pink }
        .a .b .c .d .e .target { color: black }
    "#;
    let stylist_deep = build_stylist(css_deep_miss);

    group.bench_function("deep30_8_miss_selectors", |b| {
        b.iter(|| black_box(match_elements(stylist_deep.author_map(), black_box(&deep_leaf))))
    });

    // Many universal rules (tested against every element)
    let mut css_universal = String::new();
    for i in 0..50 {
        css_universal.push_str(&format!("* {{ --var{i}: {i} }}\n"));
    }
    let stylist_univ = build_stylist(&css_universal);
    let (_, webapp) = build_webapp_dom();

    group.bench_function("50_universal_rules", |b| {
        b.iter(|| black_box(match_elements(stylist_univ.author_map(), black_box(&webapp))))
    });

    // nth-child on wide tree (100 siblings)
    let (_, wide_elements) = build_wide_dom();
    let wide_targets: Vec<_> = wide_elements[1..].to_vec();

    let css_nth_heavy = r#"
        .item:nth-child(2n+1) { background: #f5f5f5 }
        .item:nth-child(3n+2) { color: red }
        .item:nth-child(5n) { border: 1px solid }
        .item:nth-child(7n+3) { font-weight: bold }
        .item:first-child { border-top: none }
        .item:last-child { border-bottom: none }
        .list > .item:nth-child(even) { background: #fafafa }
        .list > .item:nth-child(4n+1) { margin-left: 0 }
    "#;
    let stylist_nth = build_stylist(css_nth_heavy);

    group.bench_function("8_nth_rules_100_siblings", |b| {
        b.iter(|| black_box(match_elements(stylist_nth.author_map(), black_box(&wide_targets))))
    });

    group.bench_function("8_nth_full_pipeline_100_siblings", |b| {
        b.iter(|| black_box(full_pipeline(&stylist_nth, black_box(&wide_targets))))
    });

    // ── Bloom definitive test ──
    // Key insight: bloom only helps when selectors REACH the matching engine.
    // With RuleMap bucketing, selectors are pre-filtered by class/ID/tag.
    // Bloom prunes WITHIN a bucket — when multiple descendant selectors share
    // the same key selector class but have different ancestor requirements.
    //
    // Real scenario: element has class "node", 50 rules target ".node" with
    // different ancestor requirements. Without bloom, each rule walks 30 ancestors.
    // With bloom, misses are rejected in O(1).
    let mut css_bloom = String::new();
    // 50 descendant rules all keyed on `.node` — they ALL hit the .node bucket
    for i in 0..50 {
        // Use class names that DON'T exist in ancestor chain → bloom should reject
        css_bloom.push_str(&format!(
            ".miss-ancestor-{i} .node {{ --prop{i}: {i} }}\n"
        ));
    }
    // 2 rules that DO match (ancestors .l0 and .l1 exist)
    css_bloom.push_str(".l0 .node { color: red }\n");
    css_bloom.push_str(".l1 .node { display: block }\n");
    let stylist_bloom_test = build_stylist(&css_bloom);

    // Pre-build bloom with all ancestors
    let mut deep_bloom = AncestorBloom::new();
    for el in &deep_elements[..deep_elements.len() - 1] {
        deep_bloom.push(el);
    }
    // Target: second-to-last element (has class "node", ancestors above it)
    let bloom_target = &deep_elements[deep_elements.len() - 2];

    group.bench_function("bloom_52_desc_rules_NO_bloom", |b| {
        b.iter(|| {
            let map = stylist_bloom_test.author_map();
            let mut count = 0u32;
            map.for_each_matching(black_box(bloom_target), |_| count += 1);
            black_box(count)
        })
    });

    group.bench_function("bloom_52_desc_rules_WITH_bloom", |b| {
        b.iter(|| {
            let map = stylist_bloom_test.author_map();
            let mut caches = SelectorCaches::new();
            let mut ctx = MatchingContext::for_restyle(
                &deep_bloom,
                QuirksMode::NoQuirks,
                &mut caches,
            );
            let mut count = 0u32;
            map.for_each_matching_in_context(black_box(bloom_target), &mut ctx, |_| count += 1);
            black_box(count)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_selectors,
    bench_compound_selectors,
    bench_descendant_selectors,
    bench_deep_chain,
    bench_child_combinator,
    bench_nth_child,
    bench_pseudo_state,
    bench_realworld,
    bench_worst_case,
);
criterion_main!(benches);
