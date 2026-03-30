//! Benchmark: kozan-cascade vs Stylo — stylist rebuild, cascade sort, media eval, caches.
//!
//! Compares kozan's cascade engine against Mozilla's Stylo for the operations
//! that can be isolated and benchmarked fairly:
//! - **Stylist rebuild**: parse CSS + index into selector maps (end-to-end)
//! - **Cascade sort**: sort matched declarations by priority
//! - **Media query evaluation**: evaluate @media conditions against a device
//! - **Cache operations**: sharing cache and matched properties cache
//!
//! Stylo's internal cascade is tightly integrated (no public sort/cache API),
//! so we compare what we can: the Stylist rebuild path (flush), which is the
//! main hot path for stylesheet changes.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use std::sync::LazyLock;

// ─── KOZAN IMPORTS ───

use kozan_atom::Atom;
use kozan_cascade::cascade::{self, ApplicableDeclaration};
use kozan_cascade::device::Device;
use kozan_cascade::layer::UNLAYERED;
use kozan_cascade::media;
use kozan_cascade::origin::{CascadeLevel, CascadeOrigin, Importance};
use kozan_cascade::sharing_cache::{hash_matched, MatchedPropertiesCache, SharingCache, SharingKey};
use kozan_cascade::stylist::{IndexedRule, Stylist};
use kozan_css::{
    parse_stylesheet, LengthUnit, MediaCondition, MediaFeature, MediaFeatureValue, MediaQuery,
    MediaQueryList, MediaType as CssMediaType, RangeOp,
};
use kozan_style::DeclarationBlock;

fn dummy_resolved() -> std::sync::Arc<kozan_cascade::resolver::ResolvedStyle> {
    std::sync::Arc::new(kozan_cascade::resolver::ResolvedStyle {
        style: kozan_style::ComputedStyle::default(),
        custom_properties: kozan_cascade::CustomPropertyMap::new(),
    })
}

// ─── STYLO IMPORTS ───

use servo_arc::Arc as ServoArc;

static STYLO_LOCK: LazyLock<stylo::shared_lock::SharedRwLock> =
    LazyLock::new(stylo::shared_lock::SharedRwLock::new);

static STYLO_URL: LazyLock<stylo::stylesheets::UrlExtraData> = LazyLock::new(|| {
    let u = url::Url::parse("about:bench").unwrap();
    stylo::stylesheets::UrlExtraData(ServoArc::new(u))
});

// Dummy font metrics provider for Stylo Device construction.
#[derive(Debug)]
struct DummyFontMetrics;

impl stylo::device::servo::FontMetricsProvider for DummyFontMetrics {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        _font: &stylo::properties::style_structs::Font,
        _base_size: stylo::values::computed::CSSPixelLength,
        _flags: stylo::values::specified::font::QueryFontMetricsFlags,
    ) -> stylo::font_metrics::FontMetrics {
        Default::default()
    }
    fn base_size_for_generic(
        &self,
        _generic: stylo::values::computed::font::GenericFontFamily,
    ) -> stylo::values::computed::Length {
        stylo::values::computed::Length::new(16.0)
    }
}

fn stylo_device(width: f32, height: f32) -> stylo::device::Device {
    use euclid::{Scale, Size2D};
    use style_traits::{CSSPixel, DevicePixel};

    let default_font = stylo::properties::style_structs::Font::initial_values();
    let default_values =
        stylo::properties::ComputedValues::initial_values_with_font_override(default_font);

    stylo::device::Device::new(
        stylo::media_queries::MediaType::screen(),
        stylo::context::QuirksMode::NoQuirks,
        Size2D::<f32, CSSPixel>::new(width, height),
        Scale::<f32, CSSPixel, DevicePixel>::new(1.0),
        Box::new(DummyFontMetrics),
        default_values,
        stylo::queries::values::PrefersColorScheme::Light,
    )
}

fn stylo_parse_and_rebuild(css: &str) -> usize {
    use stylo::media_queries::MediaList;
    use stylo::shared_lock::StylesheetGuards;
    use stylo::stylesheets::{AllowImportRules, DocumentStyleSheet, Origin, Stylesheet};

    let lock = STYLO_LOCK.clone();
    let media = ServoArc::new(lock.wrap(MediaList::empty()));
    let sheet = Stylesheet::from_str(
        css,
        STYLO_URL.clone(),
        Origin::Author,
        media,
        lock,
        None,
        None,
        stylo::context::QuirksMode::NoQuirks,
        AllowImportRules::Yes,
    );
    let doc_sheet = DocumentStyleSheet(ServoArc::new(sheet));

    let device = stylo_device(1024.0, 768.0);
    let mut stylist = stylo::stylist::Stylist::new(device, stylo::context::QuirksMode::NoQuirks);

    let guard = STYLO_LOCK.read();
    stylist.append_stylesheet(doc_sheet, &guard);

    let guards = StylesheetGuards::same(&guard);
    stylist.flush(&guards);

    stylist.num_selectors()
}

fn kozan_parse_and_rebuild(css: &str) -> usize {
    let sheet = parse_stylesheet(css);
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(sheet, CascadeOrigin::Author);
    stylist.rebuild();
    stylist.rule_count()
}

// ─── HELPERS ───

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
        layer_order: 0,
        scope_depth: 0,
    }
}

fn sharing_key(tag: &str, classes: &[&str], parent: u64) -> SharingKey {
    SharingKey::new(
        Atom::from(tag),
        None,
        classes.iter().map(|c| Atom::from(*c)).collect(),
        0,
        parent,
    )
}

// ─── CSS FIXTURES ───

fn gen_css_rules(n: usize) -> String {
    (0..n)
        .map(|i| format!(".c{i} {{ color: red; display: block }}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn gen_css_layered(n: usize) -> String {
    let mut css = String::from("@layer base, utils, overrides;\n");
    for i in 0..n / 3 {
        css.push_str(&format!("@layer base {{ .b{i} {{ color: red }} }}\n"));
    }
    for i in 0..n / 3 {
        css.push_str(&format!("@layer utils {{ .u{i} {{ display: flex }} }}\n"));
    }
    for i in 0..n / 3 {
        css.push_str(&format!(
            "@layer overrides {{ .o{i} {{ margin: 0 }} }}\n"
        ));
    }
    css
}

fn gen_css_media(n: usize) -> String {
    let mut css = String::new();
    for i in 0..n / 2 {
        css.push_str(&format!(".plain{i} {{ color: red }}\n"));
    }
    css.push_str("@media (min-width: 768px) {\n");
    for i in 0..n / 4 {
        css.push_str(&format!("  .tablet{i} {{ display: flex }}\n"));
    }
    css.push_str("}\n");
    css.push_str("@media (min-width: 1200px) {\n");
    for i in 0..n / 4 {
        css.push_str(&format!("  .desktop{i} {{ grid-template-columns: 1fr }}\n"));
    }
    css.push_str("}\n");
    css
}

// ═══════════════════════════════════════════════════
// BENCHMARK 1: STYLIST REBUILD — kozan vs Stylo
// ═══════════════════════════════════════════════════

fn bench_stylist_rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("stylist_rebuild");
    group.sample_size(30);

    let inputs = [
        ("10_plain", gen_css_rules(10)),
        ("100_plain", gen_css_rules(100)),
        ("500_plain", gen_css_rules(500)),
        ("100_layered", gen_css_layered(99)),
        ("100_media", gen_css_media(100)),
    ];

    for (name, css) in &inputs {
        group.bench_with_input(BenchmarkId::new("kozan", name), css, |b, css| {
            b.iter(|| black_box(kozan_parse_and_rebuild(black_box(css))))
        });

        group.bench_with_input(BenchmarkId::new("stylo", name), css, |b, css| {
            b.iter(|| black_box(stylo_parse_and_rebuild(black_box(css))))
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 2: STYLIST REBUILD ONLY (no parse)
// ═══════════════════════════════════════════════════

fn bench_stylist_rebuild_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("stylist_rebuild_only");
    group.sample_size(50);

    // Pre-parse sheets, only benchmark the rebuild step.
    let css_100 = gen_css_rules(100);
    let css_500 = gen_css_rules(500);

    for (name, css) in [("100_rules", &css_100), ("500_rules", &css_500)] {
        // Kozan: pre-parse, then measure rebuild-only.
        // We add the sheet once and call rebuild() repeatedly — rebuild()
        // takes ownership of sheets via mem::take and puts them back.
        group.bench_function(&format!("kozan/{name}"), |b| {
            let sheet = parse_stylesheet(css);
            let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
            stylist.add_stylesheet(sheet, CascadeOrigin::Author);
            b.iter(|| {
                stylist.rebuild();
                black_box(stylist.rule_count())
            })
        });

        // Stylo: pre-parse, only measure flush
        let lock = STYLO_LOCK.clone();
        let media = ServoArc::new(lock.wrap(stylo::media_queries::MediaList::empty()));
        let sheet = stylo::stylesheets::Stylesheet::from_str(
            css,
            STYLO_URL.clone(),
            stylo::stylesheets::Origin::Author,
            media,
            lock,
            None,
            None,
            stylo::context::QuirksMode::NoQuirks,
            stylo::stylesheets::AllowImportRules::Yes,
        );
        let doc_sheet = stylo::stylesheets::DocumentStyleSheet(ServoArc::new(sheet));

        group.bench_function(&format!("stylo/{name}"), |b| {
            b.iter(|| {
                let device = stylo_device(1024.0, 768.0);
                let mut stylist =
                    stylo::stylist::Stylist::new(device, stylo::context::QuirksMode::NoQuirks);
                let guard = STYLO_LOCK.read();
                stylist.append_stylesheet(doc_sheet.clone(), &guard);
                let guards = stylo::shared_lock::StylesheetGuards::same(&guard);
                stylist.flush(&guards);
                black_box(stylist.num_selectors())
            })
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 3: CASCADE SORT (kozan-only, Stylo sort is internal)
// ═══════════════════════════════════════════════════

fn bench_cascade_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("cascade_sort");

    for count in [10, 50, 100, 500] {
        let rules: Vec<IndexedRule> = (0..count)
            .map(|i| {
                let origin = match i % 3 {
                    0 => CascadeOrigin::UserAgent,
                    1 => CascadeOrigin::User,
                    _ => CascadeOrigin::Author,
                };
                let layer = if i % 5 == 0 { (i % 10) as u16 } else { UNLAYERED };
                make_rule(origin, layer)
            })
            .collect();

        let decls: Vec<ApplicableDeclaration> = (0..count)
            .map(|i| make_decl(i as u32, (i * 7 % 1000) as u32, i as u32))
            .collect();

        group.bench_with_input(BenchmarkId::new("decls", count), &count, |b, _| {
            b.iter(|| {
                let mut d = decls.clone();
                cascade::sort(black_box(&mut d), black_box(&rules));
                black_box(&d);
            })
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 4: CASCADE LEVEL CONSTRUCTION
// ═══════════════════════════════════════════════════

fn bench_cascade_level(c: &mut Criterion) {
    c.bench_function("cascade_level_new", |b| {
        b.iter(|| {
            black_box(CascadeLevel::new(
                black_box(CascadeOrigin::Author),
                black_box(Importance::Normal),
                black_box(UNLAYERED),
            ))
        })
    });

    c.bench_function("cascade_level_compare", |b| {
        let a = CascadeLevel::new(CascadeOrigin::UserAgent, Importance::Normal, UNLAYERED);
        let b_level = CascadeLevel::new(CascadeOrigin::Author, Importance::Important, 5);
        b.iter(|| black_box(black_box(a) < black_box(b_level)))
    });
}

// ═══════════════════════════════════════════════════
// BENCHMARK 5: MEDIA QUERY EVALUATION
// ═══════════════════════════════════════════════════

fn bench_media_eval(c: &mut Criterion) {
    let device = Device::new(1024.0, 768.0);

    c.bench_function("media_eval/single_width", |b| {
        let query = MediaQueryList(
            vec![MediaQuery {
                qualifier: None,
                media_type: CssMediaType::All,
                condition: Some(MediaCondition::Feature(MediaFeature::Range {
                    name: Atom::from("min-width"),
                    op: RangeOp::Ge,
                    value: MediaFeatureValue::Length(768.0, LengthUnit::Px),
                })),
            }]
            .into(),
        );
        b.iter(|| black_box(media::evaluate(black_box(&query), black_box(&device))))
    });

    c.bench_function("media_eval/and_condition", |b| {
        let query = MediaQueryList(
            vec![MediaQuery {
                qualifier: None,
                media_type: CssMediaType::Screen,
                condition: Some(MediaCondition::And(smallvec::smallvec![
                    Box::new(MediaCondition::Feature(MediaFeature::Range {
                        name: Atom::from("min-width"),
                        op: RangeOp::Ge,
                        value: MediaFeatureValue::Length(768.0, LengthUnit::Px),
                    })),
                    Box::new(MediaCondition::Feature(MediaFeature::Range {
                        name: Atom::from("max-width"),
                        op: RangeOp::Le,
                        value: MediaFeatureValue::Length(1200.0, LengthUnit::Px),
                    })),
                ])),
            }]
            .into(),
        );
        b.iter(|| black_box(media::evaluate(black_box(&query), black_box(&device))))
    });

    c.bench_function("media_eval/empty_list", |b| {
        let query = MediaQueryList::empty();
        b.iter(|| black_box(media::evaluate(black_box(&query), black_box(&device))))
    });
}

// ═══════════════════════════════════════════════════
// BENCHMARK 6: SHARING CACHE (with/without hits)
// ═══════════════════════════════════════════════════

fn make_filled_sharing_cache() -> SharingCache {
    let mut cache = SharingCache::new();
    for i in 0u32..32 {
        cache.insert(sharing_key("div", &[&format!("c{i}")], 0), dummy_resolved());
    }
    cache
}

fn bench_sharing_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("sharing_cache");

    group.bench_function("hit_front", |b| {
        let mut cache = make_filled_sharing_cache();
        let k = sharing_key("div", &["c31"], 0);
        b.iter(|| black_box(cache.get(black_box(&k)).is_some()))
    });

    group.bench_function("hit_back", |b| {
        let mut cache = make_filled_sharing_cache();
        let k = sharing_key("div", &["c0"], 0);
        b.iter(|| black_box(cache.get(black_box(&k)).is_some()))
    });

    group.bench_function("miss", |b| {
        let mut cache = make_filled_sharing_cache();
        let k = sharing_key("span", &["missing"], 0);
        b.iter(|| black_box(cache.get(black_box(&k)).is_some()))
    });

    group.bench_function("insert_evict", |b| {
        b.iter_batched(
            make_filled_sharing_cache,
            |mut cache| {
                cache.insert(black_box(sharing_key("p", &["new"], 0)), black_box(dummy_resolved()));
                black_box(&cache);
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("insert_empty", |b| {
        b.iter_batched(
            SharingCache::new,
            |mut cache| {
                cache.insert(black_box(sharing_key("div", &["btn"], 0)), black_box(dummy_resolved()));
                black_box(&cache);
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 7: MATCHED PROPERTIES CACHE
// ═══════════════════════════════════════════════════

fn bench_mpc(c: &mut Criterion) {
    let mut group = c.benchmark_group("matched_properties_cache");

    let mut cache = MatchedPropertiesCache::new();
    for i in 0u64..1000 {
        cache.insert(i * 17 + 31, dummy_resolved());
    }

    group.bench_function("hit", |b| {
        b.iter(|| black_box(cache.get(black_box(17 * 500 + 31))))
    });

    group.bench_function("miss", |b| {
        b.iter(|| black_box(cache.get(black_box(99999999))))
    });

    group.bench_function("insert", |b| {
        b.iter_batched(
            MatchedPropertiesCache::new,
            |mut cache| {
                cache.insert(black_box(12345), black_box(dummy_resolved()));
                black_box(&cache);
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 8: HASH MATCHED DECLARATIONS
// ═══════════════════════════════════════════════════

fn bench_hash_matched(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_matched");

    for count in [1, 5, 10, 50] {
        let decls: Vec<ApplicableDeclaration> = (0..count)
            .map(|i| make_decl(i, i * 10, i))
            .collect();

        group.bench_with_input(BenchmarkId::new("decls", count), &count, |b, _| {
            b.iter(|| black_box(hash_matched(black_box(&decls))))
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════
// BENCHMARK 9: CASCADE APPLY (two-pass)
// ═══════════════════════════════════════════════════

fn bench_cascade_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("cascade_apply");

    for count in [10, 50, 100] {
        let rules: Vec<IndexedRule> = (0..count)
            .map(|_| make_rule(CascadeOrigin::Author, UNLAYERED))
            .collect();

        let sorted: Vec<ApplicableDeclaration> = (0..count)
            .map(|i| make_decl(i as u32, i as u32 * 10, i as u32))
            .collect();

        group.bench_with_input(BenchmarkId::new("rules", count), &count, |b, _| {
            b.iter(|| {
                let mut applied = 0u32;
                cascade::cascade_apply(
                    black_box(&sorted),
                    black_box(&rules),
                    |_rule, _level, _imp| {
                        applied += 1;
                    },
                );
                black_box(applied)
            })
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════
// PER-ELEMENT PIPELINE BENCHMARKS
// ═══════════════════════════════════════════════════
//
// These benchmarks measure the full per-element style resolution pipeline
// with realistic CSS and DOM elements. Each benchmark isolates a different
// cache/no-cache scenario to show where time is actually spent.

use kozan_selector::bloom::AncestorBloom;
use kozan_selector::context::{MatchingContext, QuirksMode, SelectorCaches};
use kozan_selector::element::Element;
use kozan_selector::opaque::OpaqueElement;
use kozan_selector::pseudo_class::ElementState;
use smallvec::SmallVec;

/// Benchmark element — implements Element trait for realistic per-element matching.
///
/// Simulates a typical DOM element with tag, optional ID, classes, state,
/// and tree position. All identifiers use interned Atoms for O(1) comparison.
#[derive(Clone)]
struct BenchElement {
    tag: Atom,
    id: Option<Atom>,
    classes: SmallVec<[Atom; 4]>,
    el_state: ElementState,
    identity: u64,
    parent_identity: u64,
    is_root: bool,
    child_idx: u32,
    child_cnt: u32,
}

impl BenchElement {
    fn new(tag: &str, id: Option<&str>, classes: &[&str], identity: u64) -> Self {
        Self {
            tag: Atom::from(tag),
            id: id.map(Atom::from),
            classes: classes.iter().map(|c| Atom::from(*c)).collect(),
            el_state: ElementState::empty(),
            identity,
            parent_identity: 0,
            is_root: false,
            child_idx: 1,
            child_cnt: 1,
        }
    }

    fn with_parent(mut self, parent: u64) -> Self {
        self.parent_identity = parent;
        self
    }

    fn with_state(mut self, state: ElementState) -> Self {
        self.el_state = state;
        self
    }
}

impl Element for BenchElement {
    fn local_name(&self) -> &Atom { &self.tag }
    fn id(&self) -> Option<&Atom> { self.id.as_ref() }
    fn has_class(&self, class: &Atom) -> bool { self.classes.iter().any(|c| c == class) }
    fn each_class<F: FnMut(&Atom)>(&self, mut f: F) {
        for c in &self.classes { f(c); }
    }
    fn attr(&self, _: &Atom) -> Option<&str> { None }
    fn parent_element(&self) -> Option<Self> { None }
    fn prev_sibling_element(&self) -> Option<Self> { None }
    fn next_sibling_element(&self) -> Option<Self> { None }
    fn first_child_element(&self) -> Option<Self> { None }
    fn last_child_element(&self) -> Option<Self> { None }
    fn state(&self) -> ElementState { self.el_state }
    fn is_root(&self) -> bool { self.is_root }
    fn is_empty(&self) -> bool { true }
    fn child_index(&self) -> u32 { self.child_idx }
    fn child_count(&self) -> u32 { self.child_cnt }
    fn child_index_of_type(&self) -> u32 { self.child_idx }
    fn child_count_of_type(&self) -> u32 { self.child_cnt }
    fn opaque(&self) -> OpaqueElement { OpaqueElement::new(self.identity) }
}

/// Generate realistic CSS like a real web app: mix of IDs, classes, tags, combos.
fn gen_realistic_css(n: usize) -> String {
    let mut css = String::with_capacity(n * 60);
    for i in 0..n {
        match i % 10 {
            0 => css.push_str(&format!("#item{i} {{ color: red; display: flex }}\n")),
            1 => css.push_str(&format!(".btn-{i} {{ background: blue; padding: 8px }}\n")),
            2 => css.push_str(&format!(".card .title-{i} {{ font-size: 16px }}\n")),
            3 => css.push_str(&format!("div.container-{i} {{ margin: 0 auto }}\n")),
            4 => css.push_str(&format!(".nav .link-{i}:hover {{ color: white }}\n")),
            5 => css.push_str(&format!("h{} {{ font-weight: bold }}\n", (i % 6) + 1)),
            6 => css.push_str(&format!(".flex-{i} {{ display: flex; gap: 8px }}\n")),
            7 => css.push_str(&format!("* {{ box-sizing: border-box }}\n")),
            8 => css.push_str(&format!(".text-{i}, .label-{i} {{ color: #333 }}\n")),
            _ => css.push_str(&format!(".grid-{i} > div {{ grid-column: span 2 }}\n")),
        }
    }
    css
}

/// Build a Stylist with realistic CSS and return it ready for matching.
fn build_stylist_for_bench(rule_count: usize) -> Stylist {
    let css = gen_realistic_css(rule_count);
    let sheet = parse_stylesheet(&css);
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(sheet, CascadeOrigin::Author);
    stylist.rebuild();
    stylist
}

/// Create a set of diverse test elements that will match different amounts of rules.
fn bench_elements() -> Vec<BenchElement> {
    vec![
        // Element matching universal + type rules
        BenchElement::new("div", None, &[], 1).with_parent(100),
        // Element matching class rules
        BenchElement::new("div", None, &["btn-1", "flex-6"], 2).with_parent(100),
        // Element matching ID rules
        BenchElement::new("div", Some("item0"), &["card"], 3).with_parent(100),
        // Element with hover state
        BenchElement::new("a", None, &["link-4"], 4)
            .with_parent(100)
            .with_state(ElementState::HOVER),
        // Element matching many classes (Tailwind-style)
        BenchElement::new("div", None, &["flex-6", "text-8", "grid-9", "btn-1"], 5)
            .with_parent(100),
        // Element matching nothing special (miss case)
        BenchElement::new("span", None, &["unique-no-match"], 6).with_parent(100),
        // Heading element
        BenchElement::new("h1", None, &[], 7).with_parent(100),
    ]
}

fn bench_per_element_rule_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_element/rule_matching");

    for rule_count in [50, 200, 500] {
        let stylist = build_stylist_for_bench(rule_count);
        let elements = bench_elements();
        let map = stylist.author_map();

        group.bench_function(
            &format!("{rule_count}_rules/find_matching"),
            |b| {
                b.iter(|| {
                    for el in &elements {
                        black_box(map.find_matching(black_box(el)));
                    }
                })
            },
        );

        group.bench_function(
            &format!("{rule_count}_rules/for_each_matching"),
            |b| {
                b.iter(|| {
                    for el in &elements {
                        let mut count = 0u32;
                        map.for_each_matching(black_box(el), |_| count += 1);
                        black_box(count);
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_per_element_with_bloom(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_element/with_bloom");

    let stylist = build_stylist_for_bench(200);
    let elements = bench_elements();
    let map = stylist.author_map();

    // Bloom filter with some ancestors inserted (realistic scenario).
    let mut bloom = AncestorBloom::new();
    let parent = BenchElement::new("div", Some("app"), &["container", "main"], 100);
    let grandparent = BenchElement::new("body", None, &[], 99);
    bloom.push(&grandparent);
    bloom.push(&parent);

    group.bench_function("matching_in_context", |b| {
        b.iter(|| {
            let mut caches = SelectorCaches::new();
            let mut ctx = MatchingContext::for_restyle(
                &bloom,
                QuirksMode::NoQuirks,
                &mut caches,
            );
            for el in &elements {
                let mut count = 0u32;
                map.for_each_matching_in_context(black_box(el), &mut ctx, |_| count += 1);
                black_box(count);
            }
        })
    });

    group.bench_function("matching_no_bloom", |b| {
        b.iter(|| {
            for el in &elements {
                let mut count = 0u32;
                map.for_each_matching(black_box(el), |_| count += 1);
                black_box(count);
            }
        })
    });

    group.finish();
}

fn bench_per_element_full_pipeline_no_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_element/full_pipeline_no_cache");

    for rule_count in [50, 200, 500] {
        let stylist = build_stylist_for_bench(rule_count);
        let elements = bench_elements();
        let rules = stylist.rules();

        group.bench_function(&format!("{rule_count}_rules"), |b| {
            b.iter(|| {
                for el in &elements {
                    // Step 1: Match rules
                    let matches = stylist.author_map().find_matching(black_box(el));

                    // Step 2: Build applicable declarations
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

                    // Step 3: Sort by cascade priority
                    cascade::sort(&mut decls, rules);

                    // Step 4: Apply (simulate)
                    let mut applied = 0u32;
                    cascade::cascade_apply(&decls, rules, |_rule, _level, _imp| {
                        applied += 1;
                    });

                    black_box(applied);
                }
            })
        });
    }

    group.finish();
}

fn bench_per_element_sharing_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_element/sharing_cache");

    let stylist = build_stylist_for_bench(200);
    let elements = bench_elements();
    let rules = stylist.rules();

    // Pre-populate cache with all bench elements' styles.
    let mut cache = SharingCache::new();
    for (_i, el) in elements.iter().enumerate() {
        let key = SharingKey::new(
            el.tag.clone(),
            el.id.clone(),
            el.classes.clone(),
            el.el_state.bits() as u32,
            el.parent_identity,
        );
        cache.insert(key, dummy_resolved());
    }

    // Benchmark: all hits (warm cache)
    group.bench_function("all_hits", |b| {
        // Clone cache so repeated iterations don't change LRU state
        let mut c = cache.clone();
        b.iter(|| {
            for el in &elements {
                let key = SharingKey::new(
                    el.tag.clone(),
                    el.id.clone(),
                    el.classes.clone(),
                    el.el_state.bits() as u32,
                    el.parent_identity,
                );
                black_box(c.get(black_box(&key)));
            }
        })
    });

    // Benchmark: all misses → full pipeline fallback
    group.bench_function("all_misses_then_pipeline", |b| {
        let mut empty_cache = SharingCache::new();
        b.iter(|| {
            for el in &elements {
                let key = SharingKey::new(
                    el.tag.clone(),
                    el.id.clone(),
                    el.classes.clone(),
                    el.el_state.bits() as u32,
                    el.parent_identity,
                );
                let hit = empty_cache.get(black_box(&key));
                if hit.is_none() {
                    // Full pipeline on miss
                    let matches = stylist.author_map().find_matching(black_box(el));
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
                    cascade::cascade_apply(&decls, rules, |_rule, _level, _imp| {
                        applied += 1;
                    });
                    black_box(applied);
                }
            }
        })
    });

    // Benchmark: mixed (50% hit, 50% miss) — realistic scenario
    group.bench_function("mixed_hit_miss", |b| {
        // Cache only even-indexed elements
        let mut mixed_cache = SharingCache::new();
        for (i, el) in elements.iter().enumerate() {
            if i % 2 == 0 {
                let key = SharingKey::new(
                    el.tag.clone(),
                    el.id.clone(),
                    el.classes.clone(),
                    el.el_state.bits() as u32,
                    el.parent_identity,
                );
                mixed_cache.insert(key, dummy_resolved());
            }
        }
        b.iter(|| {
            for el in &elements {
                let key = SharingKey::new(
                    el.tag.clone(),
                    el.id.clone(),
                    el.classes.clone(),
                    el.el_state.bits() as u32,
                    el.parent_identity,
                );
                let hit = mixed_cache.get(black_box(&key));
                if hit.is_none() {
                    let matches = stylist.author_map().find_matching(black_box(el));
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
                    cascade::cascade_apply(&decls, rules, |_rule, _level, _imp| {
                        applied += 1;
                    });
                    black_box(applied);
                }
            }
        })
    });

    group.finish();
}

fn bench_per_element_mpc(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_element/mpc");

    let stylist = build_stylist_for_bench(200);
    let elements = bench_elements();
    let rules = stylist.rules();

    // Pre-compute MPC hashes and populate cache.
    let mut mpc = MatchedPropertiesCache::new();
    let mut hashes: Vec<u64> = Vec::new();
    for el in &elements {
        let matches = stylist.author_map().find_matching(el);
        let decls: Vec<ApplicableDeclaration> = matches
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
        let h = hash_matched(&decls);
        hashes.push(h);
        mpc.insert(h, dummy_resolved()); // All map to same style for simplicity
    }

    // MPC hit: match rules → hash → MPC lookup (skip cascade)
    group.bench_function("match_then_mpc_hit", |b| {
        b.iter(|| {
            for (i, el) in elements.iter().enumerate() {
                // Step 1: Match rules (still needed)
                let matches = stylist.author_map().find_matching(black_box(el));
                let decls: Vec<ApplicableDeclaration> = matches
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

                // Step 2: Hash matched rules
                let h = hash_matched(&decls);

                // Step 3: MPC lookup — HIT! Skip cascade.
                let result = mpc.get(black_box(h));
                black_box(result);
                let _ = i;
            }
        })
    });

    // MPC miss: match rules → hash → MPC miss → full cascade
    group.bench_function("match_then_mpc_miss", |b| {
        let empty_mpc = MatchedPropertiesCache::new();
        b.iter(|| {
            for el in &elements {
                let matches = stylist.author_map().find_matching(black_box(el));
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

                let h = hash_matched(&decls);
                let result = empty_mpc.get(black_box(h));

                if result.is_none() {
                    cascade::sort(&mut decls, rules);
                    let mut applied = 0u32;
                    cascade::cascade_apply(&decls, rules, |_rule, _level, _imp| {
                        applied += 1;
                    });
                    black_box(applied);
                }
            }
        })
    });

    group.finish();
}

/// End-to-end: simulates styling 100 elements with the full resolution pipeline
/// including sharing cache, MPC, and cascade — all layers active.
fn bench_per_element_e2e(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_element/e2e_100_elements");
    group.sample_size(30);

    let stylist = build_stylist_for_bench(200);
    let rules = stylist.rules();

    // Generate 100 diverse elements.
    let mut elements = Vec::with_capacity(100);
    let tags = ["div", "span", "p", "a", "button", "h1", "h2", "section", "header", "li"];
    let class_pool = [
        "btn-1", "flex-6", "text-8", "grid-9", "card", "nav", "active", "hidden",
        "primary", "secondary",
    ];
    for i in 0u64..100 {
        let tag = tags[i as usize % tags.len()];
        let n_classes = (i % 4) as usize;
        let classes: Vec<&str> = (0..n_classes)
            .map(|j| class_pool[(i as usize + j) % class_pool.len()])
            .collect();
        let id = if i % 5 == 0 {
            Some(format!("item{}", i / 5 * 10))
        } else {
            None
        };
        let el = BenchElement::new(
            tag,
            id.as_deref(),
            &classes,
            i + 1000,
        )
        .with_parent(if i < 10 { 0 } else { 1000 + i / 10 });

        elements.push(el);
    }

    group.bench_function("no_cache", |b| {
        b.iter(|| {
            let mut total = 0u32;
            for el in &elements {
                let matches = stylist.author_map().find_matching(black_box(el));
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
                cascade::cascade_apply(&decls, rules, |_rule, _level, _imp| {
                    applied += 1;
                });
                total += applied;
            }
            black_box(total)
        })
    });

    group.bench_function("with_sharing_cache", |b| {
        b.iter(|| {
            let mut cache = SharingCache::new();
            let mut total = 0u32;
            for el in &elements {
                let key = SharingKey::new(
                    el.tag.clone(),
                    el.id.clone(),
                    el.classes.clone(),
                    el.el_state.bits() as u32,
                    el.parent_identity,
                );
                if let Some(_idx) = cache.get(&key) {
                    total += 1; // Cache hit — skip everything
                    continue;
                }
                let matches = stylist.author_map().find_matching(black_box(el));
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
                cascade::cascade_apply(&decls, rules, |_rule, _level, _imp| {
                    applied += 1;
                });
                cache.insert(key, dummy_resolved());
                total += applied;
            }
            black_box(total)
        })
    });

    group.bench_function("with_sharing_and_mpc", |b| {
        b.iter(|| {
            let mut sharing = SharingCache::new();
            let mut mpc = MatchedPropertiesCache::new();
            let mut total = 0u32;
            for el in &elements {
                // Level 1: Sharing cache
                let key = SharingKey::new(
                    el.tag.clone(),
                    el.id.clone(),
                    el.classes.clone(),
                    el.el_state.bits() as u32,
                    el.parent_identity,
                );
                if let Some(_idx) = sharing.get(&key) {
                    total += 1;
                    continue;
                }

                // Level 2: Match rules
                let matches = stylist.author_map().find_matching(black_box(el));
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

                // Level 3: MPC check
                let h = hash_matched(&decls);
                if let Some(_idx) = mpc.get(h) {
                    sharing.insert(key, _idx.clone());
                    total += 1;
                    continue;
                }

                // Level 4: Full cascade
                cascade::sort(&mut decls, rules);
                let mut applied = 0u32;
                cascade::cascade_apply(&decls, rules, |_rule, _level, _imp| {
                    applied += 1;
                });
                mpc.insert(h, dummy_resolved());
                sharing.insert(key, dummy_resolved());
                total += applied;
            }
            black_box(total)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_stylist_rebuild,
    bench_stylist_rebuild_only,
    bench_cascade_sort,
    bench_cascade_level,
    bench_media_eval,
    bench_sharing_cache,
    bench_mpc,
    bench_hash_matched,
    bench_cascade_apply,
    bench_per_element_rule_matching,
    bench_per_element_with_bloom,
    bench_per_element_full_pipeline_no_cache,
    bench_per_element_sharing_cache,
    bench_per_element_mpc,
    bench_per_element_e2e,
);
criterion_main!(benches);
