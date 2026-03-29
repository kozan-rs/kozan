//! Benchmark: inline style parsing — kozan-css vs lightningcss vs Stylo.
//!
//! Tests parsing throughput across property categories:
//! keywords, lengths, colors, shorthands, inline blocks, variables, real-world.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

const KEYWORDS: &[&str] = &[
    "display: flex",
    "display: block",
    "display: inline-block",
    "display: grid",
    "display: none",
    "position: absolute",
    "position: relative",
    "position: fixed",
    "position: sticky",
    "overflow: hidden",
    "overflow: scroll",
    "overflow: auto",
    "visibility: hidden",
    "visibility: visible",
    "float: left",
    "float: right",
    "clear: both",
    "box-sizing: border-box",
    "pointer-events: none",
    "white-space: nowrap",
];

const LENGTHS: &[&str] = &[
    "width: 100px",
    "width: 50%",
    "width: 2.5em",
    "width: auto",
    "height: 100vh",
    "min-width: 0",
    "max-width: 1200px",
    "top: 0",
    "left: 10px",
    "right: 20%",
    "bottom: 0",
    "margin-top: 16px",
    "margin-bottom: 1.5rem",
    "padding-left: 24px",
    "padding-right: 2em",
    "gap: 8px",
    "flex-basis: 200px",
    "border-width: 1px",
    "font-size: 14px",
    "line-height: 1.5",
];

const COLORS: &[&str] = &[
    "color: red",
    "color: #ff0000",
    "color: #333",
    "color: #00000080",
    "color: currentcolor",
    "color: transparent",
    "background-color: #fff",
    "background-color: rgb(255, 128, 0)",
    "background-color: rgba(0, 0, 0, 0.5)",
    "background-color: hsl(210, 100%, 50%)",
    "border-color: #e5e7eb",
    "outline-color: blue",
    "color: rgb(51 65 85)",
    "color: hsl(0, 0%, 100%)",
    "color: oklch(0.7 0.15 180)",
];

const SHORTHANDS: &[&str] = &[
    "margin: 0",
    "margin: 10px 20px",
    "margin: 10px 20px 30px",
    "margin: 10px 20px 30px 40px",
    "padding: 16px",
    "padding: 8px 16px",
    "flex: 1",
    "flex: 1 0 auto",
    "border: 1px solid #ccc",
    "border-radius: 8px",
    "background: #fff",
    "overflow: hidden auto",
    "gap: 8px 16px",
    "inset: 0",
];

const INLINE_BLOCKS: &[&str] = &[
    "display: flex; width: 100px; height: 100px",
    "position: absolute; top: 0; left: 0; right: 0; bottom: 0",
    "display: flex; flex-direction: column; gap: 8px; padding: 16px",
    "color: #333; font-size: 14px; line-height: 1.5; font-weight: bold",
    "width: 100%; max-width: 1200px; margin: 0 auto; padding: 0 16px",
    "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 16px",
    "border: 1px solid #e5e7eb; border-radius: 8px; overflow: hidden",
    "background-color: rgba(0,0,0,0.5); color: white; padding: 8px 16px; border-radius: 4px",
];

const VARIABLES: &[&str] = &[
    "width: var(--gap)",
    "color: var(--primary)",
    "margin: var(--spacing-md)",
    "padding: var(--pad, 16px)",
    "width: calc(100% - var(--sidebar))",
    "height: calc(var(--header) + var(--content))",
    "font-size: var(--text-base, 1rem)",
    "background: var(--bg-color, #fff)",
];

const CSS_WIDE: &[&str] = &[
    "display: inherit",
    "color: initial",
    "width: unset",
    "margin: revert",
    "padding: revert-layer",
    "font-size: inherit",
    "line-height: initial",
    "flex: unset",
    "overflow: revert",
    "position: inherit",
];

const REALWORLD: &[&str] = &[
    // Button
    "display: inline-flex; align-items: center; justify-content: center; padding: 8px 16px; font-size: 14px; font-weight: 500; border-radius: 6px; border: 1px solid transparent; cursor: pointer",
    // Card
    "display: flex; flex-direction: column; border: 1px solid #e5e7eb; border-radius: 12px; overflow: hidden; background-color: #fff",
    // Modal overlay
    "position: fixed; top: 0; left: 0; right: 0; bottom: 0; display: flex; align-items: center; justify-content: center; background-color: rgba(0,0,0,0.5); z-index: 1000",
    // Input field
    "width: 100%; padding: 8px 12px; font-size: 14px; line-height: 1.5; border: 1px solid #d1d5db; border-radius: 6px; outline: none; background-color: #fff; color: #111827",
    // Sidebar
    "position: fixed; top: 0; left: 0; width: 256px; height: 100vh; display: flex; flex-direction: column; background-color: #1f2937; color: #f9fafb; overflow-y: auto; padding: 16px",
    // Grid layout
    "display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 24px; padding: 24px; max-width: 1400px; margin: 0 auto",
];

fn kozan_parse(css: &str) -> kozan_style::DeclarationBlock {
    kozan_css::parse_inline(css)
}

fn lightning_parse(css: &str) -> usize {
    use lightningcss::stylesheet::{ParserOptions, StyleAttribute};
    let options = ParserOptions::default();
    match StyleAttribute::parse(css, options) {
        Ok(attr) => attr.declarations.declarations.len() + attr.declarations.important_declarations.len(),
        Err(_) => 0,
    }
}

static STYLO_URL: std::sync::LazyLock<stylo::stylesheets::UrlExtraData> = std::sync::LazyLock::new(|| {
    let u = url::Url::parse("about:bench").unwrap();
    stylo::stylesheets::UrlExtraData(servo_arc::Arc::new(u))
});

fn stylo_parse(css: &str) -> usize {
    use stylo::properties::parse_style_attribute;
    use stylo::stylesheets::CssRuleType;

    let block = parse_style_attribute(
        css,
        &STYLO_URL,
        None,
        stylo::context::QuirksMode::NoQuirks,
        CssRuleType::Style,
    );
    block.len()
}

fn bench_category(c: &mut Criterion, name: &str, inputs: &[&str]) {
    let mut group = c.benchmark_group(name);
    group.sample_size(50);
    group.warm_up_time(std::time::Duration::from_millis(500));
    group.measurement_time(std::time::Duration::from_secs(1));

    group.bench_function("kozan", |b| {
        b.iter(|| {
            for css in inputs {
                black_box(kozan_parse(css));
            }
        });
    });

    group.bench_function("lightningcss", |b| {
        b.iter(|| {
            for css in inputs {
                black_box(lightning_parse(css));
            }
        });
    });

    group.bench_function("stylo", |b| {
        b.iter(|| {
            for css in inputs {
                black_box(stylo_parse(css));
            }
        });
    });

    group.finish();
}

fn bench_keywords(c: &mut Criterion) { bench_category(c, "inline/keywords", KEYWORDS); }
fn bench_lengths(c: &mut Criterion) { bench_category(c, "inline/lengths", LENGTHS); }
fn bench_colors(c: &mut Criterion) { bench_category(c, "inline/colors", COLORS); }
fn bench_shorthands(c: &mut Criterion) { bench_category(c, "inline/shorthands", SHORTHANDS); }
fn bench_inline_blocks(c: &mut Criterion) { bench_category(c, "inline/blocks", INLINE_BLOCKS); }
fn bench_variables(c: &mut Criterion) { bench_category(c, "inline/variables", VARIABLES); }
fn bench_css_wide(c: &mut Criterion) { bench_category(c, "inline/css_wide", CSS_WIDE); }
fn bench_realworld(c: &mut Criterion) { bench_category(c, "inline/realworld", REALWORLD); }

criterion_group!(
    benches,
    bench_keywords,
    bench_lengths,
    bench_colors,
    bench_shorthands,
    bench_inline_blocks,
    bench_variables,
    bench_css_wide,
    bench_realworld,
);
criterion_main!(benches);
