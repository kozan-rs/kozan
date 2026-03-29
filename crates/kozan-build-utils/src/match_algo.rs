//! # CSS Keyword Matching — Zero-Alloc, O(1) Algorithms
//!
//! Integer-chunk case-insensitive matching for CSS identifiers.
//! Shared between kozan-style (PropertyId::from_str) and kozan-css (Parse impls).
//!
//! ## Algorithm: Length-First If-Chain with `| 0x20` Lowercasing
//!
//! ```text
//! Step 1: match s.len()        ← free (len stored in fat pointer)
//! Step 2: load bytes as u32/u64 → OR with 0x20 mask → CMP with target
//!         = 3 CPU instructions for any keyword ≤8 bytes
//! ```

use std::collections::BTreeMap;
use crate::CodeWriter;

/// Decomposes a length into (offset, chunk_size) pairs: u64 → u32 → u16 → u8.
pub fn int_chunks(len: usize) -> Vec<(usize, usize)> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < len {
        let remaining = len - offset;
        let chunk = if remaining >= 8 { 8 }
                    else if remaining >= 4 { 4 }
                    else if remaining >= 2 { 2 }
                    else { 1 };
        chunks.push((offset, chunk));
        offset += chunk;
    }
    chunks
}

/// Generates an integer comparison condition for a CSS keyword.
///
/// CSS identifiers contain only [a-zA-Z0-9\-]. For all of these, `| 0x20`
/// is safe: letters → lowercase, digits/hyphens unchanged (bit 5 already set).
pub fn gen_int_condition(css: &str, prefix: &str) -> String {
    let bytes = css.as_bytes();
    let chunks = int_chunks(bytes.len());

    let parts: Vec<String> = chunks.iter().map(|&(offset, size)| {
        let lower: Vec<u8> = (0..size).map(|i| bytes[offset + i].to_ascii_lowercase()).collect();
        match size {
            1 => {
                format!("{prefix}[{offset}] | 0x20 == b'{}'", lower[0] as char)
            }
            2 => {
                format!(
                    "u16::from_ne_bytes([{p}[{o0}], {p}[{o1}]]) \
                     | 0x2020u16 == u16::from_ne_bytes([{b0}, {b1}])",
                    p = prefix, o0 = offset, o1 = offset + 1,
                    b0 = lower[0], b1 = lower[1],
                )
            }
            4 => {
                format!(
                    "u32::from_ne_bytes([{p}[{o0}], {p}[{o1}], {p}[{o2}], {p}[{o3}]]) \
                     | 0x20202020u32 == u32::from_ne_bytes([{b0}, {b1}, {b2}, {b3}])",
                    p = prefix, o0 = offset, o1 = offset + 1,
                    o2 = offset + 2, o3 = offset + 3,
                    b0 = lower[0], b1 = lower[1], b2 = lower[2], b3 = lower[3],
                )
            }
            8 => {
                format!(
                    "u64::from_ne_bytes([{p}[{o0}], {p}[{o1}], {p}[{o2}], {p}[{o3}], \
                     {p}[{o4}], {p}[{o5}], {p}[{o6}], {p}[{o7}]]) \
                     | 0x2020202020202020u64 == u64::from_ne_bytes([{b0}, {b1}, {b2}, {b3}, {b4}, {b5}, {b6}, {b7}])",
                    p = prefix, o0 = offset, o1 = offset + 1,
                    o2 = offset + 2, o3 = offset + 3,
                    o4 = offset + 4, o5 = offset + 5,
                    o6 = offset + 6, o7 = offset + 7,
                    b0 = lower[0], b1 = lower[1], b2 = lower[2], b3 = lower[3],
                    b4 = lower[4], b5 = lower[5], b6 = lower[6], b7 = lower[7],
                )
            }
            _ => unreachable!(),
        }
    }).collect();

    parts.join(" && ")
}

/// Human-readable comment: `"inline-block"` → `/* "inline-block" → u64[inline-b] + u32[lock] */`
pub fn chunk_comment(css: &str) -> String {
    let chunks = int_chunks(css.len());
    let parts: Vec<String> = chunks.iter().map(|&(offset, size)| {
        let slice = &css[offset..offset + size];
        let ty = match size {
            1 => "u8",
            2 => "u16",
            4 => "u32",
            8 => "u64",
            _ => unreachable!(),
        };
        format!("{ty}[{slice}]")
    }).collect();
    format!("/* \"{}\" → {} */", css, parts.join(" + "))
}

/// A keyword variant to match: CSS text → Rust expression on match.
pub struct KeywordArm {
    pub css: String,
    pub on_match: String,
}

/// Generates a length-first if-chain match block for keyword enums.
pub fn gen_keyword_match(w: &mut CodeWriter, arms: &[KeywordArm], err_expr: &str) {
    let mut by_len: BTreeMap<usize, Vec<&KeywordArm>> = BTreeMap::new();
    for arm in arms {
        by_len.entry(arm.css.len()).or_default().push(arm);
    }

    // Always use length-dispatch match. Even with one length group, the length
    // guard prevents out-of-bounds panics when used inside try_parse (where any
    // ident can arrive, not just the expected lengths).
    w.match_block("ident.len()", |w| {
        for (len, group) in &by_len {
            let body = format_if_chain_body(group, err_expr);
            w.arm(&len.to_string(), &body);
        }
        w.arm("_", err_expr);
    });
}

/// Generates a length-first if-chain using a custom variable name (e.g. "s" instead of "ident").
pub fn gen_keyword_match_var(w: &mut CodeWriter, var: &str, arms: &[KeywordArm], err_expr: &str) {
    let mut by_len: BTreeMap<usize, Vec<&KeywordArm>> = BTreeMap::new();
    for arm in arms {
        by_len.entry(arm.css.len()).or_default().push(arm);
    }

    if by_len.len() == 1 {
        let (_, group) = by_len.iter().next().unwrap();
        gen_if_chain_var(w, var, group, err_expr);
        return;
    }

    w.match_block(&format!("{var}.len()"), |w| {
        for (len, group) in &by_len {
            let body = format_if_chain_body_var(var, group, err_expr);
            w.arm(&len.to_string(), &body);
        }
        w.arm("_", err_expr);
    });
}

fn gen_if_chain_var(w: &mut CodeWriter, var: &str, group: &[&KeywordArm], err_expr: &str) {
    w.line(&format!("let b = {var}.as_bytes();"));
    if group.len() == 1 {
        let arm = &group[0];
        let cond = gen_int_condition(&arm.css, "b");
        w.line(&chunk_comment(&arm.css));
        w.block(&format!("if {cond}"), |w| { w.line(&arm.on_match); });
        w.block("else", |w| { w.line(err_expr); });
        return;
    }

    for (i, arm) in group.iter().enumerate() {
        let cond = gen_int_condition(&arm.css, "b");
        let prefix = if i == 0 { "if" } else { "else if" };
        w.line(&chunk_comment(&arm.css));
        w.block(&format!("{prefix} {cond}"), |w| { w.line(&arm.on_match); });
    }
    w.block("else", |w| { w.line(err_expr); });
}

fn format_if_chain_body(group: &[&KeywordArm], err_expr: &str) -> String {
    format_if_chain_body_var("ident", group, err_expr)
}

fn format_if_chain_body_var(var: &str, group: &[&KeywordArm], err_expr: &str) -> String {
    let mut s = format!("{{ let b = {var}.as_bytes(); ");
    for (i, arm) in group.iter().enumerate() {
        let cond = gen_int_condition(&arm.css, "b");
        let prefix = if i == 0 { "if" } else { "else if" };
        s.push_str(&format!("{} {prefix} {cond} {{ {} }} ", chunk_comment(&arm.css), arm.on_match));
    }
    s.push_str(&format!("else {{ {err_expr} }} }}"));
    s
}

pub struct MultiWordArm {
    pub word1: String,
    pub word2: String,
    pub on_match: String,
}

pub fn gen_multi_word_match(w: &mut CodeWriter, arms: &[MultiWordArm]) {
    if arms.is_empty() { return; }

    w.block(
        "if let Ok(next) = input.try_parse(|i| i.expect_ident_cloned())",
        |w| {
            for arm in arms {
                let c1 = gen_int_condition(&arm.word1, "b1");
                let c2 = gen_int_condition(&arm.word2, "b2");
                w.block(
                    &format!(
                        "if ident.len() == {} && next.len() == {} && \
                         {{ let b1 = ident.as_bytes(); {c1} }} && \
                         {{ let b2 = next.as_bytes(); {c2} }}",
                        arm.word1.len(), arm.word2.len(),
                    ),
                    |w| { w.line(&arm.on_match); },
                );
            }
        },
    );
}

pub struct BitflagArm {
    pub css: String,
    pub flag_expr: String,
}

pub fn gen_bitflag_match(
    w: &mut CodeWriter,
    type_name: &str,
    arms: &[BitflagArm],
    has_none: bool,
    err_expr: &str,
) {
    if has_none {
        w.block(
            "if input.try_parse(|i| i.expect_ident_matching(\"none\")).is_ok()",
            |w| { w.line(&format!("return Ok({type_name}::EMPTY);")); },
        );
    }

    w.line(&format!("let mut result = {type_name}::EMPTY;"));
    w.line("let mut found = false;");
    w.blank();

    // The entire ident read + match is inside try_parse so that unknown keywords
    // are NOT consumed — they may belong to a different component in a shorthand
    // (e.g., text-decoration: underline wavy red — "wavy" is a style, not a line flag).
    let err_return = "return Err(i.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue))";

    let mut by_len: BTreeMap<usize, Vec<(&str, &str)>> = BTreeMap::new();
    for arm in arms {
        by_len.entry(arm.css.len()).or_default().push((&arm.css, &arm.flag_expr));
    }

    // Build the match body that returns Ok(flag) or Err to rollback try_parse.
    let match_body = if by_len.len() == 1 {
        let (_, group) = by_len.iter().next().unwrap();
        format_bitflag_ok_chain(group, err_return)
    } else {
        let mut s = String::from("match ident.len() { ");
        for (len, group) in &by_len {
            s.push_str(&format!("{len} => {{ {} }}, ", format_bitflag_ok_chain(group, err_return)));
        }
        s.push_str(&format!("_ => {{ {err_return} }} }}"));
        s
    };

    w.line(&format!(
        "while let Ok(flag) = input.try_parse(|i| {{ \
         let ident = i.expect_ident_cloned()?; \
         let b = ident.as_bytes(); \
         {match_body} \
         }}) {{ result = result | flag; found = true; }}"
    ));

    w.blank();
    w.block("if found", |w| { w.line("Ok(result)"); });
    w.block("else", |w| { w.line(err_expr); });
}

/// Returns `Ok(flag_expr)` for use inside `try_parse`.
/// Unknown keywords return Err, causing try_parse to revert the consumed ident.
fn format_bitflag_ok_chain(group: &[(&str, &str)], err_return: &str) -> String {
    let mut s = String::new();
    for (i, (css, flag_expr)) in group.iter().enumerate() {
        let cond = gen_int_condition(css, "b");
        let prefix = if i == 0 { "if" } else { "else if" };
        s.push_str(&format!("{} {prefix} {cond} {{ Ok({flag_expr}) }} ", chunk_comment(css)));
    }
    s.push_str(&format!("else {{ {err_return} }}"));
    s
}
