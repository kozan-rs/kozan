//! Benchmark: full stylesheet parsing — kozan-css vs lightningcss vs Stylo.
//!
//! Tests real-world stylesheet parsing with Tailwind CSS output.
//! Stylo (Mozilla/Firefox) is the only true apples-to-apples comparison —
//! lightningcss is a transform tool that doesn't fully parse property values.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::LazyLock;

const TAILWIND_CSS: &str = include_str!("../fixtures/tailwind_full.css");
const TAILWIND_PREFLIGHT: &str = include_str!("../fixtures/tailwind_preflight.css");

const SIMPLE_RULES: &str = "\
.a { color: red; }
.b { display: flex; gap: 8px; }
.c { margin: 0 auto; padding: 16px; }
.d { width: 100%; max-width: 1200px; }
.e { position: absolute; top: 0; left: 0; right: 0; bottom: 0; }
";

const MEDIA_RULES: &str = "\
.base { display: block; }
@media (min-width: 640px) { .sm\\:flex { display: flex; } .sm\\:grid { display: grid; } }
@media (min-width: 768px) { .md\\:flex { display: flex; } .md\\:hidden { display: none; } }
@media (min-width: 1024px) { .lg\\:flex { display: flex; } .lg\\:grid-cols-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); } }
@media (prefers-color-scheme: dark) { .dark\\:bg-gray-900 { background-color: #111827; } .dark\\:text-white { color: #fff; } }
";

const KEYFRAMES: &str = "\
@keyframes spin { to { transform: rotate(360deg); } }
@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }
@keyframes slide-up { from { transform: translateY(10px); opacity: 0; } to { transform: none; opacity: 1; } }
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }
.animate-spin { animation: spin 1s linear infinite; }
.animate-fade-in { animation: fade-in 0.3s ease-out; }
.animate-slide-up { animation: slide-up 0.4s ease-out; }
.animate-pulse { animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite; }
";

const NESTED_RULES: &str = "\
@layer base {
  *, ::before, ::after { box-sizing: border-box; }
  html { line-height: 1.5; }
  body { margin: 0; }
}
@layer components {
  .btn { display: inline-flex; padding: 8px 16px; border-radius: 6px; }
  .card { border: 1px solid #e5e7eb; border-radius: 12px; overflow: hidden; }
}
@layer utilities {
  .flex { display: flex; }
  .grid { display: grid; }
  .hidden { display: none; }
}
";

const COMPLEX_SELECTORS: &str = "\
.container > .item:first-child { margin-top: 0; }
.nav a:hover, .nav a:focus { color: #3b82f6; text-decoration: underline; }
.form input:not([type='hidden']):not([type='submit']) { padding: 8px 12px; border: 1px solid #d1d5db; }
.table tr:nth-child(even) { background-color: #f9fafb; }
.sidebar .menu-item.active > .icon { color: #3b82f6; }
[data-theme='dark'] .card { background-color: #1f2937; border-color: #374151; }
.grid > :not([hidden]) ~ :not([hidden]) { margin-top: 16px; }
:is(.btn, .link):focus-visible { outline: 2px solid #3b82f6; outline-offset: 2px; }
";

fn kozan_parse_sheet(css: &str) -> usize {
    let sheet = kozan_css::parse_stylesheet(css);
    sheet.rules.slice.len()
}

fn lightning_parse_sheet(css: &str) -> usize {
    use lightningcss::stylesheet::{ParserOptions, StyleSheet};
    match StyleSheet::parse(css, ParserOptions::default()) {
        Ok(sheet) => sheet.rules.0.len(),
        Err(_) => 0,
    }
}

static STYLO_LOCK: LazyLock<stylo::shared_lock::SharedRwLock> =
    LazyLock::new(stylo::shared_lock::SharedRwLock::new);

static STYLO_URL: LazyLock<stylo::stylesheets::UrlExtraData> = LazyLock::new(|| {
    let u = url::Url::parse("about:bench").unwrap();
    stylo::stylesheets::UrlExtraData(servo_arc::Arc::new(u))
});

fn stylo_parse_sheet(css: &str) -> usize {
    use stylo::media_queries::MediaList;
    use stylo::stylesheets::{AllowImportRules, Origin, Stylesheet};

    let lock = STYLO_LOCK.clone();
    let media = servo_arc::Arc::new(lock.wrap(MediaList::empty()));

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
    let guard = STYLO_LOCK.read();
    let contents = sheet.contents.read_with(&guard);
    contents.rules(&guard).len()
}

fn bench_small_sheets(c: &mut Criterion) {
    let inputs = [
        ("simple_rules", SIMPLE_RULES),
        ("media_rules", MEDIA_RULES),
        ("keyframes", KEYFRAMES),
        ("nested_layers", NESTED_RULES),
        ("complex_selectors", COMPLEX_SELECTORS),
    ];

    let mut group = c.benchmark_group("stylesheet/small");
    group.sample_size(50);
    group.warm_up_time(std::time::Duration::from_millis(500));
    group.measurement_time(std::time::Duration::from_secs(1));

    for (name, css) in &inputs {
        group.throughput(Throughput::Bytes(css.len() as u64));

        group.bench_with_input(BenchmarkId::new("kozan", name), css, |b, css| {
            b.iter(|| black_box(kozan_parse_sheet(css)));
        });

        group.bench_with_input(BenchmarkId::new("lightningcss", name), css, |b, css| {
            b.iter(|| black_box(lightning_parse_sheet(css)));
        });

        group.bench_with_input(BenchmarkId::new("stylo", name), css, |b, css| {
            b.iter(|| black_box(stylo_parse_sheet(css)));
        });
    }

    group.finish();
}

fn bench_tailwind_preflight(c: &mut Criterion) {
    let mut group = c.benchmark_group("stylesheet/tailwind_preflight");
    group.sample_size(30);
    group.warm_up_time(std::time::Duration::from_millis(500));
    group.measurement_time(std::time::Duration::from_secs(2));
    group.throughput(Throughput::Bytes(TAILWIND_PREFLIGHT.len() as u64));

    group.bench_function("kozan", |b| {
        b.iter(|| black_box(kozan_parse_sheet(TAILWIND_PREFLIGHT)));
    });

    group.bench_function("lightningcss", |b| {
        b.iter(|| black_box(lightning_parse_sheet(TAILWIND_PREFLIGHT)));
    });

    group.bench_function("stylo", |b| {
        b.iter(|| black_box(stylo_parse_sheet(TAILWIND_PREFLIGHT)));
    });

    group.finish();
}

fn bench_tailwind_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("stylesheet/tailwind_full");
    group.sample_size(20);
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Bytes(TAILWIND_CSS.len() as u64));

    group.bench_function("kozan", |b| {
        b.iter(|| black_box(kozan_parse_sheet(TAILWIND_CSS)));
    });

    group.bench_function("lightningcss", |b| {
        b.iter(|| black_box(lightning_parse_sheet(TAILWIND_CSS)));
    });

    group.bench_function("stylo", |b| {
        b.iter(|| black_box(stylo_parse_sheet(TAILWIND_CSS)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_small_sheets,
    bench_tailwind_preflight,
    bench_tailwind_full,
);
criterion_main!(benches);
