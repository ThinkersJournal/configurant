// SPDX-License-Identifier: MIT OR Apache-2.0
//! The schema: what a project's configuration contains. Parsed from `configurant.toml`, strictly
//! (an unknown schema key is an error) and checked against itself.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

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
    pub min: Option<Bound>,
    #[serde(default)]
    pub max: Option<Bound>,
    #[serde(default)]
    pub choices: Vec<String>,
    pub description: String,
    /// The question a wizard asks for this value, in plain language. Without one the description is asked.
    #[serde(default)]
    pub prompt: Option<String>,
    /// A sample value shown next to the question, as it would be typed. It must pass the same checks as a
    /// default, and a secret may not have one.
    #[serde(default)]
    pub example: Option<String>,
    /// One short sentence of help for the person answering (where to find the value, what to avoid).
    #[serde(default)]
    pub hint: Option<String>,
    /// The form section this value is listed under. Without one: the first segment of the key
    /// (`server` for `server.port`), or `General` for a key with no dot.
    #[serde(default)]
    pub section: Option<String>,
    /// A form may fold this value away until asked (rarely changed; the default is usually right).
    #[serde(default)]
    pub advanced: bool,
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
    #[serde(default, rename = "step")]
    steps: Vec<Step>,
}

/// A condition that shows a step only sometimes: exactly one of `equals = "text"`, `not_equals = "text"` or
/// `set = true|false` next to the `key` it reads (the value as shown, staged if any, else current; values
/// are compared as text). Data only: no expressions, no combining several conditions yet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct When {
    pub key: String,
    #[serde(default)]
    pub equals: Option<String>,
    #[serde(default)]
    pub not_equals: Option<String>,
    /// `true`: the key has a value. `false`: it has none.
    #[serde(default)]
    pub set: Option<bool>,
}

/// One `[[step]]` of a wizard: a group of values asked together, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Step {
    pub id: String,
    pub title: String,
    /// Plain sentences shown above the questions.
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub when: Option<When>,
}

impl Step {
    /// Is the step shown? `lookup` gives a key's value as shown, or `None` when it has none. A step with
    /// no `when` is always shown; `equals` on a key with no value is false, `not_equals` is true, and
    /// `set = true` is false.
    pub fn visible(&self, lookup: &dyn Fn(&str) -> Option<String>) -> bool {
        let Some(w) = &self.when else {
            return true;
        };
        let now = lookup(&w.key);
        match (&w.equals, &w.not_equals, w.set) {
            (Some(x), _, _) => now.as_deref() == Some(x.as_str()),
            (_, Some(x), _) => now.as_deref() != Some(x.as_str()),
            (_, _, Some(set)) => now.is_some() == set,
            _ => true,
        }
    }
}

/// A library's options, mounted into an application's schema under a name. See
/// `docs/SCHEMA-FRAGMENTS.md`: the library ships the fragment text, the application mounts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    name: String,
    fragment: String,
}

impl Mount {
    /// `name` becomes the prefix of every mounted key (`webhost` gives `webhost.port`); `fragment` is
    /// the library's schema fragment text.
    pub fn new(name: &str, fragment: &str) -> Self {
        Mount {
            name: name.to_owned(),
            fragment: fragment.to_owned(),
        }
    }
}

/// The `[fragment]` table: what a fragment is, in place of a project.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct FragmentInfo {
    name: String,
    version: String,
    #[serde(default)]
    summary: String,
    schema_version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFragment {
    fragment: FragmentInfo,
    #[serde(default, rename = "value")]
    values: Vec<ValueDef>,
    #[serde(default, rename = "table")]
    tables: Vec<TableDef>,
    #[serde(default, rename = "map")]
    maps: Vec<MapDef>,
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
    #[error("secret `{0}` may not have an example (an example of a secret looks like one)")]
    SecretWithExample(String),
    #[error("example of `{key}`: {reason}")]
    BadExample { key: String, reason: String },
    #[error("`{0}` has an empty `section`")]
    EmptySection(String),
    #[error("`{0}` has an empty `hint`")]
    EmptyHint(String),
    #[error("`{0}` has an empty `prompt`")]
    EmptyPrompt(String),
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
    #[error("step id `{0}` is empty or not letters, digits, `_` and `-`")]
    BadStepId(String),
    #[error("step `{0}` is declared twice")]
    DuplicateStep(String),
    #[error("step `{0}` has an empty `title`")]
    EmptyStepTitle(String),
    #[error("step `{step}` names `{key}`, which is not a value")]
    StepUnknownKey { step: String, key: String },
    #[error("`{0}` is in two steps")]
    StepKeyTwice(String),
    #[error("`when` of step `{step}`: {reason}")]
    BadWhen { step: String, reason: String },
    #[error("mount `{0}` is mounted twice")]
    DuplicateMount(String),
    #[error("fragment mounted as `{mount}` is not valid: {reason}")]
    FragmentParse { mount: String, reason: String },
    #[error(
        "`{key}` is mounted from a fragment; the application may change only its default and wording: {reason}"
    )]
    MountOverride { key: String, reason: String },
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
    steps: Vec<Step>,
}

/// A `min` or `max` in a schema. An integer bound is kept as an integer, so an `int` is compared exactly
/// (an f64 cannot tell neighbours above 2^53 apart); a bound written with a fraction is a float.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(untagged)]
pub enum Bound {
    Int(i64),
    Float(f64),
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bound::Int(i) => write!(f, "{i}"),
            Bound::Float(x) => write!(f, "{x}"),
        }
    }
}

/// `i` against `x`, exactly (`None` for NaN).
fn cmp_int_float(i: i64, x: f64) -> Option<Ordering> {
    if x.is_nan() {
        return None;
    }
    let floor = x.floor();
    // `as i128` saturates, and every i64 lies strictly inside that range.
    let fl = floor as i128;
    Some(match i128::from(i).cmp(&fl) {
        Ordering::Equal if x > floor => Ordering::Less,
        other => other,
    })
}

impl Bound {
    /// How `v` compares with this bound (`Greater`: the value is above it). `None` if `v` is not a number or
    /// either side is NaN.
    pub(crate) fn cmp_value(&self, v: &Value) -> Option<Ordering> {
        match (v, self) {
            (Value::Int(i), Bound::Int(b)) => Some(i.cmp(b)),
            (Value::Int(i), Bound::Float(b)) => cmp_int_float(*i, *b),
            (Value::Float(x), Bound::Int(b)) => cmp_int_float(*b, *x).map(Ordering::reverse),
            (Value::Float(x), Bound::Float(b)) => x.partial_cmp(b),
            _ => None,
        }
    }

    /// How this bound compares with `other`.
    pub(crate) fn cmp_bound(self, other: Bound) -> Option<Ordering> {
        match (self, other) {
            (Bound::Int(a), Bound::Int(b)) => Some(a.cmp(&b)),
            (Bound::Int(a), Bound::Float(b)) => cmp_int_float(a, b),
            (Bound::Float(a), Bound::Int(b)) => cmp_int_float(b, a).map(Ordering::reverse),
            (Bound::Float(a), Bound::Float(b)) => a.partial_cmp(&b),
        }
    }
}

/// A value that broke a constraint of its definition.
pub(crate) enum Violation {
    OutOfRange,
    NotAllowed,
}

impl Schema {
    pub fn parse(text: &str) -> Result<Schema, SchemaError> {
        Schema::parse_with(text, &[])
    }

    /// Parse `text` with each library fragment in `mounts` added under its mount name. The application
    /// may restate a mounted value to change its default and its wording; anything else is refused.
    ///
    /// ```
    /// use configurant::{Loader, Mount, Schema};
    /// // What a library ships (a `&str` in the library, behind a feature): keys are relative.
    /// let fragment = r#"
    /// fragment = { name = "webhost", version = "1", schema_version = 1 }
    /// value = [{ key = "port", type = "int", default = 8080, env = "PORT", description = "listen port" }]
    /// "#;
    /// // What the application writes: its own schema, plus a mount name for the library's options.
    /// let app = r#"
    /// project = { name = "app", version = "1", summary = "demo", schema_version = 1, env_prefix = "APP_" }
    /// "#;
    /// let schema = Schema::parse_with(app, &[Mount::new("webhost", fragment)]).unwrap();
    /// let config = Loader::new(&schema).build().unwrap();
    /// assert_eq!(config.get::<i64>("webhost.port").unwrap(), 8080); // the library's option, no code of ours
    /// // The fragment's `env = "PORT"` becomes <env_prefix><mount>_<env>. A loader reads the variables you give it:
    /// let vars = [("APP_WEBHOST_PORT".to_string(), "9000".to_string())].into();
    /// let config = Loader::new(&schema).env(vars).build().unwrap(); // or `std::env::vars().collect()`
    /// assert_eq!(config.get::<i64>("webhost.port").unwrap(), 9000);
    /// ```
    pub fn parse_with(text: &str, mounts: &[Mount]) -> Result<Schema, SchemaError> {
        let mut raw: Raw = toml::from_str(text)?;
        if raw.project.schema_version != SCHEMA_VERSION {
            return Err(SchemaError::UnsupportedVersion(raw.project.schema_version));
        }
        let mut mounted = BTreeSet::new();
        for m in mounts {
            check_key(&m.name)?;
            if m.name.contains('.') {
                return Err(SchemaError::BadKey(m.name.clone()));
            }
            if !mounted.insert(m.name.clone()) {
                return Err(SchemaError::DuplicateMount(m.name.clone()));
            }
            splice(&mut raw, m)?;
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
        check_steps(&raw.steps, &raw.values)?;
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
            steps: raw.steps,
        })
    }

    /// The steps a wizard walks, "effective" because they are what is asked after the defaults are filled in:
    ///
    /// - with `[[step]]` entries: those, in order, then a final "More settings" step holding every value no
    ///   step lists (an unlisted value is never dropped);
    /// - with none: one step per form section (the value's `section`, else the key's first segment, else
    ///   `General`), in order of first appearance, so a schema with no `[[step]]` still has a wizard.
    ///
    /// ```
    /// use configurant::Schema;
    /// let schema = Schema::parse(r#"
    /// project = { name = "app", version = "1", summary = "demo", schema_version = 1 }
    /// value = [
    ///   { key = "mode", type = "enum", choices = ["server", "client"], default = "server", description = "what to run" },
    ///   { key = "server.port", type = "int", default = 8080, description = "listen port" },
    /// ]
    /// step = [
    ///   { id = "mode", title = "What do you want to run?", keys = ["mode"] },
    ///   { id = "net", title = "Where should it listen?", keys = ["server.port"], when = { key = "mode", equals = "server" } },
    /// ]
    /// "#).unwrap();
    /// let steps = schema.effective_steps();
    /// assert_eq!(steps[1].title, "Where should it listen?");
    /// // A step with a `when` is shown only while the condition holds; you supply how to read a value.
    /// assert!(steps[1].visible(&|key| (key == "mode").then(|| "server".to_string())));
    /// assert!(!steps[1].visible(&|key| (key == "mode").then(|| "client".to_string())));
    /// ```
    pub fn effective_steps(&self) -> Vec<Step> {
        let unique = |taken: &[Step], base: String| {
            let mut id = base.clone();
            let mut n = 2;
            while taken.iter().any(|s| s.id == id) {
                id = format!("{base}-{n}");
                n += 1;
            }
            id
        };
        let mut steps: Vec<Step> = Vec::new();
        if self.steps.is_empty() {
            for d in &self.values {
                let title = d.section_title();
                match steps.iter_mut().find(|s| s.title == title) {
                    Some(s) => s.keys.push(d.key.clone()),
                    None => {
                        let id = unique(&steps, slug(title));
                        steps.push(Step {
                            id,
                            title: title.to_owned(),
                            intro: String::new(),
                            keys: vec![d.key.clone()],
                            when: None,
                        });
                    }
                }
            }
            return steps;
        }
        steps.extend(self.steps.iter().cloned());
        let rest: Vec<String> = self
            .values
            .iter()
            .filter(|d| !self.steps.iter().any(|s| s.keys.contains(&d.key)))
            .map(|d| d.key.clone())
            .collect();
        if !rest.is_empty() {
            let id = unique(&steps, "more".to_owned());
            steps.push(Step {
                id,
                title: "More settings".to_owned(),
                intro: String::new(),
                keys: rest,
                when: None,
            });
        }
        steps
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
            if let Some(x) = &d.example {
                let _ = write!(out, "  [example: {x}]");
            }
            if let Some(h) = &d.hint {
                let _ = write!(out, "  [hint: {h}]");
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
    /// The form section this value is listed under: its `section`, else the key's first segment, else `General`.
    pub fn section_title(&self) -> &str {
        match (&self.section, self.key.split_once('.')) {
            (Some(s), _) => s,
            (None, Some((first, _))) => first,
            (None, None) => "General",
        }
    }

    /// The question to ask for this value: its `prompt`, else its description.
    pub fn question(&self) -> &str {
        self.prompt.as_deref().unwrap_or(&self.description)
    }

    pub(crate) fn violation(&self, v: &Value) -> Option<Violation> {
        if matches!(v, Value::Int(_) | Value::Float(_))
            && (self
                .min
                .is_some_and(|m| m.cmp_value(v) == Some(Ordering::Less))
                || self
                    .max
                    .is_some_and(|m| m.cmp_value(v) == Some(Ordering::Greater)))
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
/// Add a fragment's values, tables and maps to `raw` under the mount name.
fn splice(raw: &mut Raw, m: &Mount) -> Result<(), SchemaError> {
    let frag: RawFragment =
        toml::from_str(&m.fragment).map_err(|e| SchemaError::FragmentParse {
            mount: m.name.clone(),
            reason: e.to_string(),
        })?;
    if frag.fragment.schema_version != SCHEMA_VERSION {
        return Err(SchemaError::UnsupportedVersion(
            frag.fragment.schema_version,
        ));
    }
    let env_prefix = raw.project.env_prefix.clone().unwrap_or_default();
    let name = &m.name;
    for mut v in frag.values {
        v.key = format!("{name}.{}", v.key);
        v.env = v.env.map(|e| {
            format!("{env_prefix}{name}_{e}")
                .replace('-', "_")
                .to_uppercase()
        });
        v.flag = v
            .flag
            .map(|f| format!("--{name}-{}", f.trim_start_matches("--")));
        if let Some(i) = raw.values.iter().position(|a| a.key == v.key) {
            let a = raw.values.remove(i);
            v = override_of(v, a)?;
        }
        raw.values.push(v);
    }
    for mut t in frag.tables {
        t.key = format!("{name}.{}", t.key);
        raw.tables.push(t);
    }
    for mut mp in frag.maps {
        mp.key = format!("{name}.{}", mp.key);
        raw.maps.push(mp);
    }
    Ok(())
}

/// The application restates a mounted value: it may set the default and the wording only.
fn override_of(mut from_fragment: ValueDef, app: ValueDef) -> Result<ValueDef, SchemaError> {
    let refuse = |reason: &str| SchemaError::MountOverride {
        key: app.key.clone(),
        reason: reason.into(),
    };
    if app.ty != from_fragment.ty {
        return Err(refuse("the type differs from the fragment's"));
    }
    if app.min.is_some()
        || app.max.is_some()
        || !app.choices.is_empty()
        || app.secret
        || app.env.is_some()
        || app.source.is_some()
        || app.flag.is_some()
        || app.restart
        || app.provider.is_some()
        || app.required
        || app.section.is_some()
        || app.advanced
    {
        return Err(refuse(
            "it sets something other than default, description, prompt, example or hint",
        ));
    }
    if app.default.is_some() {
        from_fragment.default = app.default;
    }
    from_fragment.description = app.description;
    if app.prompt.is_some() {
        from_fragment.prompt = app.prompt;
    }
    if app.example.is_some() {
        from_fragment.example = app.example;
    }
    if app.hint.is_some() {
        from_fragment.hint = app.hint;
    }
    Ok(from_fragment)
}

/// A step id from a section title: lower case letters and digits, other runs become one `-`.
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "step".to_owned()
    } else {
        out
    }
}

/// Steps name real values, once each; a `when` reads a real value and makes exactly one test.
fn check_steps(steps: &[Step], values: &[ValueDef]) -> Result<(), SchemaError> {
    let known = |k: &str| values.iter().any(|v| v.key == k);
    let mut ids = BTreeSet::new();
    let mut used = BTreeSet::new();
    for s in steps {
        let id_ok = !s.id.is_empty()
            && s.id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !id_ok {
            return Err(SchemaError::BadStepId(s.id.clone()));
        }
        if !ids.insert(s.id.clone()) {
            return Err(SchemaError::DuplicateStep(s.id.clone()));
        }
        if s.title.trim().is_empty() {
            return Err(SchemaError::EmptyStepTitle(s.id.clone()));
        }
        for k in &s.keys {
            if !known(k) {
                return Err(SchemaError::StepUnknownKey {
                    step: s.id.clone(),
                    key: k.clone(),
                });
            }
            if !used.insert(k.clone()) {
                return Err(SchemaError::StepKeyTwice(k.clone()));
            }
        }
        if let Some(w) = &s.when {
            let bad = |reason: &str| SchemaError::BadWhen {
                step: s.id.clone(),
                reason: reason.into(),
            };
            if !known(&w.key) {
                return Err(bad("it reads a key that is not a value"));
            }
            let tests = usize::from(w.equals.is_some())
                + usize::from(w.not_equals.is_some())
                + usize::from(w.set.is_some());
            if tests != 1 {
                return Err(bad("it needs exactly one of equals, not_equals, set"));
            }
        }
    }
    Ok(())
}

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
        && a.cmp_bound(b) == Some(Ordering::Greater)
    {
        return Err(SchemaError::BadRange(d.key.clone()));
    }
    if d.prompt.as_deref().is_some_and(|p| p.trim().is_empty()) {
        return Err(SchemaError::EmptyPrompt(d.key.clone()));
    }
    if d.section.as_deref().is_some_and(|s| s.trim().is_empty()) {
        return Err(SchemaError::EmptySection(d.key.clone()));
    }
    if d.hint.as_deref().is_some_and(|h| h.trim().is_empty()) {
        return Err(SchemaError::EmptyHint(d.key.clone()));
    }
    if let Some(text) = &d.example {
        if d.secret {
            return Err(SchemaError::SecretWithExample(d.key.clone()));
        }
        let bad = |reason: &str| SchemaError::BadExample {
            key: d.key.clone(),
            reason: reason.into(),
        };
        let v = Value::parse_text(d.ty, text)
            .ok_or_else(|| bad(&format!("not a valid {}", d.ty.name())))?;
        match d.violation(&v) {
            Some(Violation::OutOfRange) => return Err(bad("outside min/max")),
            Some(Violation::NotAllowed) => return Err(bad("not one of choices")),
            None => {}
        }
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
