//! Benchmark: kozan css_match! (integer chunks) vs Stylo match_ignore_ascii_case! (string compare).
//! Tests: short (2-4), medium (5-8), long (9-16), very long (17-30), hyphens, digits, many variants.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kozan_style_macros::css_match;
use cssparser::match_ignore_ascii_case;

macro_rules! dual_matcher {
    ($kozan_fn:ident, $stylo_fn:ident, { $( $css:literal => $val:expr ),+ $(,)? }) => {
        #[inline(never)]
        pub fn $kozan_fn(ident: &str) -> u32 {
            css_match! { ident, $( $css => $val, )+ _ => 0 }
        }

        #[inline(never)]
        pub fn $stylo_fn(ident: &str) -> u32 {
            match_ignore_ascii_case! { ident, $( $css => $val, )+ _ => 0 }
        }
    };
}

dual_matcher!(kozan_tiny, stylo_tiny, {
    "em" => 1, "ex" => 2, "ch" => 3, "ic" => 4, "lh" => 5,
    "px" => 6, "cm" => 7, "mm" => 8, "in" => 9, "pt" => 10,
    "pc" => 11, "vw" => 12, "vh" => 13, "fr" => 14,
    "rem" => 15, "cap" => 16, "rlh" => 17, "dvw" => 18, "svh" => 19,
});

dual_matcher!(kozan_short, stylo_short, {
    "disc" => 1, "none" => 2, "flex" => 3, "grid" => 4,
    "bold" => 5, "auto" => 6, "left" => 7, "both" => 8,
    "wrap" => 9, "ease" => 10, "fill" => 11, "clip" => 12,
    "ruby" => 13, "wavy" => 14,
});

dual_matcher!(kozan_hyphen, stylo_hyphen, {
    "sans-serif" => 1, "pre-wrap" => 2, "no-wrap" => 3, "sub" => 4,
    "pre-line" => 5, "srgb" => 6, "oklab" => 7, "oklch" => 8,
    "a98-rgb" => 9, "xyz-d50" => 10, "xyz-d65" => 11,
});

dual_matcher!(kozan_same_len, stylo_same_len, {
    "inline" => 1, "hidden" => 2, "scroll" => 3, "nowrap" => 4,
    "center" => 5, "normal" => 6, "italic" => 7, "double" => 8,
    "dotted" => 9, "dashed" => 10, "groove" => 11,
});

dual_matcher!(kozan_long, stylo_long, {
    "underline" => 1, "overline" => 2, "uppercase" => 3,
    "lowercase" => 4, "capitalize" => 5, "crosshair" => 6,
    "monospace" => 7, "condensed" => 8, "collapsed" => 9,
    "translate" => 10, "rotate" => 11,
});

dual_matcher!(kozan_long_hyphen, stylo_long_hyphen, {
    "inline-block" => 1, "inline-flex" => 2, "line-through" => 3,
    "break-word" => 4, "space-around" => 5, "space-between" => 6,
    "flex-start" => 7, "flex-end" => 8, "column-reverse" => 9,
    "wrap-reverse" => 10, "border-box" => 11, "content-box" => 12,
    "padding-box" => 13,
});

dual_matcher!(kozan_very_long, stylo_very_long, {
    "repeating-linear-gradient" => 1, "repeating-radial-gradient" => 2,
    "repeating-conic-gradient" => 3, "alternate-reverse" => 4,
    "allow-discrete" => 5, "ultra-condensed" => 6,
    "extra-condensed" => 7, "semi-condensed" => 8,
    "semi-expanded" => 9, "extra-expanded" => 10, "ultra-expanded" => 11,
});

dual_matcher!(kozan_mixed, stylo_mixed, {
    "rgb" => 1, "hsl" => 2, "lab" => 3, "lch" => 4,
    "oklch" => 5, "oklab" => 6, "color" => 7, "srgb" => 8,
    "display-p3" => 9, "srgb-linear" => 10, "prophoto-rgb" => 11,
    "a98-rgb" => 12, "rec2020" => 13, "xyz-d50" => 14, "xyz-d65" => 15,
    "color-mix" => 16, "light-dark" => 17, "transparent" => 18,
    "currentcolor" => 19,
});

dual_matcher!(kozan_digits, stylo_digits, {
    "matrix3d" => 1, "rotate3d" => 2, "scale3d" => 3,
    "translate3d" => 4, "rec2020" => 5, "level4" => 6,
    "h1" => 7, "h2" => 8, "h3" => 9,
});

fn bench_match_group(
    c: &mut Criterion,
    name: &str,
    kozan_fn: fn(&str) -> u32,
    stylo_fn: fn(&str) -> u32,
    inputs: &[&str],
) {
    let mut group = c.benchmark_group(name);
    group.sample_size(100);
    group.warm_up_time(std::time::Duration::from_millis(500));
    group.measurement_time(std::time::Duration::from_secs(2));

    group.bench_function("kozan_css_match", |b| {
        b.iter(|| {
            for s in inputs {
                black_box(kozan_fn(black_box(s)));
            }
        });
    });

    group.bench_function("stylo_match_ignore_ascii_case", |b| {
        b.iter(|| {
            for s in inputs {
                black_box(stylo_fn(black_box(s)));
            }
        });
    });

    group.finish();
}

fn bench_tiny(c: &mut Criterion) {
    bench_match_group(c, "match/tiny_2-3b", kozan_tiny, stylo_tiny,
        &["px", "PX", "REM", "dvw", "xxx", "em", "FR", "vh", "Lh", "cap"]);
}

fn bench_short(c: &mut Criterion) {
    bench_match_group(c, "match/short_4b", kozan_short, stylo_short,
        &["disc", "NONE", "Flex", "grid", "AUTO", "wavy", "zzzz", "BOLD", "wrap", "clip"]);
}

fn bench_same_len(c: &mut Criterion) {
    bench_match_group(c, "match/same_len_6b", kozan_same_len, stylo_same_len,
        &["inline", "HIDDEN", "Scroll", "CENTER", "dashed", "groove", "xxxxxx"]);
}

fn bench_hyphen(c: &mut Criterion) {
    bench_match_group(c, "match/hyphenated", kozan_hyphen, stylo_hyphen,
        &["sans-serif", "SANS-SERIF", "pre-wrap", "A98-RGB", "xyz-d50", "unknown"]);
}

fn bench_long(c: &mut Criterion) {
    bench_match_group(c, "match/long_9-12b", kozan_long, stylo_long,
        &["underline", "UPPERCASE", "Capitalize", "crosshair", "monospace", "xxxxxxxxx"]);
}

fn bench_long_hyphen(c: &mut Criterion) {
    bench_match_group(c, "match/long_hyphen_11-16b", kozan_long_hyphen, stylo_long_hyphen,
        &["inline-block", "COLUMN-REVERSE", "space-between", "content-box", "unknown-val"]);
}

fn bench_very_long(c: &mut Criterion) {
    bench_match_group(c, "match/very_long_15-25b", kozan_very_long, stylo_very_long,
        &["repeating-linear-gradient", "REPEATING-RADIAL-GRADIENT", "ultra-condensed",
          "extra-expanded", "nope-nope-nope-nope-nope"]);
}

fn bench_mixed(c: &mut Criterion) {
    bench_match_group(c, "match/mixed_real_world", kozan_mixed, stylo_mixed,
        &["rgb", "OKLCH", "display-p3", "currentcolor", "TRANSPARENT", "srgb-linear",
          "prophoto-rgb", "xyz-d65", "unknown-color"]);
}

fn bench_digits(c: &mut Criterion) {
    bench_match_group(c, "match/digits", kozan_digits, stylo_digits,
        &["matrix3d", "MATRIX3D", "translate3d", "REC2020", "h1", "H3", "xx"]);
}

criterion_group!(
    benches,
    bench_tiny,
    bench_short,
    bench_same_len,
    bench_hyphen,
    bench_long,
    bench_long_hyphen,
    bench_very_long,
    bench_mixed,
    bench_digits,
);
criterion_main!(benches);
