use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub struct TypesFile {
    #[serde(rename = "enum", default)]
    pub enums: Vec<EnumDef>,
    #[serde(default)]
    pub bitflags: Vec<BitflagsDef>,
    /// List wrapper types: `struct Foo(pub Box<[T]>)` with Default.
    #[serde(default)]
    pub list: Vec<ListDef>,
}

impl EnumDef {
    /// Returns true if this enum has any data-carrying variants.
    pub fn has_data_variants(&self) -> bool {
        self.variants.iter().any(|v| v.ty.is_some())
    }

    /// Keyword-only variants (no data).
    pub fn keyword_variants(&self) -> impl Iterator<Item = &EnumVariant> {
        self.variants.iter().filter(|v| v.ty.is_none())
    }

    /// Data-carrying variants.
    pub fn data_variants(&self) -> impl Iterator<Item = &EnumVariant> {
        self.variants.iter().filter(|v| v.ty.is_some())
    }
}

impl EnumVariant {
    /// Returns true if this variant carries data.
    pub fn is_data(&self) -> bool {
        self.ty.is_some()
    }
}

/// List wrapper definition — generates `struct Name(pub Box<[Inner]>)`.
#[derive(Deserialize)]
pub struct ListDef {
    pub name: String,
    /// Inner type, e.g. `"Duration"`, `"TimingFunction"`.
    pub inner: String,
    /// Default expression for the inner item, e.g. `"Duration::ZERO"`.
    pub default_inner: String,
    pub spec: Option<String>,
}

#[derive(Deserialize)]
pub struct EnumDef {
    pub name: String,
    /// `repr` — required for pure keyword enums, empty for enums with data variants.
    #[serde(default)]
    pub repr: String,
    pub default: String,
    pub spec: Option<String>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Deserialize)]
pub struct EnumVariant {
    pub name: String,
    /// CSS keyword for this variant. Empty for data-only variants (no keyword parsing).
    #[serde(default)]
    pub css: String,
    /// When set, this variant holds data of this type: `Name(Type)`.
    /// When absent, this is a unit keyword variant: `Name`.
    /// Data variants are tried as parse fallbacks after all keyword variants fail.
    #[serde(rename = "type", default)]
    pub ty: Option<String>,
}

#[derive(Deserialize)]
pub struct BitflagsDef {
    pub name: String,
    pub repr: String,
    pub default: Option<Vec<String>>,
    pub spec: Option<String>,
    pub flags: Vec<BitflagEntry>,
}

#[derive(Deserialize)]
pub struct BitflagEntry {
    pub name: String,
    pub css: String,
    pub bit: u32,
}

#[derive(Deserialize)]
pub struct PropertiesFile {
    #[serde(default)]
    pub property: Vec<PropertyDef>,
    #[serde(default)]
    pub shorthand: Vec<ShorthandDef>,
    #[serde(default)]
    pub logical: Vec<LogicalDef>,
}

#[derive(Deserialize)]
pub struct PropertyDef {
    pub css: String,
    pub field: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub initial: String,
    pub inherited: bool,
    #[serde(default)]
    pub animatable: bool,
    pub spec: Option<String>,
}

#[derive(Deserialize)]
pub struct ShorthandDef {
    pub css: String,
    pub longhands: Vec<String>,
    pub spec: Option<String>,
}

#[derive(Deserialize)]
pub struct LogicalDef {
    pub css: String,
    pub axis: String,
    pub edge: String,
    pub physical: LogicalPhysical,
    pub spec: Option<String>,
}

#[derive(Deserialize)]
pub struct LogicalPhysical {
    pub horizontal_ltr: String,
    pub horizontal_rtl: String,
    pub vertical_ltr: String,
    pub vertical_rtl: String,
}

pub struct PropertyGroup {
    pub name: String,
    pub struct_name: String,
    pub properties: Vec<PropertyDef>,
    pub shorthands: Vec<ShorthandDef>,
    pub logicals: Vec<LogicalDef>,
}

pub fn load_types(schema_dir: &Path) -> TypesFile {
    let path = schema_dir.join("types.toml");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    toml::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

pub fn load_property_groups(schema_dir: &Path) -> Vec<PropertyGroup> {
    let props_dir = schema_dir.join("properties");
    let mut groups = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(&props_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", props_dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let stem = path.file_stem().expect("file has stem").to_string_lossy();
        let struct_name = to_pascal_case(&stem) + "Style";

        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let file: PropertiesFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));

        groups.push(PropertyGroup {
            name: stem.to_string(),
            struct_name,
            properties: file.property,
            shorthands: file.shorthand,
            logicals: file.logical,
        });
    }

    groups
}

pub fn collect_enum_auto_none(enums: &[EnumDef]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut has_auto = Vec::new();
    let mut has_none = Vec::new();
    let mut has_normal = Vec::new();

    for e in enums {
        for v in &e.variants {
            match v.name.as_str() {
                "Auto" => has_auto.push(e.name.clone()),
                "None" => has_none.push(e.name.clone()),
                "Normal" => has_normal.push(e.name.clone()),
                _ => {}
            }
        }
    }

    (has_auto, has_none, has_normal)
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
