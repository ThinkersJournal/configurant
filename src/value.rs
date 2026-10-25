// SPDX-License-Identifier: MIT OR Apache-2.0
//! Value types and conversion from text (environment, command line) and from TOML (files,
//! defaults).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

/// The type a schema gives a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ValueType {
    String,
    Int,
    Float,
    Bool,
    Path,
    Enum,
    List,
    /// An array of tables (`[[name]]`); declared with `[[table]]`, never as a `[[value]]`.
    Table,
    /// A table with arbitrary keys; declared with `[[map]]`, never as a `[[value]]`.
    Map,
}

impl ValueType {
    pub fn name(self) -> &'static str {
        match self {
            ValueType::String => "string",
            ValueType::Int => "int",
            ValueType::Float => "float",
            ValueType::Bool => "bool",
            ValueType::Path => "path",
            ValueType::Enum => "enum",
            ValueType::List => "list",
            ValueType::Table => "table",
            ValueType::Map => "map",
        }
    }
}

/// One row of an array of tables: field name to value.
pub type Row = BTreeMap<String, Value>;

/// A typed configuration value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Path(PathBuf),
    Enum(String),
    /// A list of strings; from text it is comma-separated (`\,` is a literal comma, `\\` a backslash).
    List(Vec<String>),
    /// The rows of an array of tables. Only a configuration file can set it.
    Rows(Vec<Row>),
    /// A map with arbitrary string keys. Only a configuration file can set it.
    Map(BTreeMap<String, Value>),
}

/// Split list text on commas. `\,` is a literal comma and `\\` a literal backslash; any other backslash is kept
/// as written, so a Windows path in a list is unchanged. Items are trimmed.
fn split_list(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if matches!(chars.peek(), Some(',' | '\\')) => cur.extend(chars.next()),
            ',' => items.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    items.push(cur);
    items.into_iter().map(|s| s.trim().to_owned()).collect()
}

impl Value {
    pub fn type_of(&self) -> ValueType {
        match self {
            Value::String(_) => ValueType::String,
            Value::Int(_) => ValueType::Int,
            Value::Float(_) => ValueType::Float,
            Value::Bool(_) => ValueType::Bool,
            Value::Path(_) => ValueType::Path,
            Value::Enum(_) => ValueType::Enum,
            Value::List(_) => ValueType::List,
            Value::Rows(_) => ValueType::Table,
            Value::Map(_) => ValueType::Map,
        }
    }

    /// Parse text (environment variable, command-line argument) as `ty`; `None` if it does not parse: the
    /// caller names the key and decides whether the text may be shown.
    pub fn parse_text(ty: ValueType, text: &str) -> Option<Value> {
        Some(match ty {
            ValueType::String => Value::String(text.to_owned()),
            ValueType::Enum => Value::Enum(text.to_owned()),
            ValueType::Path => Value::Path(PathBuf::from(text)),
            ValueType::Int => Value::Int(text.trim().parse().ok()?),
            ValueType::Float => Value::Float(text.trim().parse().ok()?),
            ValueType::Bool => match text.trim() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                _ => return None,
            },
            ValueType::Table | ValueType::Map => return None,
            ValueType::List => Value::List(if text.trim().is_empty() {
                Vec::new()
            } else {
                split_list(text)
            }),
        })
    }

    /// Convert a TOML value to `ty`. The error is the TOML type name found.
    pub fn from_toml(ty: ValueType, v: &toml::Value) -> Result<Value, &'static str> {
        match (ty, v) {
            (ValueType::String, toml::Value::String(s)) => Ok(Value::String(s.clone())),
            (ValueType::Enum, toml::Value::String(s)) => Ok(Value::Enum(s.clone())),
            (ValueType::Path, toml::Value::String(s)) => Ok(Value::Path(PathBuf::from(s))),
            (ValueType::Int, toml::Value::Integer(i)) => Ok(Value::Int(*i)),
            (ValueType::Float, toml::Value::Float(f)) => Ok(Value::Float(*f)),
            (ValueType::Float, toml::Value::Integer(i)) => Ok(Value::Float(*i as f64)),
            (ValueType::Bool, toml::Value::Boolean(b)) => Ok(Value::Bool(*b)),
            (ValueType::List, toml::Value::Array(items)) => items
                .iter()
                .map(|i| match i {
                    toml::Value::String(s) => Ok(s.clone()),
                    other => Err(other.type_str()),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List),
            (_, other) => Err(other.type_str()),
        }
    }

    /// The value as it is shown to a human (strings quoted). Callers redact secrets before this.
    pub fn shown(&self) -> String {
        match self {
            Value::String(s) | Value::Enum(s) => format!("{s:?}"),
            Value::Path(p) => format!("{:?}", p.display().to_string()),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::List(l) => format!("{l:?}"),
            Value::Rows(rows) => {
                let rows: Vec<String> = rows.iter().map(shown_map).collect();
                format!("[{}]", rows.join(", "))
            }
            Value::Map(m) => shown_map(m),
        }
    }

    pub(crate) fn to_toml(&self) -> toml::Value {
        match self {
            Value::String(s) | Value::Enum(s) => toml::Value::String(s.clone()),
            Value::Path(p) => toml::Value::String(p.to_string_lossy().into_owned()),
            Value::Int(i) => toml::Value::Integer(*i),
            Value::Float(f) => toml::Value::Float(*f),
            Value::Bool(b) => toml::Value::Boolean(*b),
            Value::List(l) => {
                toml::Value::Array(l.iter().cloned().map(toml::Value::String).collect())
            }
            Value::Rows(rows) => toml::Value::Array(
                rows.iter()
                    .map(|r| toml::Value::Table(to_toml_table(r)))
                    .collect(),
            ),
            Value::Map(m) => toml::Value::Table(to_toml_table(m)),
        }
    }

    /// Does `text` contain this value, raw or as `Debug` would quote it? Empty values match nothing.
    pub(crate) fn appears_in(&self, text: &str) -> bool {
        let mut forms: Vec<String> = Vec::new();
        match self {
            Value::Rows(rows) => return rows.iter().flatten().any(|(_, v)| v.appears_in(text)),
            Value::Map(m) => return m.values().any(|v| v.appears_in(text)),
            Value::String(s) | Value::Enum(s) => forms.push(s.clone()),
            Value::Path(p) => forms.push(p.to_string_lossy().into_owned()),
            Value::List(l) => forms.extend(l.iter().cloned()),
            Value::Int(i) => forms.push(i.to_string()),
            Value::Float(f) => forms.push(f.to_string()),
            Value::Bool(b) => forms.push(b.to_string()),
        }
        let escaped: Vec<String> = forms
            .iter()
            .map(|f| {
                let d = format!("{f:?}");
                d[1..d.len() - 1].to_owned()
            })
            .collect();
        forms
            .iter()
            .chain(&escaped)
            .any(|f| !f.is_empty() && text.contains(f.as_str()))
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
}

fn shown_map(m: &BTreeMap<String, Value>) -> String {
    let items: Vec<String> = m
        .iter()
        .map(|(k, v)| format!("{k}: {}", v.shown()))
        .collect();
    format!("{{{}}}", items.join(", "))
}

fn to_toml_table(m: &BTreeMap<String, Value>) -> toml::Table {
    m.iter().map(|(k, v)| (k.clone(), v.to_toml())).collect()
}
