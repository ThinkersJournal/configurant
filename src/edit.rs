// SPDX-License-Identifier: MIT OR Apache-2.0
//! Editing for front ends (TUI, GUI): the part of a front end that is not drawing.
//!
//! A [`Session`] stages changes to a loaded [`Config`]. Every change meets the rules a load does
//! (type, range, choices, provider module), a secret is refused (it comes only from the environment),
//! and nothing happens until the user confirms: [`Session::review`] returns a [`Review`] (what would
//! change, what needs a restart, the modules' [`Plan`] for display), and only [`Review::confirm`]
//! yields the [`Confirmed`] that can produce the new file text and hands over the plan to execute.
//! This crate never writes a file or executes a plan; the front end does, after asking.

use std::collections::BTreeMap;
use std::fmt;

use toml_edit::{Array, DocumentMut, Item, Table, TableLike, Value as EditValue};

use crate::config::{Config, check_value};
use crate::error::{ConfigError, Origin, suggestion};
use crate::module::{Choice, Modules, Needs, Plan};
use crate::value::{Value, ValueType};

const REDACTED: &str = "<redacted>";

/// One value as a front end lists it. A secret's value is `<redacted>` and it is not editable.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Field {
    pub key: String,
    pub ty: ValueType,
    pub description: String,
    pub secret: bool,
    /// The `<source>:<reference>` a secret declares: a name, never the secret.
    pub source: Option<String>,
    pub restart: bool,
    pub editable: bool,
    /// The schema's `choices` (empty unless it is an enum). A provider's list is [`Session::choices`].
    pub choices: Vec<String>,
    /// The module whose list this value is chosen from.
    pub provider: Option<String>,
    /// The value now in force, as shown.
    pub current: Option<String>,
    /// The staged value, as shown.
    pub staged: Option<String>,
}

/// Staged changes to a loaded configuration.
pub struct Session<'a> {
    config: &'a Config,
    modules: Option<&'a Modules>,
    staged: BTreeMap<String, Value>,
}

impl fmt::Debug for Session<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("config", self.config)
            .field("staged", &self.staged)
            .finish()
    }
}

impl<'a> Session<'a> {
    pub fn new(config: &'a Config, modules: Option<&'a Modules>) -> Self {
        Session {
            config,
            modules,
            staged: BTreeMap::new(),
        }
    }

    /// Every value of the schema, in schema order.
    pub fn fields(&self) -> Vec<Field> {
        self.config
            .schema()
            .values()
            .iter()
            .map(|d| Field {
                key: d.key.clone(),
                ty: d.ty,
                description: d.description.clone(),
                secret: d.secret,
                source: d.source.clone(),
                restart: d.restart,
                editable: !d.secret,
                choices: d.choices.clone(),
                provider: d.provider.clone(),
                current: self.config.value(&d.key).map(|v| shown(d.secret, v)),
                staged: self.staged.get(&d.key).map(Value::shown),
            })
            .collect()
    }

    /// The choices for a key: its provider module's list for `needs`, else the schema's own.
    pub fn choices(&self, key: &str, needs: &Needs) -> Vec<Choice> {
        let Some(def) = self.config.schema().get(key) else {
            return Vec::new();
        };
        if let Some(m) = def
            .provider
            .as_deref()
            .and_then(|p| self.modules.and_then(|m| m.get(p)))
        {
            return m.list_choices(needs);
        }
        def.choices.iter().map(|c| Choice::new(c, "")).collect()
    }

    /// Stage `text` as the new value of `key`. Refused, with a named error and nothing staged, if the
    /// key is unknown or secret or the value does not fit. Staging the value already in force is not a
    /// change and un-stages the key.
    pub fn stage(&mut self, key: &str, text: &str) -> Result<(), ConfigError> {
        let schema = self.config.schema();
        let Some(def) = schema.get(key) else {
            if schema.table(key).is_some() || schema.map(key).is_some() {
                return Err(ConfigError::FileOnly {
                    key: key.to_owned(),
                });
            }
            let known = schema.all_keys();
            return Err(ConfigError::UnknownKey {
                key: key.to_owned(),
                suggestion: suggestion(key, known),
                origin: Origin::Edit,
            });
        };
        if def.secret {
            return Err(ConfigError::SecretInForbiddenSource {
                key: key.to_owned(),
                origin: Origin::Edit,
            });
        }
        let value = Value::parse_text(def.ty, text).ok_or_else(|| ConfigError::WrongType {
            key: key.to_owned(),
            expected: def.ty.name(),
            found: format!("{text:?}"),
            origin: Origin::Edit,
        })?;
        check_value(def, &value, self.modules, &Origin::Edit)?;
        if self.config.value(key) == Some(&value) {
            self.staged.remove(key);
        } else {
            self.staged.insert(key.to_owned(), value);
        }
        Ok(())
    }

    /// Forget a staged change.
    pub fn unstage(&mut self, key: &str) {
        self.staged.remove(key);
    }

    /// What confirming would do. Changes nothing.
    pub fn review(&self) -> Review {
        let schema = self.config.schema();
        let mut changes = Vec::new();
        let mut restart = Vec::new();
        let mut plan = Plan::default();
        let mut values = Vec::new();
        for def in schema.values() {
            let Some(new) = self.staged.get(&def.key) else {
                continue;
            };
            let old = self.config.value(&def.key);
            changes.push(Change {
                key: def.key.clone(),
                from: old.map(Value::shown),
                to: new.shown(),
            });
            if def.restart {
                restart.push(def.key.clone());
            }
            if let (Some(old), Some(m)) = (
                old,
                def.provider
                    .as_deref()
                    .and_then(|p| self.modules.and_then(|m| m.get(p))),
            ) {
                plan.actions.extend(m.on_change(old, new).actions);
            }
            values.push((def.key.clone(), new.clone()));
        }
        Review {
            changes,
            restart,
            plan,
            values,
        }
    }
}

fn shown(secret: bool, v: &Value) -> String {
    if secret {
        REDACTED.to_owned()
    } else {
        v.shown()
    }
}

/// One staged change, as shown (staged values are never secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Change {
    pub key: String,
    pub from: Option<String>,
    pub to: String,
}

/// What a confirmation would do: shown to the user, who then confirms or not.
#[derive(Debug, Clone)]
pub struct Review {
    changes: Vec<Change>,
    restart: Vec<String>,
    plan: Plan,
    values: Vec<(String, Value)>,
}

impl Review {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// Keys among the changes whose schema entry says changing them needs a restart.
    pub fn restart_needed(&self) -> &[String] {
        &self.restart
    }

    /// The modules' plan, for display. It is executed only from [`Confirmed::plan`].
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// The user said yes.
    pub fn confirm(self) -> Confirmed {
        Confirmed {
            changes: self.changes,
            plan: self.plan,
            values: self.values,
        }
    }
}

/// Changes the user has confirmed. The only source of new file text and of a plan to execute.
#[derive(Debug, Clone)]
pub struct Confirmed {
    changes: Vec<Change>,
    plan: Plan,
    values: Vec<(String, Value)>,
}

/// Why new file text could not be made.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EditError {
    #[error("the file is not valid TOML: {0}")]
    Parse(String),
    #[error("`{0}` cannot be set: part of its path is not a table in the file")]
    NotATable(String),
}

impl Confirmed {
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// The plan to execute (the front end does, with both endpoints in hand).
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// `existing` (a configuration file's text; empty is fine) with the changes applied. Comments and
    /// layout survive; the caller writes the result (keeping a backup is the caller's job).
    pub fn file_text(&self, existing: &str) -> Result<String, EditError> {
        let mut doc: DocumentMut = existing
            .parse()
            .map_err(|e: toml_edit::TomlError| EditError::Parse(e.message().to_owned()))?;
        for (key, value) in &self.values {
            set(doc.as_table_mut(), key, value)?;
        }
        Ok(doc.to_string())
    }
}

fn edit_value(v: &Value) -> EditValue {
    match v {
        Value::String(s) | Value::Enum(s) => EditValue::from(s.as_str()),
        Value::Path(p) => EditValue::from(p.to_string_lossy().as_ref()),
        Value::Int(i) => EditValue::from(*i),
        Value::Float(f) => EditValue::from(*f),
        Value::Bool(b) => EditValue::from(*b),
        Value::List(l) => EditValue::Array(l.iter().map(String::as_str).collect::<Array>()),
        // Never staged (`Session::stage` refuses these keys); kept total rather than a panic.
        Value::Rows(_) | Value::Map(_) => v
            .to_toml()
            .to_string()
            .parse::<EditValue>()
            .unwrap_or_else(|_| EditValue::from("")),
    }
}

fn set(root: &mut Table, key: &str, value: &Value) -> Result<(), EditError> {
    let mut parts: Vec<&str> = key.split('.').collect();
    let leaf = parts.pop().unwrap_or_default();
    let mut table: &mut dyn TableLike = root;
    for part in parts {
        let item = table.entry(part).or_insert_with(|| {
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        });
        table = item
            .as_table_like_mut()
            .ok_or_else(|| EditError::NotATable(key.to_owned()))?;
    }
    let mut new = edit_value(value);
    if let Some(old) = table.get(leaf).and_then(Item::as_value) {
        *new.decor_mut() = old.decor().clone();
    }
    table.insert(leaf, Item::Value(new));
    Ok(())
}
