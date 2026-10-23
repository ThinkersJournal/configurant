// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adding one `[[value]]` to a schema's text, as data: the library returns the new text and only a front end
//! writes a file. Comments and layout survive (`toml_edit`), and the result must parse as a schema, so a
//! rejected addition leaves the caller with the text it had.
//!
//! A secret is added with no default and no flag (the schema's own rule), and no error carries a default.

use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use crate::schema::{Schema, SchemaError};
use crate::value::ValueType;

/// A value to add. `flag` is deliberately absent: a scan cannot know a good flag name, and a secret may not
/// have one.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct NewValue {
    pub key: String,
    pub ty: ValueType,
    pub description: String,
    pub env: Option<String>,
    pub default: Option<toml::Value>,
    pub choices: Vec<String>,
    pub secret: bool,
}

impl NewValue {
    pub fn new(key: &str, ty: ValueType, description: &str) -> NewValue {
        NewValue {
            key: key.into(),
            ty,
            description: description.into(),
            env: None,
            default: None,
            choices: Vec::new(),
            secret: false,
        }
    }
}

/// Why a value could not be added. No variant carries a default.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaEditError {
    #[error("schema text: {0}")]
    SchemaParse(String),
    #[error("`{0}` is secret, so it cannot have a default (a default would be printed by `help`)")]
    SecretWithDefault(String),
    #[error("the schema would not accept it: {0}")]
    Invalid(SchemaError),
}

/// The schema text with `new` appended as a `[[value]]`.
pub fn add_value(schema_text: &str, new: &NewValue) -> Result<String, SchemaEditError> {
    Schema::parse(schema_text).map_err(|e| SchemaEditError::SchemaParse(e.to_string()))?;
    if new.secret && new.default.is_some() {
        return Err(SchemaEditError::SecretWithDefault(new.key.clone()));
    }
    let mut doc: DocumentMut = schema_text
        .parse()
        .map_err(|e: toml_edit::TomlError| SchemaEditError::SchemaParse(e.to_string()))?;
    let mut t = Table::new();
    t["key"] = value(new.key.as_str());
    t["type"] = value(new.ty.name());
    if let Some(d) = &new.default {
        t["default"] = Item::Value(to_edit(d));
    }
    if !new.choices.is_empty() {
        let mut a = toml_edit::Array::new();
        for c in &new.choices {
            a.push(c.as_str());
        }
        t["choices"] = value(a);
    }
    t["description"] = value(new.description.as_str());
    if new.secret {
        t["secret"] = value(true);
    }
    if let Some(e) = &new.env {
        t["env"] = value(e.as_str());
    }
    match doc
        .entry("value")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
    {
        Item::ArrayOfTables(a) => a.push(t),
        _ => {
            return Err(SchemaEditError::SchemaParse(
                "`value` is not an array of tables".into(),
            ));
        }
    }
    let out = doc.to_string();
    Schema::parse(&out).map_err(SchemaEditError::Invalid)?;
    Ok(out)
}

fn to_edit(v: &toml::Value) -> toml_edit::Value {
    match v {
        toml::Value::String(s) => s.as_str().into(),
        toml::Value::Integer(i) => (*i).into(),
        toml::Value::Float(f) => (*f).into(),
        toml::Value::Boolean(b) => (*b).into(),
        other => other
            .to_string()
            .parse::<toml_edit::Value>()
            .unwrap_or_else(|_| "".into()),
    }
}
