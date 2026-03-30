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
    w.blank();

    gen_apply_declaration(&mut w, groups);

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
        // all_longhands() — returns all physical longhand PropertyIds except
        // direction and unicode-bidi (for the `all` CSS shorthand).
        w.doc("All physical longhand property IDs, excluding `direction` and `unicode-bidi`.");
        w.doc("Used by the `all` CSS shorthand (CSS Cascading Level 5 §3.2).");
        w.fn_block("all_longhands() -> &'static [PropertyId]", |w| {
            let mut ids = Vec::new();
            for group in groups {
                for prop in &group.properties {
                    // CSS spec: `all` resets everything EXCEPT direction and unicode-bidi
                    if prop.css == "direction" || prop.css == "unicode-bidi" {
                        continue;
                    }
                    ids.push(format!("PropertyId::{}", to_pascal(&prop.css)));
                }
            }
            w.line(&format!("&[{}]", ids.join(", ")));
        });
        w.blank();

        w.doc("If this is a shorthand, returns the longhands it expands to.");
        w.fn_block("longhands(self) -> Option<&'static [PropertyId]>", |w| {
            w.match_block("self", |w| {
                for group in groups {
                    for short in &group.shorthands {
                        // `all` shorthand expands to ALL longhands (except direction/unicode-bidi).
                        // Use the dynamic all_longhands() instead of the TOML placeholder.
                        if short.css == "all" {
                            w.arm("Self::All", "Some(Self::all_longhands())");
                            continue;
                        }
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

/// Qualify ambiguous initial expressions with the property's specified type.
///
/// Handles:
/// - `Default::default()` → `<Type as Default>::default()`
/// - `generics::Foo::Bar` when type is `generics::Foo<X>` → `generics::Foo::<X>::Bar`
fn qualify_initial(initial: &str, ty: &str) -> String {
    if initial == "Default::default()" {
        return format!("<{ty} as Default>::default()");
    }

    // If the type has generic params and the initial doesn't, inject them.
    // e.g. type = "generics::LPOrNormal<specified::LP>", initial = "generics::LPOrNormal::Normal"
    // → "generics::LPOrNormal::<specified::LP>::Normal"
    if let Some(angle_start) = ty.find('<') {
        let type_prefix = &ty[..angle_start]; // "generics::LPOrNormal"
        let type_params = &ty[angle_start..];  // "<specified::LP>"
        if initial.starts_with(type_prefix) && !initial.contains('<') {
            let suffix = &initial[type_prefix.len()..]; // "::Normal"
            return format!("{type_prefix}::{type_params}{suffix}");
        }
    }

    initial.to_string()
}

/// Build a cached lookup: CSS name → (group_name, PropertyDef).
/// Replaces O(n) linear scans with O(1) HashMap lookups (build-time only).
fn build_prop_cache<'a>(groups: &'a [PropertyGroup]) -> std::collections::HashMap<&'a str, (&'a str, &'a kozan_build_utils::PropertyDef)> {
    let mut map = std::collections::HashMap::new();
    for group in groups {
        for prop in &group.properties {
            map.insert(prop.css.as_str(), (group.name.as_str(), prop));
        }
    }
    map
}

fn gen_apply_declaration(w: &mut CodeWriter, groups: &[PropertyGroup]) {
    // Pre-build CSS name → (group, property) cache for O(1) logical property lookups.
    let prop_cache = build_prop_cache(groups);

    // Generate the apply_prop! macro first.
    // W3C CSS Cascading Level 5:
    // - `revert`: Roll back to previous origin's value. When no previous origin
    //   exists (typical: no user stylesheet), behaves like `unset`.
    // - `revert-layer`: Roll back to previous layer's value. When in first layer
    //   or unlayered, behaves like `unset`.
    // - `unset`: `inherit` for inherited properties, `initial` for non-inherited.
    //
    // The macro treats Revert/RevertLayer as Unset. Full multi-origin rollback
    // requires per-property origin tracking in the resolver (future work).
    w.raw(r#"/// Internal macro for applying a Declared value to a computed field.
/// Handles all CSS-wide keywords including `revert` / `revert-layer`.
macro_rules! apply_prop {
    ($d:expr, $target:expr, $parent:expr, $initial:expr, $ctx:expr, inherit) => {
        match $d {
            Declared::Value(v) => $target = crate::ToComputedValue::to_computed_value(v, $ctx),
            Declared::Initial => $target = crate::ToComputedValue::to_computed_value(&$initial, $ctx),
            // Inherited property: inherit, unset, revert, revert-layer all inherit.
            // If no parent exists (root element), reset to initial value.
            Declared::Inherit | Declared::Unset | Declared::Revert | Declared::RevertLayer => {
                match $parent {
                    Some(pv) => $target = pv.clone(),
                    None => $target = crate::ToComputedValue::to_computed_value(&$initial, $ctx),
                }
            },
            Declared::WithVariables(_) => {} // Handled by resolver before apply
        }
    };
    ($d:expr, $target:expr, $parent:expr, $initial:expr, $ctx:expr, reset) => {
        match $d {
            Declared::Value(v) => $target = crate::ToComputedValue::to_computed_value(v, $ctx),
            Declared::Inherit => if let Some(pv) = $parent { $target = pv.clone(); },
            // Non-inherited property: initial, unset, revert, revert-layer all reset.
            Declared::Initial | Declared::Unset | Declared::Revert | Declared::RevertLayer => {
                $target = crate::ToComputedValue::to_computed_value(&$initial, $ctx);
            },
            Declared::WithVariables(_) => {} // Handled by resolver before apply
        }
    };
}
"#);

    w.impl_block("ComputedStyle", |w| {
        w.doc("Apply a single property declaration to this computed style.");
        w.doc("`WithVariables` must be substituted before calling this.");
        w.doc("Logical properties are resolved using the current direction/writing-mode.");
        w.line("#[allow(clippy::match_single_binding)]");
        w.block(
            "pub fn apply_declaration(\n        &mut self,\n        decl: &PropertyDeclaration,\n        parent: Option<&ComputedStyle>,\n        ctx: &crate::ComputeContext,\n    )",
            |w| {
                w.match_block("decl", |w| {
                    // Physical properties — one arm each.
                    for group in groups {
                        for prop in &group.properties {
                            let pascal = to_pascal(&prop.css);
                            let g = &group.name;
                            let f = &prop.field;
                            let initial = qualify_initial(&prop.initial, &prop.ty);
                            let mode = if prop.inherited { "inherit" } else { "reset" };

                            w.arm(
                                &format!("PropertyDeclaration::{pascal}(d)"),
                                &format!(
                                    "apply_prop!(d, self.{g}.{f}, parent.map(|p| &p.{g}.{f}), {initial}, ctx, {mode})"
                                ),
                            );
                        }

                        // Logical properties — resolve to physical based on writing-mode + direction.
                        for logical in &group.logicals {
                            let pascal = to_pascal(&logical.css);

                            // Look up all 4 physical targets.
                            let hl = prop_cache.get(logical.physical.horizontal_ltr.as_str()).copied();
                            let hr = prop_cache.get(logical.physical.horizontal_rtl.as_str()).copied();
                            let vl = prop_cache.get(logical.physical.vertical_ltr.as_str()).copied();
                            let vr = prop_cache.get(logical.physical.vertical_rtl.as_str()).copied();

                            let Some((hl_g, hl_p)) = hl else {
                                panic!("Logical `{}`: physical `{}` not found", logical.css, logical.physical.horizontal_ltr);
                            };
                            let Some((hr_g, hr_p)) = hr else {
                                panic!("Logical `{}`: physical `{}` not found", logical.css, logical.physical.horizontal_rtl);
                            };
                            let Some((vl_g, vl_p)) = vl else {
                                panic!("Logical `{}`: physical `{}` not found", logical.css, logical.physical.vertical_ltr);
                            };
                            let Some((vr_g, vr_p)) = vr else {
                                panic!("Logical `{}`: physical `{}` not found", logical.css, logical.physical.vertical_rtl);
                            };

                            let mode = if hl_p.inherited { "inherit" } else { "reset" };

                            w.block(&format!("PropertyDeclaration::{pascal}(d) =>"), |w| {
                                w.line("let horiz = matches!(self.text.writing_mode, WritingMode::HorizontalTb);");
                                w.line("let ltr = matches!(self.text.direction, Direction::Ltr);");
                                w.match_block("(horiz, ltr)", |w| {
                                    for (pat, pg, pp) in [
                                        ("(true, true)", hl_g, hl_p),
                                        ("(true, false)", hr_g, hr_p),
                                        ("(false, true)", vl_g, vl_p),
                                        ("(false, false)", vr_g, vr_p),
                                    ] {
                                        let ini = qualify_initial(&pp.initial, &pp.ty);
                                        w.arm(
                                            pat,
                                            &format!(
                                                "apply_prop!(d, self.{}.{}, parent.map(|p| &p.{}.{}), {ini}, ctx, {mode})",
                                                pg, pp.field, pg, pp.field
                                            ),
                                        );
                                    }
                                });
                            });
                        }
                    }

                    // Custom properties — handled separately by the resolver.
                    w.arm("PropertyDeclaration::Custom { .. }", "{}");
                });
            },
        );
    });
}

use kozan_build_utils::to_pascal;
