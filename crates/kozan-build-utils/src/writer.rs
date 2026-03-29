pub struct CodeWriter {
    buf: String,
    indent: usize,
}

impl CodeWriter {
    pub fn new() -> Self {
        Self {
            buf: String::with_capacity(64 * 1024),
            indent: 0,
        }
    }

    pub fn finish(self) -> String {
        self.buf
    }

    pub fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.buf.push_str("    ");
        }
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    pub fn blank(&mut self) {
        self.buf.push('\n');
    }

    pub fn doc(&mut self, text: &str) {
        self.line(&format!("/// {text}"));
    }

    pub fn doc_link(&mut self, label: &str, url: &str) {
        self.line(&format!("/// [{label}]({url})"));
    }

    pub fn maybe_doc_link(&mut self, label: &str, spec: &Option<String>) {
        if let Some(url) = spec {
            self.doc_link(label, url);
        }
    }

    pub fn derive(&mut self, traits: &[&str]) {
        self.line(&format!("#[derive({})]", traits.join(", ")));
    }

    pub fn repr(&mut self, ty: &str) {
        self.line(&format!("#[repr({ty})]"));
    }

    pub fn block(&mut self, header: &str, f: impl FnOnce(&mut Self)) {
        self.line(&format!("{header} {{"));
        self.indent += 1;
        f(self);
        self.indent -= 1;
        self.line("}");
    }

    pub fn impl_block(&mut self, ty: &str, f: impl FnOnce(&mut Self)) {
        self.block(&format!("impl {ty}"), f);
    }

    pub fn impl_trait(&mut self, tr: &str, ty: &str, f: impl FnOnce(&mut Self)) {
        self.block(&format!("impl {tr} for {ty}"), f);
    }

    pub fn fn_block(&mut self, sig: &str, f: impl FnOnce(&mut Self)) {
        self.block(&format!("pub fn {sig}"), f);
    }

    pub fn const_fn_block(&mut self, sig: &str, f: impl FnOnce(&mut Self)) {
        self.block(&format!("pub const fn {sig}"), f);
    }

    pub fn match_block(&mut self, expr: &str, f: impl FnOnce(&mut Self)) {
        self.block(&format!("match {expr}"), f);
    }

    /// Write an if / else-if / else chain.
    ///
    /// Each entry is `(condition, body_fn)`. The last entry may use `""` as
    /// condition to generate the `else` branch.
    pub fn if_else_chain(&mut self, branches: Vec<(&str, Box<dyn FnOnce(&mut Self) + '_>)>) {
        for (i, (cond, body)) in branches.into_iter().enumerate() {
            let header = if i == 0 {
                format!("if {cond} {{")
            } else if cond.is_empty() {
                "} else {".to_string()
            } else {
                format!("}} else if {cond} {{")
            };
            self.line(&header);
            self.indent += 1;
            body(self);
            self.indent -= 1;
        }
        self.line("}");
    }

    pub fn arm(&mut self, pattern: &str, body: &str) {
        self.line(&format!("{pattern} => {body},"));
    }

    pub fn field(&mut self, name: &str, ty: &str) {
        self.line(&format!("pub {name}: {ty},"));
    }

    pub fn field_init(&mut self, name: &str, value: &str) {
        self.line(&format!("{name}: {value},"));
    }

    pub fn inline_attr(&mut self) {
        self.line("#[inline]");
    }

    pub fn impl_default(&mut self, ty: &str, f: impl FnOnce(&mut Self)) {
        self.impl_trait("Default", ty, |w| {
            w.inline_attr();
            w.block("fn default() -> Self", f);
        });
    }

    pub fn impl_display_via(&mut self, ty: &str, method: &str) {
        self.impl_trait("core::fmt::Display", ty, |w| {
            w.block(
                "fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result",
                |w| { w.line(&format!("f.write_str(self.{method}())")); },
            );
        });
    }

    pub fn impl_from_str_match(&mut self, ty: &str, arms: &[(&str, &str)]) {
        self.impl_trait("core::str::FromStr", ty, |w| {
            w.line("type Err = ();");
            w.blank();
            w.block("fn from_str(s: &str) -> Result<Self, ()>", |w| {
                w.match_block("s", |w| {
                    for (css, variant) in arms {
                        w.arm(&format!("\"{css}\""), &format!("Ok(Self::{variant})"));
                    }
                    w.arm("_", "Err(())");
                });
            });
        });
    }

    pub fn impl_try_from_int(&mut self, ty: &str, repr: &str, variants: &[(usize, &str)]) {
        self.impl_trait(&format!("TryFrom<{repr}>"), ty, |w| {
            w.line("type Error = ();");
            w.blank();
            w.block(&format!("fn try_from(v: {repr}) -> Result<Self, ()>"), |w| {
                w.match_block("v", |w| {
                    for (i, variant) in variants {
                        w.arm(&format!("{i}"), &format!("Ok(Self::{variant})"));
                    }
                    w.arm("_", "Err(())");
                });
            });
        });
    }

    pub fn impl_from_marker(&mut self, marker: &str, ty: &str, variant: &str) {
        self.impl_trait(&format!("From<crate::{marker}>"), ty, |w| {
            w.inline_attr();
            w.block(&format!("fn from(_: crate::{marker}) -> Self"), |w| {
                w.line(&format!("Self::{variant}"));
            });
        });
    }
}
