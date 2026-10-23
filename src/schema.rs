// SPDX-License-Identifier: MIT OR Apache-2.0
//! The schema: what a project's configuration contains. Parsed from `configurant.toml`, strictly
//! (an unknown schema key is an error) and checked against itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::Deserialize;

use crate::value::{Value, ValueType};

/// The only schema format version this release reads.
pub const SCHEMA_VERSION: u32 = 1;

/// The `[project]` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Project {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub summary: String,
    pub schema_version: u32,
    /// Environment variables starting with this that the schema does not declare are errors.
    #[serde(default)]
    pub env_prefix: Option<String>,
    /// The format version of this project's configuration files (a top-level `config_version` in a
    /// file). `[[migration]]` steps carry older files up to it.
    #[serde(default = "first_config_version")]
    pub config_version: u32,
}

fn first_config_version() -> u32 {
    1
}

/// One `[[migration]]`: the step from config version `from` to `to` (always `from + 1`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Migration {
    pub from: u32,
    pub to: u32,
    #[serde(default)]
    pub ops: Vec<Op>,
}

/// One step of a migration. Every operation is reversible by construction, so a migrated file can
/// be carried back; an operation that would lose information is deliberately not offered (`drop` is
/// lossless only because it names the one value it may remove).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum Op {
    /// Move the value at dotted key `from` to `to`.
    Rename { from: String, to: String },
    /// Replace the string value at `key` by `map[value]`. The map must be one-to-one.
    MapValue {
        key: String,
        map: std::collections::BTreeMap<String, String>,
    },
    /// Remove the key `key`, but only while it holds exactly the string `value`; any other value is an
    /// error, so nothing is lost and carrying back writes `key = value` again. A file that never had the
    /// key gains it when carried back.
    Drop { key: String, value: String },
}

/// One `[[value]]` entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ValueDef {
    /// Dotted key, e.g. `storage.backend`.
    pub key: String,
    #[serde(rename = "type")]
    pub ty: ValueType,
    #[serde(default)]
    pub default: Option<toml::Value>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub choices: Vec<String>,
    pub description: String,
    /// Never printed; accepted only from the environment (or, later, a secret-source module).
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub env: Option<String>,
    /// Where else a `secret` may come from: `<source>:<reference>`, e.g. `vault:MYAPP_API_TOKEN`. The
    /// reference names the secret and is not secret; the `configurant-secrets` crate fetches it. The
    /// environment still wins.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub flag: Option<String>,
    /// Changing it needs a restart (shown by `explain` and `help`).
    #[serde(default)]
    pub restart: bool,
    /// The module whose list this value is chosen from.
    #[serde(default)]
    pub provider: Option<String>,
    /// Loading fails if no source (default included) sets it.
    #[serde(default)]
    pub required: bool,
}

/// One `[[table]]`: an array of tables (`[[name]]` in a configuration file) whose rows have the declared
/// fields. A field is a `[[table.field]]`: the keys of a `[[value]]` except that its `key` is a single
/// name and it has no `env`, `flag`, `provider` or `secret` (a row can only come from a file, and a
/// file may not hold a secret). Only a configuration file can set one.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct TableDef {
    /// Dotted key of the array, e.g. `mount`.
    pub key: String,
    pub description: String,
    /// Loading fails if the file does not set it (an empty array counts as set).
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub restart: bool,
    #[serde(default, rename = "field")]
    pub fields: Vec<ValueDef>,
    #[serde(skip)]
    defaults: BTreeMap<String, Value>,
}

impl TableDef {
    /// The declared field called `name`.
    pub fn field(&self, name: &str) -> Option<&ValueDef> {
        self.fields.iter().find(|f| f.key == name)
    }

    /// The default of field `name`, which a row that omits it gets.
    pub fn default_of(&self, name: &str) -> Option<&Value> {
        self.defaults.get(name)
    }
}

/// One `[[map]]`: a table whose keys are arbitrary non-empty strings and whose values all have one
/// scalar type (`job.env` is a map of strings). Only a configuration file can set one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct MapDef {
    /// Dotted key of the table, e.g. `job.env`.
    pub key: String,
    pub value_type: ValueType,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub restart: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    project: Project,
    #[serde(default, rename = "value")]
    values: Vec<ValueDef>,
    #[serde(default, rename = "table")]
    tables: Vec<TableDef>,
    #[serde(default, rename = "map")]
    maps: Vec<MapDef>,
    #[serde(default, rename = "migration")]
    migrations: Vec<Migration>,
}

/// Why a schema was refused.
#[derive(Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaError {
    #[error("schema is not valid: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("schema_version {0} is not supported (this release reads version 1)")]
    UnsupportedVersion(u32),
    #[error("key `{0}` is not a dotted path of letters, digits, `_` and `-`")]
    BadKey(String),
    #[error("key `{0}` is declared twice")]
    DuplicateKey(String),
    #[error("env name `{0}` is used by two values")]
    DuplicateEnv(String),
    #[error("flag `{0}` is used by two values")]
    DuplicateFlag(String),
    #[error("flag `{0}` must look like `--name`")]
    BadFlag(String),
    #[error("`{a}` is both a value and a table prefix of `{b}`")]
    KeyConflict { a: String, b: String },
    #[error("secret `{0}` may not have a default (a default is printed by help)")]
    SecretWithDefault(String),
    #[error("secret `{0}` may not have a command-line flag")]
    SecretWithFlag(String),
    #[error("`{0}` has a `source` but is not a secret")]
    SourceOnNonSecret(String),
    #[error("`source` of `{0}` must look like `<source>:<reference>`")]
    BadSource(String),
    #[error("enum `{0}` needs a non-empty `choices`")]
    EnumNeedsChoices(String),
    #[error("`{0}` has `choices` but is not an enum")]
    ChoicesOnNonEnum(String),
    #[error("`{0}` has min/max but is not an int or float")]
    RangeOnNonNumeric(String),
    #[error("`{0}` has min greater than max")]
    BadRange(String),
    #[error("default of `{key}`: {reason}")]
    BadDefault { key: String, reason: String },
    #[error("`{0}` is reserved for the configuration file's version and cannot be a value key")]
    ReservedKey(String),
    #[error("`{0}` is a table or a map: declare it with `[[table]]` or `[[map]]`, not `[[value]]`")]
    CompositeValue(String),
    #[error(
        "field `{field}` of table `{table}` must be a single name of letters, digits, `_` and `-`"
    )]
    BadFieldName { table: String, field: String },
    #[error("field `{field}` is declared twice in table `{table}`")]
    DuplicateField { table: String, field: String },
    #[error("field `{field}` of table `{table}` has `{option}`, which a row field cannot have")]
    RowFieldOption {
        table: String,
        field: String,
        option: &'static str,
    },
    #[error("map `{0}` has a value_type that is not string, int, float, bool or path")]
    BadMapValueType(String),
    #[error("migrations do not form a chain to config_version {current}: {reason}")]
    MigrationChain { current: u32, reason: String },
    #[error("migration {from} -> {to}: {reason}")]
    BadMigration { from: u32, to: u32, reason: String },
}

/// A parsed, self-consistent schema.
#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub project: Project,
    values: Vec<ValueDef>,
    defaults: BTreeMap<String, Value>,
    tables: Vec<TableDef>,
    maps: Vec<MapDef>,
    migrations: Vec<Migration>,
}

/// A value that broke a constraint of its definition.
pub(crate) enum Violation {
    OutOfRange,
    NotAllowed,
}

impl Schema {
    pub fn parse(text: &str) -> Result<Schema, SchemaError> {
        let mut raw: Raw = toml::from_str(text)?;
        if raw.project.schema_version != SCHEMA_VERSION {
            return Err(SchemaError::UnsupportedVersion(raw.project.schema_version));
        }
        let mut keys = BTreeSet::new();
        let mut envs = BTreeSet::new();
        let mut flags = BTreeSet::new();
        let mut defaults = BTreeMap::new();
        for d in &raw.values {
            check_key(&d.key)?;
            if d.key == "config_version" || d.key.starts_with("config_version.") {
                return Err(SchemaError::ReservedKey(d.key.clone()));
            }
            if !keys.insert(d.key.clone()) {
                return Err(SchemaError::DuplicateKey(d.key.clone()));
            }
            if let Some(e) = &d.env
                && !envs.insert(e.clone())
            {
                return Err(SchemaError::DuplicateEnv(e.clone()));
            }
            if let Some(fl) = &d.flag {
                if !fl.starts_with("--") || fl.len() < 3 || fl.contains('=') || fl.contains(' ') {
                    return Err(SchemaError::BadFlag(fl.clone()));
                }
                if !flags.insert(fl.clone()) {
                    return Err(SchemaError::DuplicateFlag(fl.clone()));
                }
            }
            if matches!(d.ty, ValueType::Table | ValueType::Map) {
                return Err(SchemaError::CompositeValue(d.key.clone()));
            }
            if let Some(v) = check_def(d)? {
                defaults.insert(d.key.clone(), v);
            }
        }
        for t in &mut raw.tables {
            claim_key(&t.key, &mut keys)?;
            let mut seen = BTreeSet::new();
            for f in &t.fields {
                let bad_name = || SchemaError::BadFieldName {
                    table: t.key.clone(),
                    field: f.key.clone(),
                };
                check_key(&f.key).map_err(|_| bad_name())?;
                if f.key.contains('.') {
                    return Err(bad_name());
                }
                if !seen.insert(f.key.clone()) {
                    return Err(SchemaError::DuplicateField {
                        table: t.key.clone(),
                        field: f.key.clone(),
                    });
                }
                let option = if f.env.is_some() {
                    Some("env")
                } else if f.flag.is_some() {
                    Some("flag")
                } else if f.provider.is_some() {
                    Some("provider")
                } else if f.secret {
                    Some("secret")
                } else {
                    None
                };
                if let Some(option) = option {
                    return Err(SchemaError::RowFieldOption {
                        table: t.key.clone(),
                        field: f.key.clone(),
                        option,
                    });
                }
                if matches!(f.ty, ValueType::Table | ValueType::Map) {
                    return Err(SchemaError::CompositeValue(format!("{}.{}", t.key, f.key)));
                }
                if let Some(v) = check_def(f)? {
                    t.defaults.insert(f.key.clone(), v);
                }
            }
        }
        for m in &raw.maps {
            claim_key(&m.key, &mut keys)?;
            if !matches!(
                m.value_type,
                ValueType::String
                    | ValueType::Int
                    | ValueType::Float
                    | ValueType::Bool
                    | ValueType::Path
            ) {
                return Err(SchemaError::BadMapValueType(m.key.clone()));
            }
        }
        check_migrations(raw.project.config_version, &raw.migrations)?;
        for a in &keys {
            if let Some(b) = keys.iter().find(|b| {
                b.len() > a.len() && b.starts_with(a.as_str()) && b.as_bytes()[a.len()] == b'.'
            }) {
                return Err(SchemaError::KeyConflict {
                    a: a.clone(),
                    b: b.clone(),
                });
            }
        }
        Ok(Schema {
            project: raw.project,
            values: raw.values,
            defaults,
            tables: raw.tables,
            maps: raw.maps,
            migrations: raw.migrations,
        })
    }

    /// Every declared array of tables, in schema order.
    pub fn tables(&self) -> &[TableDef] {
        &self.tables
    }

    /// Every declared map, in schema order.
    pub fn maps(&self) -> &[MapDef] {
        &self.maps
    }

    pub fn table(&self, key: &str) -> Option<&TableDef> {
        self.tables.iter().find(|t| t.key == key)
    }

    pub fn map(&self, key: &str) -> Option<&MapDef> {
        self.maps.iter().find(|m| m.key == key)
    }

    /// Every declared key: values, arrays of tables and maps.
    pub fn all_keys(&self) -> impl Iterator<Item = &str> {
        self.values
            .iter()
            .map(|d| d.key.as_str())
            .chain(self.tables.iter().map(|t| t.key.as_str()))
            .chain(self.maps.iter().map(|m| m.key.as_str()))
    }

    /// Every declared value, in schema order.
    pub fn values(&self) -> &[ValueDef] {
        &self.values
    }

    /// The declared migrations, oldest first.
    pub fn migrations(&self) -> &[Migration] {
        &self.migrations
    }

    pub fn get(&self, key: &str) -> Option<&ValueDef> {
        self.values.iter().find(|d| d.key == key)
    }

    pub fn default_of(&self, key: &str) -> Option<&Value> {
        self.defaults.get(key)
    }

    /// `--help` text. A secret shows its environment variable and the word `secret`, never a value.
    pub fn help(&self) -> String {
        let mut out = format!(
            "{} {}\n{}\n\nVALUES\n",
            self.project.name, self.project.version, self.project.summary
        );
        for d in &self.values {
            let flag = d.flag.as_deref().unwrap_or("(no flag)");
            let _ = write!(out, "  {flag} <{}>  {}", d.ty.name(), d.description);
            if let Some(e) = &d.env {
                let _ = write!(out, "  [env: {e}]");
            }
            if d.secret {
                out.push_str("  [secret]");
                if let Some(src) = &d.source {
                    let _ = write!(out, "  [source: {src}]");
                }
            } else if let Some(v) = self.defaults.get(&d.key) {
                let _ = write!(out, "  [default: {}]", v.shown());
            }
            if d.restart {
                out.push_str("  [restart needed]");
            }
            out.push('\n');
        }
        if !self.tables.is_empty() {
            out.push_str("\nTABLES (configuration file only; one `[[name]]` per row)\n");
            for t in &self.tables {
                let _ = write!(out, "  [[{}]]  {}", t.key, t.description);
                if t.required {
                    out.push_str("  [required]");
                }
                if t.restart {
                    out.push_str("  [restart needed]");
                }
                out.push('\n');
                for f in &t.fields {
                    let _ = write!(out, "    {} <{}>  {}", f.key, f.ty.name(), f.description);
                    if f.required {
                        out.push_str("  [required]");
                    }
                    if let Some(v) = t.defaults.get(&f.key) {
                        let _ = write!(out, "  [default: {}]", v.shown());
                    }
                    out.push('\n');
                }
            }
        }
        if !self.maps.is_empty() {
            out.push_str("\nMAPS (configuration file only; any non-empty key)\n");
            for m in &self.maps {
                let _ = write!(
                    out,
                    "  [{}]  <{}>  {}",
                    m.key,
                    m.value_type.name(),
                    m.description
                );
                if m.required {
                    out.push_str("  [required]");
                }
                if m.restart {
                    out.push_str("  [restart needed]");
                }
                out.push('\n');
            }
        }
        out
    }
}

impl ValueDef {
    pub(crate) fn violation(&self, v: &Value) -> Option<Violation> {
        if let Some(n) = v.as_f64()
            && (self.min.is_some_and(|m| n < m) || self.max.is_some_and(|m| n > m))
        {
            return Some(Violation::OutOfRange);
        }
        if self.ty == ValueType::Enum
            && let Value::Enum(s) = v
            && !self.choices.contains(s)
        {
            return Some(Violation::NotAllowed);
        }
        None
    }
}

/// What can be checked about one definition alone: secret rules, enum choices, range, and the default
/// (converted and checked). Shared by `[[value]]` and the fields of a `[[table]]`.
fn check_def(d: &ValueDef) -> Result<Option<Value>, SchemaError> {
    if d.secret && d.default.is_some() {
        return Err(SchemaError::SecretWithDefault(d.key.clone()));
    }
    if d.secret && d.flag.is_some() {
        return Err(SchemaError::SecretWithFlag(d.key.clone()));
    }
    if let Some(src) = &d.source {
        if !d.secret {
            return Err(SchemaError::SourceOnNonSecret(d.key.clone()));
        }
        if !matches!(src.split_once(':'), Some((n, r)) if !n.is_empty() && !r.is_empty()) {
            return Err(SchemaError::BadSource(d.key.clone()));
        }
    }
    match d.ty {
        ValueType::Enum if d.choices.is_empty() => {
            return Err(SchemaError::EnumNeedsChoices(d.key.clone()));
        }
        ValueType::Enum => {}
        _ if !d.choices.is_empty() => {
            return Err(SchemaError::ChoicesOnNonEnum(d.key.clone()));
        }
        _ => {}
    }
    if (d.min.is_some() || d.max.is_some()) && !matches!(d.ty, ValueType::Int | ValueType::Float) {
        return Err(SchemaError::RangeOnNonNumeric(d.key.clone()));
    }
    if let (Some(a), Some(b)) = (d.min, d.max)
        && a > b
    {
        return Err(SchemaError::BadRange(d.key.clone()));
    }
    let Some(raw_default) = &d.default else {
        return Ok(None);
    };
    let bad = |reason: String| SchemaError::BadDefault {
        key: d.key.clone(),
        reason,
    };
    let v = Value::from_toml(d.ty, raw_default)
        .map_err(|found| bad(format!("expected {}, found {found}", d.ty.name())))?;
    match d.violation(&v) {
        Some(Violation::OutOfRange) => return Err(bad("outside min/max".into())),
        Some(Violation::NotAllowed) => return Err(bad("not one of choices".into())),
        None => {}
    }
    Ok(Some(v))
}

/// A table or map key joins the values' namespace: well-formed, not reserved, not repeated.
fn claim_key(key: &str, keys: &mut BTreeSet<String>) -> Result<(), SchemaError> {
    check_key(key)?;
    if key == "config_version" || key.starts_with("config_version.") {
        return Err(SchemaError::ReservedKey(key.to_owned()));
    }
    if !keys.insert(key.to_owned()) {
        return Err(SchemaError::DuplicateKey(key.to_owned()));
    }
    Ok(())
}

/// The migrations must be exactly the steps 1->2, 2->3, ... up to `config_version`, each made of
/// operations that can be undone.
fn check_migrations(current: u32, migrations: &[Migration]) -> Result<(), SchemaError> {
    let chain = |reason: String| SchemaError::MigrationChain { current, reason };
    if current == 0 {
        return Err(chain("config_version starts at 1".into()));
    }
    for m in migrations {
        let bad = |reason: &str| SchemaError::BadMigration {
            from: m.from,
            to: m.to,
            reason: reason.into(),
        };
        if m.from.checked_add(1) != Some(m.to) {
            return Err(bad("a migration goes from one version to the next"));
        }
        for op in &m.ops {
            match op {
                Op::Rename { from, to } => {
                    check_key(from)?;
                    check_key(to)?;
                    if from == to {
                        return Err(bad("a rename must change the key"));
                    }
                }
                Op::Drop { key, .. } => check_key(key)?,
                Op::MapValue { key, map } => {
                    check_key(key)?;
                    if map.is_empty() {
                        return Err(bad("a map_value needs a non-empty map"));
                    }
                    let outputs: BTreeSet<&String> = map.values().collect();
                    if outputs.len() != map.len() {
                        return Err(bad("a map_value must be one-to-one to be carried back"));
                    }
                }
            }
        }
    }
    let expected = usize::try_from(current - 1).unwrap_or(usize::MAX);
    if migrations.len() != expected {
        return Err(chain(format!(
            "{} steps declared, {expected} needed",
            migrations.len()
        )));
    }
    for (i, m) in migrations.iter().enumerate() {
        if usize::try_from(m.from).ok() != Some(i + 1) {
            return Err(chain(format!(
                "step {} should start at version {}",
                i + 1,
                i + 1
            )));
        }
    }
    Ok(())
}

fn check_key(key: &str) -> Result<(), SchemaError> {
    let ok = !key.is_empty()
        && key.split('.').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
    if ok {
        Ok(())
    } else {
        Err(SchemaError::BadKey(key.to_owned()))
    }
}
