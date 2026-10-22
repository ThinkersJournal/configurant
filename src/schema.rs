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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    project: Project,
    #[serde(default, rename = "value")]
    values: Vec<ValueDef>,
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
}

/// A parsed, self-consistent schema.
#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub project: Project,
    values: Vec<ValueDef>,
    defaults: BTreeMap<String, Value>,
}

/// A value that broke a constraint of its definition.
pub(crate) enum Violation {
    OutOfRange,
    NotAllowed,
}

impl Schema {
    pub fn parse(text: &str) -> Result<Schema, SchemaError> {
        let raw: Raw = toml::from_str(text)?;
        if raw.project.schema_version != SCHEMA_VERSION {
            return Err(SchemaError::UnsupportedVersion(raw.project.schema_version));
        }
        let mut keys = BTreeSet::new();
        let mut envs = BTreeSet::new();
        let mut flags = BTreeSet::new();
        let mut defaults = BTreeMap::new();
        for d in &raw.values {
            check_key(&d.key)?;
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
            if d.secret && d.default.is_some() {
                return Err(SchemaError::SecretWithDefault(d.key.clone()));
            }
            if d.secret && d.flag.is_some() {
                return Err(SchemaError::SecretWithFlag(d.key.clone()));
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
            if (d.min.is_some() || d.max.is_some())
                && !matches!(d.ty, ValueType::Int | ValueType::Float)
            {
                return Err(SchemaError::RangeOnNonNumeric(d.key.clone()));
            }
            if let (Some(a), Some(b)) = (d.min, d.max)
                && a > b
            {
                return Err(SchemaError::BadRange(d.key.clone()));
            }
            if let Some(raw_default) = &d.default {
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
                defaults.insert(d.key.clone(), v);
            }
        }
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
        })
    }

    /// Every declared value, in schema order.
    pub fn values(&self) -> &[ValueDef] {
        &self.values
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
            } else if let Some(v) = self.defaults.get(&d.key) {
                let _ = write!(out, "  [default: {}]", v.shown());
            }
            if d.restart {
                out.push_str("  [restart needed]");
            }
            out.push('\n');
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
