use kozan_build_utils::{PropertyGroup, CodeWriter};


pub fn generate(groups: &[PropertyGroup]) -> String {
    let mut w = CodeWriter::new();

    gen_property_id(&mut w, groups);
    w.blank();

    for group in groups {
        if !group.properties.is_empty() {
            gen_group_struct(&mut w, group);
            w.blank();
        }
    }

    gen_computed_style(&mut w, groups);
    w.blank();

    gen_computed_style_inherit(&mut w, groups);
    w.blank();

    gen_property_id_css(&mut w, groups);
    w.blank();

    gen_property_id_from_str(&mut w, groups);
    w.blank();

    gen_shorthand_expansion(&mut w, groups);
    w.blank();

    gen_logical_resolution(&mut w, groups);

    w.finish()
}

fn gen_property_id(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.doc("Every CSS property name — physical, logical, and shorthand.");
    w.repr("u16");
    w.derive(&["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash"]);
    w.block("pub enum PropertyId", |w| {
        let mut idx = 0u16;

        for group in groups {
            for prop in &group.properties {
                w.maybe_doc_link("MDN", &prop.spec);
                w.line(&format!("{} = {idx},", to_pascal(&prop.css)));
                idx += 1;
            }
        }

        for group in groups {
            for logical in &group.logicals {
                w.maybe_doc_link("MDN", &logical.spec);
                w.doc(&format!("Logical ({} {})", logical.axis, logical.edge));
                w.line(&format!("{} = {idx},", to_pascal(&logical.css)));
                idx += 1;
            }
        }

        for group in groups {
            for short in &group.shorthands {
                w.maybe_doc_link("MDN", &short.spec);
                w.doc(&format!("Shorthand for: {}", short.longhands.join(", ")));
                w.line(&format!("{} = {idx},", to_pascal(&short.css)));
                idx += 1;
            }
        }

        w.doc("Custom property (`--*`).");
        w.line(&format!("Custom = {idx},"));
    });
}

fn gen_group_struct(w: &mut CodeWriter, group: &PropertyGroup) {
    w.derive(&["Clone", "Debug", "PartialEq"]);
    w.block(&format!("pub struct {}", group.struct_name), |w| {
        for prop in &group.properties {
            if let Some(spec) = &prop.spec {
                w.doc_link("MDN", spec);
            }
            let field_ty = format!("<{} as crate::ToComputedValue>::ComputedValue", prop.ty);
            w.field(&prop.field, &field_ty);
        }
    });
    w.blank();

    w.impl_default(&group.struct_name, |w| {
        w.line("let ctx = crate::ComputeContext::default();");
        w.block("Self", |w| {
            for prop in &group.properties {
                let expr = format!(
                    "<{} as crate::ToComputedValue>::to_computed_value(&{}, &ctx)",
                    prop.ty, prop.initial
                );
                w.field_init(&prop.field, &expr);
            }
        });
    });
}

fn gen_computed_style(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.doc("All computed style properties, organized by group.");
    w.derive(&["Clone", "Debug", "PartialEq"]);
    w.block("pub struct ComputedStyle", |w| {
        for group in groups {
            if !group.properties.is_empty() {
                w.field(&group.name, &group.struct_name);
            }
        }
    });
    w.blank();

    w.impl_default("ComputedStyle", |w| {
        w.block("Self", |w| {
            for group in groups {
                if !group.properties.is_empty() {
                    w.field_init(&group.name, &format!("{}::default()", group.struct_name));
                }
            }
        });
    });
}

fn gen_computed_style_inherit(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.impl_block("ComputedStyle", |w| {
        w.doc("Creates a new style that inherits applicable properties from the parent.");
        w.doc("Inherited values clone from parent (Atom: O(1) via Arc, Box<[T]>: O(n)).");
        w.doc("Non-inherited values are set to their CSS initial values.");
        w.block("pub fn inherit(parent: &ComputedStyle) -> ComputedStyle", |w| {
            w.block("ComputedStyle", |w| {
                for group in groups {
                    if group.properties.is_empty() {
                        continue;
                    }
                    let all_inherited = group.properties.iter().all(|p| p.inherited);
                    let none_inherited = group.properties.iter().all(|p| !p.inherited);

                    if all_inherited {
                        // All inherited: clone group (Atom clone = Arc bump, Box clone = memcpy)
                        w.field_init(&group.name, &format!("parent.{}.clone()", group.name));
                    } else if none_inherited {
                        // All non-inherited: use defaults
                        w.field_init(&group.name, &format!("{}::default()", group.struct_name));
                    } else {
                        // Mixed: clone inherited fields, default the rest
                        w.line(&format!("{}: {{", group.name));
                        w.line(&format!("    let mut g = {}::default();", group.struct_name));
                        for prop in &group.properties {
                            if prop.inherited {
                                w.line(&format!("    g.{f} = parent.{g}.{f}.clone();",
                                    f = prop.field, g = group.name));
                            }
                        }
                        w.line("    g");
                        w.line("},");
                    }
                }
            });
        });
    });
}

fn gen_property_id_css(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.impl_block("PropertyId", |w| {
        w.const_fn_block("as_css(self) -> &'static str", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for prop in &group.properties {
                        w.arm(&format!("Self::{}", to_pascal(&prop.css)), &format!("\"{}\"", prop.css));
                    }
                    for logical in &group.logicals {
                        w.arm(&format!("Self::{}", to_pascal(&logical.css)), &format!("\"{}\"", logical.css));
                    }
                    for short in &group.shorthands {
                        w.arm(&format!("Self::{}", to_pascal(&short.css)), &format!("\"{}\"", short.css));
                    }
                }
                w.arm("Self::Custom", "\"--*\"");
            });
        });
        w.blank();

        w.const_fn_block("is_inherited(self) -> bool", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for prop in &group.properties {
                        if prop.inherited {
                            w.arm(&format!("Self::{}", to_pascal(&prop.css)), "true");
                        }
                    }
                }
                w.arm("_", "false");
            });
        });
        w.blank();

        w.const_fn_block("is_animatable(self) -> bool", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for prop in &group.properties {
                        if prop.animatable {
                            w.arm(&format!("Self::{}", to_pascal(&prop.css)), "true");
                        }
                    }
                }
                w.arm("_", "false");
            });
        });
    });
}

fn gen_property_id_from_str(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    use kozan_build_utils::match_algo::{KeywordArm, gen_keyword_match_var};

    // Collect all property names → PropertyId variants.
    let mut arms: Vec<KeywordArm> = Vec::new();
    for group in groups {
        for prop in &group.properties {
            arms.push(KeywordArm {
                css: prop.css.clone(),
                on_match: format!("Ok(Self::{})", to_pascal(&prop.css)),
            });
        }
        for logical in &group.logicals {
            arms.push(KeywordArm {
                css: logical.css.clone(),
                on_match: format!("Ok(Self::{})", to_pascal(&logical.css)),
            });
        }
        for short in &group.shorthands {
            arms.push(KeywordArm {
                css: short.css.clone(),
                on_match: format!("Ok(Self::{})", to_pascal(&short.css)),
            });
        }
    }

    w.impl_trait("core::str::FromStr", "PropertyId", |w| {
        w.line("type Err = ();");
        w.blank();
        w.block("fn from_str(s: &str) -> Result<Self, ()>", |w| {
            // Integer-chunk matching: length-first dispatch + u32/u64 comparisons.
            // ~272 property names matched in O(1) integer ops instead of string equality.
            gen_keyword_match_var(w, "s", &arms, "Err(())");
        });
    });
}

fn gen_shorthand_expansion(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.impl_block("PropertyId", |w| {
        w.doc("If this is a shorthand, returns the longhands it expands to.");
        w.const_fn_block("longhands(self) -> Option<&'static [PropertyId]>", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for short in &group.shorthands {
                        let ids: Vec<String> = short.longhands.iter()
                            .map(|lh| format!("PropertyId::{}", to_pascal(lh)))
                            .collect();
                        w.arm(
                            &format!("Self::{}", to_pascal(&short.css)),
                            &format!("Some(&[{}])", ids.join(", ")),
                        );
                    }
                }
                w.arm("_", "None");
            });
        });
        w.blank();

        w.doc("Resolves a logical property to its physical equivalent.");
        w.fn_block("resolve_logical(self, horizontal: bool, ltr: bool) -> Self", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for logical in &group.logicals {
                        let name = to_pascal(&logical.css);
                        let h_ltr = to_pascal(&logical.physical.horizontal_ltr);
                        let h_rtl = to_pascal(&logical.physical.horizontal_rtl);
                        let v_ltr = to_pascal(&logical.physical.vertical_ltr);
                        let v_rtl = to_pascal(&logical.physical.vertical_rtl);

                        w.block(&format!("Self::{name} =>"), |w| {
                            w.match_block("(horizontal, ltr)", |w| {
                                w.arm("(true, true)", &format!("Self::{h_ltr}"));
                                w.arm("(true, false)", &format!("Self::{h_rtl}"));
                                w.arm("(false, true)", &format!("Self::{v_ltr}"));
                                w.arm("(false, false)", &format!("Self::{v_rtl}"));
                            });
                        });
                    }
                }
                w.arm("other", "other");
            });
        });
    });
}

fn gen_logical_resolution(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    w.impl_block("PropertyId", |w| {
        w.const_fn_block("is_logical(self) -> bool", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for logical in &group.logicals {
                        w.arm(&format!("Self::{}", to_pascal(&logical.css)), "true");
                    }
                }
                w.arm("_", "false");
            });
        });
        w.blank();

        w.const_fn_block("is_shorthand(self) -> bool", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for short in &group.shorthands {
                        w.arm(&format!("Self::{}", to_pascal(&short.css)), "true");
                    }
                }
                w.arm("_", "false");
            });
        });
    });
}

fn to_pascal(css: &str) -> String {
    css.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
