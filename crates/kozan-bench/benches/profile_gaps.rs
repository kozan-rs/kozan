//! Diagnostic benchmark: isolate where kozan-css spends time on Tailwind.
//!
//! Breaks down into phases:
//! 1. Tokenization only (cssparser baseline)
//! 2. Selector parsing only
//! 3. Declaration parsing only (inline)
//! 4. Full stylesheet parsing (kozan vs lightningcss reference)
//!
//! Comparing phase times to full parse reveals which phase dominates.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

const TAILWIND_CSS: &str = include_str!("../fixtures/tailwind_full.css");

fn bench_tokenize_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("profile/1_tokenize");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Bytes(TAILWIND_CSS.len() as u64));

    group.bench_function("cssparser_tokens", |b| {
        b.iter(|| {
            let mut input = cssparser::ParserInput::new(TAILWIND_CSS);
            let mut parser = cssparser::Parser::new(&mut input);
            let mut count = 0u32;
            while parser.next_including_whitespace().is_ok() {
                count += 1;
            }
            black_box(count)
        });
    });

    group.finish();
}

fn extract_selector_strings(css: &str) -> Vec<&str> {
    let mut selectors = Vec::new();
    for line in css.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('@') || trimmed.starts_with('}') || trimmed.is_empty() {
            continue;
        }
        if let Some(brace_pos) = trimmed.find('{') {
            let sel = trimmed[..brace_pos].trim();
            if !sel.is_empty() {
                selectors.push(sel);
            }
        }
    }
    selectors
}

fn bench_selectors_only(c: &mut Criterion) {
    let selectors = extract_selector_strings(TAILWIND_CSS);
    let total_bytes: usize = selectors.iter().map(|s| s.len()).sum();

    let mut group = c.benchmark_group("profile/2_selectors");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Bytes(total_bytes as u64));

    group.bench_function("kozan", |b| {
        b.iter(|| {
            let mut count = 0u32;
            for sel in &selectors {
                if kozan_selector::parser::parse(sel).is_ok() {
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.finish();
}

fn extract_declaration_blocks(css: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut depth = 0i32;
    let mut current_block = String::new();
    let mut in_block = false;

    for ch in css.chars() {
        match ch {
            '{' => {
                depth += 1;
                if depth >= 1 {
                    in_block = true;
                    current_block.clear();
                    continue;
                }
            }
            '}' => {
                if in_block {
                    let trimmed = current_block.trim().to_string();
                    if !trimmed.is_empty() && !trimmed.starts_with('@') && trimmed.contains(':') {
                        blocks.push(trimmed);
                    }
                    in_block = false;
                }
                depth -= 1;
                continue;
            }
            _ => {}
        }
        if in_block {
            current_block.push(ch);
        }
    }
    blocks
}

fn bench_declarations_only(c: &mut Criterion) {
    let blocks = extract_declaration_blocks(TAILWIND_CSS);
    let total_bytes: usize = blocks.iter().map(|s| s.len()).sum();

    let mut group = c.benchmark_group("profile/3_declarations");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Bytes(total_bytes as u64));

    group.bench_function("kozan", |b| {
        b.iter(|| {
            let mut count = 0u32;
            for block in &blocks {
                let decls = kozan_css::parse_inline(block);
                count += decls.len() as u32;
            }
            black_box(count)
        });
    });

    group.finish();
}

fn bench_full_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("profile/4_full_parse");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Bytes(TAILWIND_CSS.len() as u64));

    group.bench_function("kozan", |b| {
        b.iter(|| {
            let sheet = kozan_css::parse_stylesheet(TAILWIND_CSS);
            black_box(sheet.rules.slice.len())
        });
    });

    group.bench_function("lightningcss", |b| {
        b.iter(|| {
            let sheet = lightningcss::stylesheet::StyleSheet::parse(
                TAILWIND_CSS,
                lightningcss::stylesheet::ParserOptions::default(),
            );
            black_box(sheet.map(|s| s.rules.0.len()).unwrap_or(0))
        });
    });

    group.finish();
}

fn bench_allocation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("profile/5_allocation");
    group.sample_size(50);
    group.measurement_time(std::time::Duration::from_secs(2));

    // Box::new overhead for 1000 style rules
    group.bench_function("box_new_1000", |b| {
        b.iter(|| {
            let mut count = 0u64;
            for i in 0..1000u64 {
                let b = Box::new(i);
                count += *b;
            }
            black_box(count)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tokenize_only,
    bench_selectors_only,
    bench_declarations_only,
    bench_full_parse,
    bench_allocation_overhead,
);
criterion_main!(benches);
