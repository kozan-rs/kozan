//! Manual profiling: break down Tailwind full parse into exact phases.
//!
//! Run: cargo run -p kozan-bench --release --example profile_tailwind

use std::time::Instant;

const TAILWIND_CSS: &str = include_str!("../fixtures/tailwind_full.css");
const ITERS: u32 = 100;

fn main() {
    println!("Tailwind full: {} bytes, {} lines", TAILWIND_CSS.len(), TAILWIND_CSS.lines().count());
    println!("Running {} iterations each...\n", ITERS);

    // Warm up
    for _ in 0..5 {
        let _ = kozan_css::parse_stylesheet(TAILWIND_CSS);
    }

    // -----------------------------------------------------------------------
    // 1. Full kozan parse
    // -----------------------------------------------------------------------
    let start = Instant::now();
    for _ in 0..ITERS {
        std::hint::black_box(kozan_css::parse_stylesheet(std::hint::black_box(TAILWIND_CSS)));
    }
    let kozan_total = start.elapsed();
    let kozan_per = kozan_total / ITERS;

    // -----------------------------------------------------------------------
    // 2. Full lightningcss parse
    // -----------------------------------------------------------------------
    // Warm up
    for _ in 0..5 {
        let _ = lightningcss::stylesheet::StyleSheet::parse(
            TAILWIND_CSS,
            lightningcss::stylesheet::ParserOptions::default(),
        );
    }
    let start = Instant::now();
    for _ in 0..ITERS {
        let _ = std::hint::black_box(lightningcss::stylesheet::StyleSheet::parse(
            std::hint::black_box(TAILWIND_CSS),
            lightningcss::stylesheet::ParserOptions::default(),
        ));
    }
    let lcss_total = start.elapsed();
    let lcss_per = lcss_total / ITERS;

    // -----------------------------------------------------------------------
    // 3. Pure tokenization (cssparser baseline)
    // -----------------------------------------------------------------------
    let start = Instant::now();
    for _ in 0..ITERS {
        let mut input = cssparser::ParserInput::new(std::hint::black_box(TAILWIND_CSS));
        let mut parser = cssparser::Parser::new(&mut input);
        let mut count = 0u32;
        while parser.next_including_whitespace().is_ok() {
            count += 1;
        }
        std::hint::black_box(count);
    }
    let tokenize_per = start.elapsed() / ITERS;

    // -----------------------------------------------------------------------
    // 4. Selector-only parsing
    // -----------------------------------------------------------------------
    let selectors: Vec<&str> = TAILWIND_CSS.lines().filter_map(|line| {
        let t = line.trim();
        if t.starts_with('@') || t.starts_with('}') || t.is_empty() { return None; }
        t.find('{').map(|pos| t[..pos].trim())
    }).filter(|s| !s.is_empty()).collect();

    let start = Instant::now();
    for _ in 0..ITERS {
        for sel in &selectors {
            let _ = std::hint::black_box(kozan_selector::parser::parse(std::hint::black_box(sel)));
        }
    }
    let sel_per = start.elapsed() / ITERS;

    // -----------------------------------------------------------------------
    // 5. Declaration-only parsing (inline)
    // -----------------------------------------------------------------------
    let decl_blocks: Vec<String> = {
        let mut blocks = Vec::new();
        let mut buf = String::new();
        let mut in_block = false;
        for ch in TAILWIND_CSS.chars() {
            match ch {
                '{' => {
                    in_block = true;
                    buf.clear();
                    continue;
                }
                '}' => {
                    if in_block {
                        let t = buf.trim().to_string();
                        if !t.is_empty() && !t.starts_with('@') && t.contains(':') {
                            blocks.push(t);
                        }
                        in_block = false;
                    }
                    continue;
                }
                _ => {}
            }
            if in_block { buf.push(ch); }
        }
        blocks
    };

    let start = Instant::now();
    for _ in 0..ITERS {
        for block in &decl_blocks {
            std::hint::black_box(kozan_css::parse_inline(std::hint::black_box(block)));
        }
    }
    let decl_per = start.elapsed() / ITERS;

    // -----------------------------------------------------------------------
    // 6. Atom interning overhead
    // -----------------------------------------------------------------------
    let words: Vec<&str> = selectors.iter()
        .flat_map(|s| s.split(|c: char| !c.is_alphanumeric() && c != '-'))
        .filter(|w| !w.is_empty())
        .collect();

    let start = Instant::now();
    for _ in 0..ITERS {
        for w in &words {
            std::hint::black_box(kozan_atom::Atom::new(std::hint::black_box(*w)));
        }
    }
    let atom_per = start.elapsed() / ITERS;

    // -----------------------------------------------------------------------
    // 7. PropertyId dispatch (name → enum)
    // -----------------------------------------------------------------------
    let prop_names = [
        "color", "display", "margin", "padding", "width", "height",
        "background-color", "border", "font-size", "flex", "grid",
        "position", "top", "left", "right", "bottom", "overflow",
        "opacity", "z-index", "transform", "transition", "animation",
        "gap", "justify-content", "align-items", "flex-direction",
        "max-width", "min-height", "border-radius", "box-shadow",
    ];
    let start = Instant::now();
    for _ in 0..(ITERS * 100) {
        for name in &prop_names {
            let _ = std::hint::black_box(name.parse::<kozan_style::PropertyId>());
        }
    }
    let prop_dispatch_per = start.elapsed() / (ITERS * 100);

    // -----------------------------------------------------------------------
    // 8. Box + ThinArc allocation (simulate 2400 rules)
    // -----------------------------------------------------------------------
    let start = Instant::now();
    for _ in 0..ITERS {
        let mut rules: Vec<u64> = Vec::with_capacity(128);
        for i in 0..2400u64 {
            rules.push(std::hint::black_box(i));
        }
        // Simulate ThinArc creation
        std::hint::black_box(rules.len());
    }
    let alloc_per = start.elapsed() / ITERS;

    // -----------------------------------------------------------------------
    // 9. DeclarationBlock::new + push overhead (simulate 3600 decls)
    // -----------------------------------------------------------------------
    let start = Instant::now();
    for _ in 0..ITERS {
        for _ in 0..2400 {
            let mut block = kozan_style::DeclarationBlock::new();
            block.normal();
            // Simulate ~1.5 pushes per block
            std::hint::black_box(&mut block);
        }
    }
    let block_per = start.elapsed() / ITERS;

    // -----------------------------------------------------------------------
    // 10. try_parse overhead (simulate 3600 failed try_parses)
    // -----------------------------------------------------------------------
    // css_wide_keyword check always fails for normal values.
    // In the full parse, this runs for every single declaration.
    let sample_decl = "color: red";
    let start = Instant::now();
    for _ in 0..ITERS {
        for _ in 0..3600 {
            let mut pi = cssparser::ParserInput::new(std::hint::black_box(sample_decl));
            let mut p = cssparser::Parser::new(&mut pi);
            let _ = p.try_parse(|i| -> Result<(), cssparser::ParseError<'_, ()>> {
                let tok = i.expect_ident()?;
                if *tok == "inherit" || *tok == "initial" || *tok == "unset" {
                    Ok(())
                } else {
                    Err(i.new_custom_error(()))
                }
            });
            std::hint::black_box(&mut p);
        }
    }
    let try_parse_per = start.elapsed() / ITERS;

    // -----------------------------------------------------------------------
    // Results
    // -----------------------------------------------------------------------
    println!("=== PHASE BREAKDOWN ===");
    println!("  Tokenization:           {:>8.3}ms", tokenize_per.as_secs_f64() * 1000.0);
    println!("  Selector parsing:       {:>8.3}ms  ({} selectors)", sel_per.as_secs_f64() * 1000.0, selectors.len());
    println!("  Declaration parsing:    {:>8.3}ms  ({} blocks)", decl_per.as_secs_f64() * 1000.0, decl_blocks.len());
    println!("  Atom interning:         {:>8.3}ms  ({} atoms)", atom_per.as_secs_f64() * 1000.0, words.len());
    println!("  PropertyId dispatch:    {:>8.3}ms  ({} lookups)", prop_dispatch_per.as_secs_f64() * 1000.0, prop_names.len());
    println!();
    println!("  Sum of isolated phases: {:>8.3}ms", (tokenize_per + sel_per + decl_per).as_secs_f64() * 1000.0);
    println!();
    println!("=== FULL PARSE ===");
    println!("  kozan:        {:>8.3}ms", kozan_per.as_secs_f64() * 1000.0);
    println!("  lightningcss: {:>8.3}ms", lcss_per.as_secs_f64() * 1000.0);
    println!("  ratio:        {:>8.2}x", kozan_per.as_secs_f64() / lcss_per.as_secs_f64());
    println!();
    let overhead = kozan_per.as_secs_f64() - (tokenize_per + sel_per + decl_per).as_secs_f64();
    println!("  Integration overhead:   {:>8.3}ms ({:.0}% of total)",
        overhead * 1000.0,
        (overhead / kozan_per.as_secs_f64()) * 100.0);
    let gap = kozan_per.as_secs_f64() - lcss_per.as_secs_f64();
    println!("  Gap vs lightningcss:    {:>8.3}ms", gap * 1000.0);
    println!();
    println!("=== OVERHEAD SOURCES ===");
    println!("  Vec+ThinArc alloc (2400): {:>6.3}ms", alloc_per.as_secs_f64() * 1000.0);
    println!("  DeclarationBlock new:     {:>6.3}ms", block_per.as_secs_f64() * 1000.0);
    println!("  try_parse overhead:       {:>6.3}ms", try_parse_per.as_secs_f64() * 1000.0);
    println!("  Sum of measured overhead: {:>6.3}ms", (alloc_per + block_per + try_parse_per).as_secs_f64() * 1000.0);
    println!("  Remaining unexplained:   {:>6.3}ms", (overhead - (alloc_per + block_per + try_parse_per).as_secs_f64()) * 1000.0);
}
