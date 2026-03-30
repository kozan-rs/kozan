use kozan_build_utils::{TypesFile, PropertyGroup, EnumDef, BitflagsDef, ListDef, CodeWriter};
use crate::match_algo::{KeywordArm, MultiWordArm, BitflagArm};

pub fn generate(types: &TypesFile, groups: &[PropertyGroup]) -> String {
    let mut w = CodeWriter::new();

    w.blank();

    for e in &types.enums {
        gen_enum_parse(&mut w, e);
        w.blank();
    }

    for b in &types.bitflags {
        gen_bitflags_parse(&mut w, b);
        w.blank();
    }

    for l in &types.list {
        gen_list_parse(&mut w, l);
        w.blank();
    }

    gen_property_dispatch(&mut w, groups);
    w.blank();
    gen_shorthand_dispatch(&mut w, groups);
    w.blank();
    gen_keyword_declaration(&mut w, groups);
    w.blank();
    gen_unparsed_declaration(&mut w, groups);

    w.finish()
}

/// Generates `Parse` impl for a keyword enum (or keyword+data enum).
///
/// Uses length-first byte pattern dispatch — see `match_algo.rs` for algorithm docs.
/// For enums with data variants, tries keywords first, then data fallbacks in order.
fn gen_enum_parse(w: &mut CodeWriter, e: &EnumDef) {
    let name = &e.name;

    if e.has_data_variants() {
        gen_data_enum_parse(w, e);
        return;
    }

    let has_multi_word = e.variants.iter().any(|v| v.css.contains(' '));

    w.block(
        &format!("impl crate::Parse for {name}"),
        |w| {
            w.block(
                "fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>>",
                |w| {
                    w.line("let location = input.current_source_location();");
                    let err = "Err(location.new_custom_error(crate::CustomError::InvalidValue))";

                    if has_multi_word {
                        gen_enum_parse_multi_word(w, e, err);
                    } else {
                        gen_enum_parse_simple(w, e, err);
                    }
                },
            );
        },
    );
}

/// Enum with keyword + data variants.
///
/// Generated parse order: try ident keywords first (via try_parse so we don't
/// consume tokens on failure), then try each data variant's type as a fallback.
fn gen_data_enum_parse(w: &mut CodeWriter, e: &EnumDef) {
    let name = &e.name;
    let kw_variants: Vec<_> = e.keyword_variants().collect();
    let data_variants: Vec<_> = e.data_variants().collect();

    w.block(
        &format!("impl crate::Parse for {name}"),
        |w| {
            w.block(
                "fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>>",
                |w| {
                    // Try keywords via try_parse (doesn't consume on failure).
                    if !kw_variants.is_empty() {
                        w.block(
                            "if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned())",
                            |w| {
                                let arms: Vec<KeywordArm> = kw_variants.iter().map(|v| {
                                    KeywordArm {
                                        css: v.css.clone(),
                                        on_match: format!("return Ok({name}::{})", v.name),
                                    }
                                }).collect();

                                // Unknown keywords fall through to data variant parsing.
                                let err = "{}";
                                crate::match_algo::gen_keyword_match(w, &arms, err);
                            },
                        );
                    }

                    // Try each data variant as a fallback, in declaration order.
                    for (i, v) in data_variants.iter().enumerate() {
                        let ty = v.ty.as_ref().unwrap();
                        let is_last = i == data_variants.len() - 1;
                        if is_last {
                            // Last fallback — propagate the error directly.
                            w.line(&format!(
                                "<{ty} as crate::Parse>::parse(input).map({name}::{})",
                                v.name
                            ));
                        } else {
                            // Not last — wrap in try_parse.
                            w.block(
                                &format!(
                                    "if let Ok(v) = input.try_parse(<{ty} as crate::Parse>::parse)"
                                ),
                                |w| {
                                    w.line(&format!("return Ok({name}::{}(v));", v.name));
                                },
                            );
                        }
                    }
                },
            );
        },
    );
}

/// Simple enum: all single-word variants.
///
/// Generated code uses zero-alloc byte pattern matching:
/// `expect_ident()` → borrowed, `as_bytes()` → pointer cast, pattern → integer ops.
fn gen_enum_parse_simple(w: &mut CodeWriter, e: &EnumDef, err: &str) {
    w.line("let ident = input.expect_ident()?;");

    let arms: Vec<KeywordArm> = e.variants.iter().map(|v| {
        KeywordArm {
            css: v.css.clone(),
            on_match: format!("Ok({}::{})", e.name, v.name),
        }
    }).collect();

    crate::match_algo::gen_keyword_match(w, &arms, err);
}

/// Enum with multi-word variants like "inline block", "alternate over".
///
/// Tries multi-word matches first (greedy via `try_parse`), falls back to single-word.
/// Both paths use byte pattern matching — zero alloc.
fn gen_enum_parse_multi_word(w: &mut CodeWriter, e: &EnumDef, err: &str) {
    w.line("let ident = input.expect_ident_cloned()?;");

    // Multi-word arms.
    let multi_arms: Vec<MultiWordArm> = e.variants.iter()
        .filter(|v| v.css.contains(' '))
        .map(|v| {
            let words: Vec<&str> = v.css.split(' ').collect();
            MultiWordArm {
                word1: words[0].to_string(),
                word2: words[1].to_string(),
                on_match: format!("return Ok({}::{})", e.name, v.name),
            }
        })
        .collect();

    crate::match_algo::gen_multi_word_match(w, &multi_arms);

    // Single-word fallback.
    let single_arms: Vec<KeywordArm> = e.variants.iter()
        .filter(|v| !v.css.contains(' '))
        .map(|v| {
            KeywordArm {
                css: v.css.clone(),
                on_match: format!("Ok({}::{})", e.name, v.name),
            }
        })
        .collect();

    crate::match_algo::gen_keyword_match(w, &single_arms, err);
}

/// Generates `Parse` impl for a bitflags type.
///
/// Uses byte pattern matching in a loop — case-insensitive, zero-alloc per iteration.
fn gen_bitflags_parse(w: &mut CodeWriter, b: &BitflagsDef) {
    let name = &b.name;

    w.block(
        &format!("impl crate::Parse for {name}"),
        |w| {
            w.block(
                "fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>>",
                |w| {
                    w.line("let location = input.current_source_location();");
                    let err = "Err(location.new_custom_error(crate::CustomError::InvalidValue))";

                    let has_none = b.flags.iter().any(|f| f.css == "none");

                    let arms: Vec<BitflagArm> = b.flags.iter()
                        .filter(|f| f.css != "none")
                        .map(|f| BitflagArm {
                            css: f.css.clone(),
                            flag_expr: format!("{name}::{}", f.name),
                        })
                        .collect();

                    crate::match_algo::gen_bitflag_match(w, name, &arms, has_none, err);
                },
            );
        },
    );
}

/// Generates `Parse` impl for a list wrapper — comma-separated values.
fn gen_list_parse(w: &mut CodeWriter, l: &ListDef) {
    let name = &l.name;
    let inner = &l.inner;

    w.block(
        &format!("impl crate::Parse for {name}"),
        |w| {
            w.block(
                "fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>>",
                |w| {
                    w.line(&format!(
                        "let first = <{inner} as crate::Parse>::parse(input)?;"
                    ));
                    w.line("let mut list = vec![first];");
                    w.block("while input.try_parse(|i| i.expect_comma()).is_ok()", |w| {
                        w.line(&format!(
                            "list.push(<{inner} as crate::Parse>::parse(input)?);"
                        ));
                    });
                    w.line("Ok(Self(list.into_boxed_slice()))");
                },
            );
        },
    );
}

/// Property dispatch — `match id { PropertyId::X => parse... }`.
///
/// Already O(1): PropertyId is `#[repr(u16)]`, Rust compiles to a jump table.
/// No optimization needed here — the match on integer enum is the fastest possible.
/// Look up the type of a physical property by its CSS name.
fn find_physical_type<'a>(groups: &'a [PropertyGroup], css_name: &str) -> Option<&'a str> {
    groups.iter()
        .flat_map(|g| &g.properties)
        .find(|p| p.css == css_name)
        .map(|p| p.ty.as_str())
}

/// Look up the type of any longhand — either a direct `[[property]]` or a
/// `[[logical]]` (resolved via its physical counterpart).
fn find_longhand_type<'a>(groups: &'a [PropertyGroup], css_name: &str) -> Option<&'a str> {
    if let Some(ty) = find_physical_type(groups, css_name) {
        return Some(ty);
    }
    groups.iter()
        .flat_map(|g| &g.logicals)
        .find(|l| l.css == css_name)
        .and_then(|l| find_physical_type(groups, &l.physical.horizontal_ltr))
}

fn gen_property_dispatch(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.block(
        "pub(crate) fn parse_property_value<'i>(\
         id: PropertyId, input: &mut Parser<'i, '_>\
         ) -> Result<PropertyDeclaration, Error<'i>>",
        |w| {
            w.match_block("id", |w| {
                for group in groups {
                    for prop in &group.properties {
                        let pascal = to_pascal(&prop.css);
                        let ty = &prop.ty;
                        w.arm(
                            &format!("PropertyId::{pascal}"),
                            &format!(
                                "<{ty} as crate::Parse>::parse(input)\
                                 .map(|v| PropertyDeclaration::{pascal}(Declared::Value(v)))"
                            ),
                        );
                    }
                    // Logical properties — same parser as physical counterpart
                    for logical in &group.logicals {
                        if let Some(ty) = find_physical_type(groups, &logical.physical.horizontal_ltr) {
                            let pascal = to_pascal(&logical.css);
                            w.arm(
                                &format!("PropertyId::{pascal}"),
                                &format!(
                                    "<{ty} as crate::Parse>::parse(input)\
                                     .map(|v| PropertyDeclaration::{pascal}(Declared::Value(v)))"
                                ),
                            );
                        }
                    }
                }
                w.arm("PropertyId::Custom", "unreachable!(\"custom properties parsed separately\")");
                w.arm("_", "Err(input.new_custom_error(crate::CustomError::UnknownProperty))");
            });
        },
    );
}

/// Generates `parse_shorthand_value` — auto-dispatches same-type shorthands.
///
/// For shorthands where ALL longhands share the same type (e.g. margin, padding,
/// gap, overflow), generates calls to `crate::shorthand::box4` (4-value) or
/// `crate::shorthand::pair2` (2-value). Mixed-type shorthands (border-top,
/// flex-flow, etc.) return `None` and are handled by hand-written code.
fn gen_shorthand_dispatch(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.block(
        "pub(crate) fn parse_shorthand_value<'i>(\
         id: PropertyId, input: &mut Parser<'i, '_>\
         ) -> Option<Result<smallvec::SmallVec<[PropertyDeclaration; 4]>, Error<'i>>>",
        |w| {
            w.match_block("id", |w| {
                for group in groups {
                    for short in &group.shorthands {
                        let types: Vec<Option<&str>> = short.longhands.iter()
                            .map(|lh| find_longhand_type(groups, lh))
                            .collect();

                        // All longhands must have a known type, and all must be the same.
                        let all_same = types.iter().all(|t| t.is_some())
                            && types.windows(2).all(|pair| pair[0] == pair[1]);

                        if !all_same { continue; }

                        let ty = types[0].unwrap();
                        let pascal = to_pascal(&short.css);
                        let lh_ids: Vec<String> = short.longhands.iter()
                            .map(|lh| format!("PropertyDeclaration::{}", to_pascal(lh)))
                            .collect();

                        match short.longhands.len() {
                            2 => {
                                w.arm(
                                    &format!("PropertyId::{pascal}"),
                                    &format!(
                                        "Some(crate::shorthand::pair2::<{ty}>(input, {}, {}))",
                                        lh_ids[0], lh_ids[1],
                                    ),
                                );
                            }
                            4 => {
                                w.arm(
                                    &format!("PropertyId::{pascal}"),
                                    &format!(
                                        "Some(crate::shorthand::box4::<{ty}>(input, {}, {}, {}, {}))",
                                        lh_ids[0], lh_ids[1], lh_ids[2], lh_ids[3],
                                    ),
                                );
                            }
                            3 => {
                                w.arm(
                                    &format!("PropertyId::{pascal}"),
                                    &format!(
                                        "Some(crate::shorthand::triple3::<{ty}>(input, {}, {}, {}))",
                                        lh_ids[0], lh_ids[1], lh_ids[2],
                                    ),
                                );
                            }
                            _ => {
                                // 5+ same-type longhands — not handled generically yet.
                            }
                        }
                    }
                }
                w.arm("_", "None");
            });
        },
    );
}

/// Generates `make_keyword_declaration` — wraps a CSS-wide keyword for any PropertyId.
fn gen_keyword_declaration(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.block(
        "pub(crate) fn make_keyword_declaration(\
         id: PropertyId, keyword: &crate::declaration::CssWideKeyword\
         ) -> Option<PropertyDeclaration>",
        |w| {
            w.line("use crate::declaration::CssWideKeyword;");
            w.blank();
            w.line("macro_rules! kw {");
            w.line("    ($variant:path) => {");
            w.line("        Some(match keyword {");
            w.line("            CssWideKeyword::Inherit => $variant(Declared::Inherit),");
            w.line("            CssWideKeyword::Initial => $variant(Declared::Initial),");
            w.line("            CssWideKeyword::Unset => $variant(Declared::Unset),");
            w.line("            CssWideKeyword::Revert => $variant(Declared::Revert),");
            w.line("            CssWideKeyword::RevertLayer => $variant(Declared::RevertLayer),");
            w.line("        })");
            w.line("    };");
            w.line("}");
            w.blank();
            w.match_block("id", |w| {
                for group in groups {
                    for prop in &group.properties {
                        let pascal = to_pascal(&prop.css);
                        w.arm(
                            &format!("PropertyId::{pascal}"),
                            &format!("kw!(PropertyDeclaration::{pascal})"),
                        );
                    }
                    for logical in &group.logicals {
                        if find_physical_type(groups, &logical.physical.horizontal_ltr).is_some() {
                            let pascal = to_pascal(&logical.css);
                            w.arm(
                                &format!("PropertyId::{pascal}"),
                                &format!("kw!(PropertyDeclaration::{pascal})"),
                            );
                        }
                    }
                }
                w.arm("_", "None");
            });
        },
    );
}

/// Generates `make_unparsed_declaration` — wraps an UnparsedValue for any PropertyId.
fn gen_unparsed_declaration(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.block(
        "pub(crate) fn make_unparsed_declaration(\
         id: PropertyId, unparsed: kozan_style::UnparsedValue\
         ) -> Option<PropertyDeclaration>",
        |w| {
            w.match_block("id", |w| {
                for group in groups {
                    for prop in &group.properties {
                        let pascal = to_pascal(&prop.css);
                        w.arm(
                            &format!("PropertyId::{pascal}"),
                            &format!(
                                "Some(PropertyDeclaration::{pascal}(Declared::WithVariables(unparsed)))"
                            ),
                        );
                    }
                    for logical in &group.logicals {
                        if find_physical_type(groups, &logical.physical.horizontal_ltr).is_some() {
                            let pascal = to_pascal(&logical.css);
                            w.arm(
                                &format!("PropertyId::{pascal}"),
                                &format!(
                                    "Some(PropertyDeclaration::{pascal}(Declared::WithVariables(unparsed)))"
                                ),
                            );
                        }
                    }
                }
                w.arm("_", "None");
            });
        },
    );
}

use kozan_build_utils::to_pascal;
