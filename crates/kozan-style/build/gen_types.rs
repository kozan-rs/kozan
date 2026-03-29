use kozan_build_utils::{BitflagsDef, EnumDef, TypesFile, collect_enum_auto_none, CodeWriter};

pub fn generate(types: &TypesFile) -> String {
    let mut w = CodeWriter::new();

    for e in &types.enums {
        if e.has_data_variants() {
            gen_data_enum(&mut w, e);
        } else {
            gen_enum(&mut w, e);
        }
        w.blank();
    }

    for b in &types.bitflags {
        gen_bitflags(&mut w, b);
        w.blank();
    }

    for l in &types.list {
        gen_list_wrapper(&mut w, l);
        w.blank();
    }

    let (has_auto, has_none, has_normal) = collect_enum_auto_none(&types.enums);
    gen_marker_from_impls(&mut w, &has_auto, &has_none, &has_normal);
    gen_into_declared_impls(&mut w, types);
    gen_trait_impls(&mut w, types);

    w.finish()
}

fn gen_enum(w: &mut CodeWriter, e: &EnumDef) {
    w.maybe_doc_link("MDN", &e.spec);
    w.repr(&e.repr);
    w.derive(&["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash"]);
    w.block(&format!("pub enum {}", e.name), |w| {
        for (i, v) in e.variants.iter().enumerate() {
            w.line(&format!("{} = {i},", v.name));
        }
    });
    w.blank();

    let css_variant_pairs: Vec<(&str, &str)> = e.variants.iter()
        .map(|v| (v.css.as_str(), v.name.as_str()))
        .collect();
    let idx_variant_pairs: Vec<(usize, &str)> = e.variants.iter()
        .enumerate()
        .map(|(i, v)| (i, v.name.as_str()))
        .collect();

    w.impl_default(&e.name, |w| {
        w.line(&format!("Self::{}", e.default));
    });
    w.blank();

    w.impl_block(&e.name, |w| {
        w.const_fn_block("as_css(self) -> &'static str", |w| {
            w.match_block("self", |w| {
                for v in &e.variants {
                    w.arm(&format!("Self::{}", v.name), &format!("\"{}\"", v.css));
                }
            });
        });
    });
    w.blank();

    w.impl_display_via(&e.name, "as_css");
    w.blank();

    w.impl_from_str_match(&e.name, &css_variant_pairs);
    w.blank();

    w.impl_try_from_int(&e.name, &e.repr, &idx_variant_pairs);
    w.blank();
}

/// Generates an enum with mixed keyword + data variants.
///
/// Unlike pure keyword enums, these can't be `Copy` or `#[repr(u8)]`
/// because data variants carry heap-allocated types.
fn gen_data_enum(w: &mut CodeWriter, e: &EnumDef) {
    w.maybe_doc_link("MDN", &e.spec);
    w.derive(&["Clone", "Debug", "PartialEq"]);
    w.block(&format!("pub enum {}", e.name), |w| {
        for v in &e.variants {
            if let Some(ty) = &v.ty {
                w.line(&format!("{}({}),", v.name, qualify_type(ty)));
            } else {
                w.line(&format!("{},", v.name));
            }
        }
    });
    w.blank();

    w.impl_default(&e.name, |w| {
        w.line(&format!("Self::{}", e.default));
    });
    w.blank();

    // ToComputedValue — delegate to inner for data variants, identity for keywords.
    w.impl_trait("crate::ToComputedValue", &e.name, |w| {
        w.line("type ComputedValue = Self;");
        w.inline_attr();
        w.block("fn to_computed_value(&self, _ctx: &crate::ComputeContext) -> Self", |w| {
            w.line("self.clone()");
        });
        w.inline_attr();
        w.block("fn from_computed_value(computed: &Self) -> Self", |w| {
            w.line("computed.clone()");
        });
    });
    w.blank();
}

/// Generates a list wrapper type: `struct Name(pub Box<[Inner]>)`.
fn gen_list_wrapper(w: &mut CodeWriter, l: &kozan_build_utils::ListDef) {
    let inner = qualify_type(&l.inner);
    let default_inner = {
        // Qualify any type references in the default expression too.
        // Simple heuristic: if default_inner contains "::" it's already qualified.
        if l.default_inner.contains("std::") {
            l.default_inner.clone()
        } else {
            l.default_inner.clone()
        }
    };

    w.maybe_doc_link("MDN", &l.spec);
    w.derive(&["Clone", "Debug", "PartialEq"]);
    w.line(&format!("pub struct {}(pub Box<[{}]>);", l.name, inner));
    w.blank();

    w.impl_default(&l.name, |w| {
        w.line(&format!("Self(Box::from([{default_inner}]))"));
    });
    w.blank();

    // ToComputedValue — identity.
    w.impl_trait("crate::ToComputedValue", &l.name, |w| {
        w.line("type ComputedValue = Self;");
        w.inline_attr();
        w.block("fn to_computed_value(&self, _ctx: &crate::ComputeContext) -> Self", |w| {
            w.line("self.clone()");
        });
        w.inline_attr();
        w.block("fn from_computed_value(computed: &Self) -> Self", |w| {
            w.line("computed.clone()");
        });
    });
    w.blank();
}

fn gen_bitflags(w: &mut CodeWriter, b: &BitflagsDef) {
    w.maybe_doc_link("MDN", &b.spec);
    w.repr("transparent");
    w.derive(&["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash"]);
    w.line(&format!("pub struct {}(pub {});", b.name, b.repr));
    w.blank();

    w.impl_block(&b.name, |w| {
        w.line("pub const EMPTY: Self = Self(0);");
        w.blank();
        for flag in &b.flags {
            w.line(&format!("pub const {}: Self = Self(1 << {});", flag.name, flag.bit));
        }
        w.blank();

        w.const_fn_block("contains(self, other: Self) -> bool", |w| {
            w.line("self.0 & other.0 == other.0");
        });
        w.blank();

        w.fn_block("insert(&mut self, other: Self)", |w| {
            w.line("self.0 |= other.0;");
        });
        w.blank();

        w.fn_block("remove(&mut self, other: Self)", |w| {
            w.line("self.0 &= !other.0;");
        });
        w.blank();

        w.const_fn_block("is_empty(self) -> bool", |w| {
            w.line("self.0 == 0");
        });
        w.blank();

        w.const_fn_block(&format!("bits(self) -> {}", b.repr), |w| {
            w.line("self.0");
        });
    });
    w.blank();

    w.impl_default(&b.name, |w| {
        match &b.default {
            Some(flags) if !flags.is_empty() => {
                let expr = flags.iter()
                    .map(|f| format!("Self::{f}.0"))
                    .collect::<Vec<_>>()
                    .join(" | ");
                w.line(&format!("Self({expr})"));
            }
            _ => w.line("Self::EMPTY"),
        }
    });
    w.blank();

    for (tr, op, expr) in [
        ("core::ops::BitOr", "bitor", "Self(self.0 | rhs.0)"),
        ("core::ops::BitAnd", "bitand", "Self(self.0 & rhs.0)"),
    ] {
        w.impl_trait(tr, &b.name, |w| {
            w.line("type Output = Self;");
            w.inline_attr();
            w.block(&format!("fn {op}(self, rhs: Self) -> Self"), |w| {
                w.line(expr);
            });
        });
        w.blank();
    }

    w.impl_trait("core::ops::BitOrAssign", &b.name, |w| {
        w.inline_attr();
        w.block("fn bitor_assign(&mut self, rhs: Self)", |w| {
            w.line("self.0 |= rhs.0;");
        });
    });
    w.blank();

    w.impl_trait("core::ops::BitAndAssign", &b.name, |w| {
        w.inline_attr();
        w.block("fn bitand_assign(&mut self, rhs: Self)", |w| {
            w.line("self.0 &= rhs.0;");
        });
    });
    w.blank();

    w.impl_trait("core::ops::Not", &b.name, |w| {
        w.line("type Output = Self;");
        w.inline_attr();
        w.block("fn not(self) -> Self", |w| {
            w.line("Self(!self.0)");
        });
    });
    w.blank();

    w.impl_trait("core::fmt::Display", &b.name, |w| {
        w.block("fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result", |w| {
            w.line("let mut first = true;");
            for flag in &b.flags {
                w.block(&format!("if self.contains(Self::{})", flag.name), |w| {
                    w.block("if !first", |w| {
                        w.line("f.write_str(\" \")?;");
                    });
                    w.line(&format!("f.write_str(\"{}\")?;", flag.css));
                    w.line("first = false;");
                });
            }
            w.block("if first", |w| {
                w.line("f.write_str(\"none\")?;");
            });
            w.line("Ok(())");
        });
    });
    w.blank();

    let css_flag_pairs: Vec<(&str, &str)> = b.flags.iter()
        .map(|f| (f.css.as_str(), f.name.as_str()))
        .collect();

    w.impl_trait("core::str::FromStr", &b.name, |w| {
        w.line("type Err = ();");
        w.blank();
        w.block("fn from_str(s: &str) -> Result<Self, ()>", |w| {
            w.line("let mut result = Self::EMPTY;");
            w.block("for part in s.split_whitespace()", |w| {
                w.match_block("part", |w| {
                    for (css, flag) in &css_flag_pairs {
                        w.arm(&format!("\"{css}\""), &format!("result.insert(Self::{flag})"));
                    }
                    w.arm("_", "return Err(())");
                });
            });
            w.line("Ok(result)");
        });
    });
}

fn gen_trait_impls(w: &mut CodeWriter, types: &TypesFile) {
    for e in &types.enums {
        let n = &e.name;
        let has_data = e.has_data_variants();

        // ToComputedValue — already generated inline for data enums.
        if !has_data {
            w.impl_trait("crate::ToComputedValue", n, |w| {
                w.line("type ComputedValue = Self;");
                w.inline_attr();
                w.block("fn to_computed_value(&self, _ctx: &crate::ComputeContext) -> Self", |w| {
                    w.line("*self");
                });
                w.inline_attr();
                w.block("fn from_computed_value(computed: &Self) -> Self", |w| {
                    w.line("*computed");
                });
            });
            w.blank();
        }

        // Animate — discrete: swap at 50%. Use clone for data enums, copy for keyword.
        let copy_expr = if has_data { "self.clone()" } else { "*self" };
        let other_expr = if has_data { "other.clone()" } else { "*other" };
        w.impl_trait("crate::Animate", n, |w| {
            w.inline_attr();
            w.block("fn animate(&self, other: &Self, procedure: crate::Procedure) -> Result<Self, ()>", |w| {
                w.block("match procedure", |w| {
                    w.block("crate::Procedure::Interpolate { progress } =>", |w| {
                        w.line(&format!("Ok(if progress < 0.5 {{ {copy_expr} }} else {{ {other_expr} }})"));
                    });
                    w.line("_ => Err(()),");
                });
            });
        });
        w.blank();

        // ToAnimatedZero — no meaningful zero for enums
        w.impl_trait("crate::ToAnimatedZero", n, |w| {
            w.inline_attr();
            w.block("fn to_animated_zero(&self) -> Result<Self, ()>", |w| {
                w.line("Err(())");
            });
        });
        w.blank();

        // ComputeSquaredDistance — 0 if equal, 1 if different
        w.impl_trait("crate::ComputeSquaredDistance", n, |w| {
            w.inline_attr();
            w.block("fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()>", |w| {
                w.line("Ok(if self == other { 0.0 } else { 1.0 })");
            });
        });
        w.blank();
    }

    // Same for bitflags
    for b in &types.bitflags {
        let n = &b.name;

        w.impl_trait("crate::ToComputedValue", n, |w| {
            w.line("type ComputedValue = Self;");
            w.inline_attr();
            w.block("fn to_computed_value(&self, _ctx: &crate::ComputeContext) -> Self", |w| {
                w.line("*self");
            });
            w.inline_attr();
            w.block("fn from_computed_value(computed: &Self) -> Self", |w| {
                w.line("*computed");
            });
        });
        w.blank();

        w.impl_trait("crate::Animate", n, |w| {
            w.inline_attr();
            w.block("fn animate(&self, other: &Self, procedure: crate::Procedure) -> Result<Self, ()>", |w| {
                w.block("match procedure", |w| {
                    w.block("crate::Procedure::Interpolate { progress } =>", |w| {
                        w.line("Ok(if progress < 0.5 { *self } else { *other })");
                    });
                    w.line("_ => Err(()),");
                });
            });
        });
        w.blank();

        w.impl_trait("crate::ToAnimatedZero", n, |w| {
            w.inline_attr();
            w.block("fn to_animated_zero(&self) -> Result<Self, ()>", |w| {
                w.line("Err(())");
            });
        });
        w.blank();
    }
}

fn gen_into_declared_impls(w: &mut CodeWriter, types: &TypesFile) {
    for e in &types.enums {
        w.block(&format!("impl<'a> crate::IntoDeclared<{}> for {}", e.name, e.name), |w| {
            w.inline_attr();
            w.block(&format!("fn into_declared(self) -> crate::Declared<{}>", e.name), |w| {
                w.line("crate::Declared::Value(self)");
            });
        });
        w.blank();
    }
    for b in &types.bitflags {
        w.block(&format!("impl<'a> crate::IntoDeclared<{}> for {}", b.name, b.name), |w| {
            w.inline_attr();
            w.block(&format!("fn into_declared(self) -> crate::Declared<{}>", b.name), |w| {
                w.line("crate::Declared::Value(self)");
            });
        });
        w.blank();
    }
}

/// Qualify a type from TOML for use in generated code.
/// Primitives (`f32`, `i32`, `bool`, `u32`, etc.) stay as-is.
/// Crate types get `crate::` prefix.
fn qualify_type(ty: &str) -> String {
    let is_primitive = matches!(
        ty,
        "f32" | "f64" | "i8" | "i16" | "i32" | "i64"
            | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" | "bool"
    ) || ty.starts_with("std::");

    if is_primitive {
        ty.to_string()
    } else {
        format!("crate::{ty}")
    }
}

fn gen_marker_from_impls(
    w: &mut CodeWriter,
    has_auto: &[String],
    has_none: &[String],
    has_normal: &[String],
) {
    for ty in has_auto {
        w.impl_from_marker("Auto", ty, "Auto");
        w.blank();
    }
    for ty in has_none {
        w.impl_from_marker("CssNone", ty, "None");
        w.blank();
    }
    for ty in has_normal {
        w.impl_from_marker("Normal", ty, "Normal");
        w.blank();
    }
}
