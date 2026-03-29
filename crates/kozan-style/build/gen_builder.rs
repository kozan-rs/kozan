use kozan_build_utils::{PropertyGroup, CodeWriter};

pub fn generate(groups: &[PropertyGroup]) -> String {
    let mut w = CodeWriter::new();
    let has_lt = groups.iter()
        .flat_map(|g| &g.properties)
        .any(|p| p.ty.contains("'a"));
    let lt = if has_lt { "<'a>" } else { "" };

    gen_importance(&mut w);
    w.blank();
    gen_property_declaration(&mut w, groups, lt);
    w.blank();
    gen_declaration_block(&mut w, lt);
    w.blank();
    gen_style_setter_trait(&mut w, groups, lt);

    w.finish()
}

fn gen_importance(w: &mut CodeWriter) {
    w.repr("u8");
    w.derive(&["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash"]);
    w.block("pub enum Importance", |w| {
        w.line("Normal = 0,");
        w.line("Important = 1,");
    });
}

/// Look up the type of a physical property by its CSS name.
fn find_physical_type<'a>(groups: &'a [PropertyGroup], css_name: &str) -> Option<&'a str> {
    groups.iter()
        .flat_map(|g| &g.properties)
        .find(|p| p.css == css_name)
        .map(|p| p.ty.as_str())
}

fn gen_property_declaration(w: &mut CodeWriter, groups: &[PropertyGroup], lt: &str) {
    w.doc("CSS property declaration — key + typed value in one enum.");
    w.derive(&["Clone", "Debug", "PartialEq"]);
    w.block(&format!("pub enum PropertyDeclaration{lt}"), |w| {
        for group in groups {
            for prop in &group.properties {
                w.maybe_doc_link("MDN", &prop.spec);
                w.line(&format!("{}(Declared<{}>),", to_pascal(&prop.css), prop.ty));
            }
            // Logical properties — same type as their physical counterpart
            for logical in &group.logicals {
                let physical_type = find_physical_type(groups, &logical.physical.horizontal_ltr);
                if let Some(ty) = physical_type {
                    w.maybe_doc_link("MDN", &logical.spec);
                    w.line(&format!("{}(Declared<{}>),", to_pascal(&logical.css), ty));
                }
            }
        }
        w.doc("Custom property (`--name: value`).");
        w.line("Custom { name: kozan_atom::Atom, value: kozan_atom::Atom },");
    });
    w.blank();

    let impl_lt = if lt.is_empty() { String::new() } else { format!("{lt} PropertyDeclaration{lt}") };
    let impl_header = if lt.is_empty() { "PropertyDeclaration".to_string() } else { impl_lt };
        w.impl_block(&impl_header, |w| {
            w.fn_block("id(&self) -> PropertyId", |w| {
                w.match_block("self", |w| {
                    for group in groups {
                        for prop in &group.properties {
                            let p = to_pascal(&prop.css);
                            w.arm(&format!("Self::{p}(_)"), &format!("PropertyId::{p}"));
                        }
                        for logical in &group.logicals {
                            let physical_type = find_physical_type(groups, &logical.physical.horizontal_ltr);
                            if physical_type.is_some() {
                                let p = to_pascal(&logical.css);
                                w.arm(&format!("Self::{p}(_)"), &format!("PropertyId::{p}"));
                            }
                        }
                    }
                    w.arm("Self::Custom { .. }", "PropertyId::Custom");
                });
            });
        });
}

fn gen_declaration_block(w: &mut CodeWriter, lt: &str) {
    w.doc("Ordered list of CSS declarations. Implements `StyleSetter` — IS the builder.");
    w.derive(&["Clone", "Debug"]);
    w.block(&format!("pub struct DeclarationBlock{lt}"), |w| {
        w.line(&format!("entries: Vec<(PropertyDeclaration{lt}, Importance)>,"));
        w.line("importance: Importance,");
    });
    w.blank();

    let impl_header = if lt.is_empty() {
        "DeclarationBlock".to_string()
    } else {
        format!("{lt} DeclarationBlock{lt}")
    };

    w.impl_block(&impl_header, |w| {
        w.fn_block("new() -> Self", |w| {
            w.block("Self", |w| {
                w.field_init("entries", "Vec::new()");
                w.field_init("importance", "Importance::Normal");
            });
        });
        w.blank();

        w.doc("Subsequent declarations are `!important`.");
        w.line("#[inline]");
        w.block("pub fn important(&mut self) -> &mut Self", |w| {
            w.line("self.importance = Importance::Important;");
            w.line("self");
        });
        w.blank();

        w.doc("Subsequent declarations are normal (default).");
        w.line("#[inline]");
        w.block("pub fn normal(&mut self) -> &mut Self", |w| {
            w.line("self.importance = Importance::Normal;");
            w.line("self");
        });
        w.blank();

        w.fn_block(&format!("entries(&self) -> &[(PropertyDeclaration{lt}, Importance)]"), |w| {
            w.line("&self.entries");
        });
        w.blank();

        w.fn_block("len(&self) -> usize", |w| {
            w.line("self.entries.len()");
        });
        w.blank();

        w.fn_block("is_empty(&self) -> bool", |w| {
            w.line("self.entries.is_empty()");
        });
    });
    w.blank();

    let (trait_name, type_name) = if lt.is_empty() {
        ("StyleSetter".to_string(), "DeclarationBlock".to_string())
    } else {
        (format!("StyleSetter{lt}"), format!("DeclarationBlock{lt}"))
    };
    w.block(&format!("impl{lt} {trait_name} for {type_name}"), |w| {
        w.line("#[inline]");
        w.block(&format!("fn on_set(&mut self, decl: PropertyDeclaration{lt})"), |w| {
            w.line("self.entries.push((decl, self.importance));");
        });
    });
}

fn gen_style_setter_trait(w: &mut CodeWriter, groups: &[PropertyGroup], lt: &str) {
    w.doc("Style setter. Implement `on_set` → get all property methods free.");
    w.block(&format!("pub trait StyleSetter{lt}: Sized"), |w| {
        w.line(&format!("fn on_set(&mut self, decl: PropertyDeclaration{lt});"));
        w.blank();

        for group in groups {
            for prop in &group.properties {
                w.maybe_doc_link("MDN", &prop.spec);
                w.line("#[inline]");
                w.block(
                    &format!(
                        "fn {}(&mut self, v: impl IntoDeclared<{}>) -> &mut Self",
                        prop.field, prop.ty,
                    ),
                    |w| {
                        w.line(&format!(
                            "self.on_set(PropertyDeclaration::{}(v.into_declared()));",
                            to_pascal(&prop.css),
                        ));
                        w.line("self");
                    },
                );
                w.blank();
            }
        }

        // Logical properties — same setter pattern as physical
        for group in groups {
            for logical in &group.logicals {
                let physical_type = find_physical_type(groups, &logical.physical.horizontal_ltr);
                if let Some(ty) = physical_type {
                    let method = logical.css.replace('-', "_");
                    w.maybe_doc_link("MDN", &logical.spec);
                    w.line("#[inline]");
                    w.block(
                        &format!(
                            "fn {method}(&mut self, v: impl IntoDeclared<{ty}>) -> &mut Self",
                        ),
                        |w| {
                            w.line(&format!(
                                "self.on_set(PropertyDeclaration::{}(v.into_declared()));",
                                to_pascal(&logical.css),
                            ));
                            w.line("self");
                        },
                    );
                    w.blank();
                }
            }
        }

        // Custom properties: --name: value
        w.doc("Sets a custom property (`--name: value`).");
        w.line("#[inline]");
        w.block(
            "fn property(&mut self, name: impl Into<kozan_atom::Atom>, value: impl Into<kozan_atom::Atom>) -> &mut Self",
            |w| {
                w.line("self.on_set(PropertyDeclaration::Custom {");
                w.line("    name: name.into(),");
                w.line("    value: value.into(),");
                w.line("});");
                w.line("self");
            },
        );
        w.blank();

        // Shorthands (same-type longhands only)
        for group in groups {
            for short in &group.shorthands {
                let method = short.css.replace('-', "_");
                let fields: Vec<String> = short.longhands.iter()
                    .map(|lh| lh.replace('-', "_"))
                    .collect();

                let first_type = groups.iter()
                    .flat_map(|g| &g.properties)
                    .find(|p| p.css == short.longhands[0])
                    .map(|p| p.ty.as_str());

                let Some(prop_type) = first_type else { continue };

                let all_same = short.longhands.iter().all(|lh| {
                    groups.iter()
                        .flat_map(|g| &g.properties)
                        .find(|p| p.css == *lh)
                        .map(|p| p.ty.as_str()) == Some(prop_type)
                });
                if !all_same { continue; }

                let all_exist = fields.iter().all(|f| {
                    groups.iter().flat_map(|g| &g.properties).any(|p| p.field == *f)
                });
                if !all_exist { continue; }

                w.maybe_doc_link("MDN", &short.spec);
                w.line("#[inline]");
                w.block(
                    &format!(
                        "fn {method}(&mut self, v: impl IntoDeclared<{prop_type}> + Clone) -> &mut Self",
                    ),
                    |w| {
                        for (i, field) in fields.iter().enumerate() {
                            if i < fields.len() - 1 {
                                w.line(&format!("self.{field}(v.clone());"));
                            } else {
                                w.line(&format!("self.{field}(v);"));
                            }
                        }
                        w.line("self");
                    },
                );
                w.blank();
            }
        }
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
