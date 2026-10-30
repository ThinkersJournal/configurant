// SPDX-License-Identifier: MIT OR Apache-2.0
//! The module interface: how other tools plug into configuration without the core depending on
//! them (a persistence module, a secrets store, ports, paths, logging).
//!
//! A module contributes choice lists, validation and change plans for the values whose schema
//! entry names it with `provider = "<module>"`. It is optional: with the module absent, layering
//! is unchanged and only a *non-default* value for one of its keys is refused (naming the module).
//!
//! **A [`Plan`] is data.** `on_change` returns what *should* happen when a value changes (for
//! example copying data from the old storage backend to the new one); the module never acts on its
//! own. The front end (CLI, TUI, GUI) shows the plan and executes it only after the user confirms.

use std::collections::BTreeMap;

use crate::schema::ValueDef;
use crate::value::Value;

/// Version of this interface: 2 added [`Module::validate_all`]. A module built against a version from
/// [`MIN_INTERFACE_VERSION`] up to this one is accepted (a version 1 module has no `validate_all` and never
/// needed one); any other is refused at registration.
pub const INTERFACE_VERSION: u32 = 2;

/// The oldest interface version still accepted.
pub const MIN_INTERFACE_VERSION: u32 = 1;

/// What the project needs from the choices (for example capabilities and an order of
/// importance); a module defines the meaning of the strings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Needs {
    pub required: Vec<String>,
    pub order: Vec<String>,
}

/// One selectable value with the reason it is (or is not) a good fit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub reason: String,
}

impl Choice {
    pub fn new(id: &str, reason: &str) -> Self {
        Choice {
            id: id.to_owned(),
            reason: reason.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActionKind {
    /// Information only.
    Note,
    /// Copy data between two ids of the module's choice list.
    DataTransfer { from: String, to: String },
    /// A step an [`crate::exec::ExecPlan`] can run after confirmation (directory, file, command, service).
    Step(Box<crate::exec::Step>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub description: String,
    pub kind: ActionKind,
}

/// What should happen after a value changes. Executed by the front end, with confirmation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub actions: Vec<Action>,
}

/// The plug-in interface.
pub trait Module {
    /// The name a schema's `provider` refers to.
    fn name(&self) -> &str;

    /// The interface version this module was written against. The default is 1 (`MIN_INTERFACE_VERSION`): a
    /// module that is recompiled against a newer library must not start claiming a version it was not written
    /// for. A module that overrides [`Module::validate_all`] returns 2 explicitly.
    fn interface_version(&self) -> u32 {
        MIN_INTERFACE_VERSION
    }

    /// The choices for `needs`, best first, each with a reason.
    fn list_choices(&self, needs: &Needs) -> Vec<Choice>;

    /// Is `value` acceptable? The error text may be shown unless the value is a secret.
    fn validate(&self, value: &Value) -> Result<(), String>;

    /// Like [`Module::validate`], for the schema key `def` that names this module: a module whose rule depends
    /// on the key's declaration (the `ports` module reads `privileged`) overrides this. The default ignores
    /// the key and calls `validate`, so a module written before this method keeps working unchanged.
    fn validate_for(&self, def: &ValueDef, value: &Value) -> Result<(), String> {
        let _ = def;
        self.validate(value)
    }

    /// Check the values of every key that names this module together (interface version 2), for a rule that
    /// compares keys, such as two ports that collide. Called once per load with the winning value of each such
    /// key, sorted by key. Returns `(key, message)` for each key at fault; the loader attaches the origin and
    /// redacts the message for a `secret` key. The default returns nothing.
    fn validate_all(&self, values: &[(&str, &Value)]) -> Vec<(String, String)> {
        let _ = values;
        Vec::new()
    }

    /// What should happen when the value changes from `old` to `new`. Data only.
    fn on_change(&self, old: &Value, new: &Value) -> Plan {
        let _ = (old, new);
        Plan::default()
    }
}

/// Why a module could not be registered.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ModuleError {
    #[error("module `{module}` speaks interface version {got}; this library speaks {want}")]
    InterfaceMismatch { module: String, got: u32, want: u32 },
    #[error("a module named `{0}` is already registered")]
    Duplicate(String),
}

/// The registered modules.
#[derive(Default)]
pub struct Modules {
    by_name: BTreeMap<String, Box<dyn Module>>,
}

impl Modules {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, module: Box<dyn Module>) -> Result<(), ModuleError> {
        let name = module.name().to_owned();
        if !(MIN_INTERFACE_VERSION..=INTERFACE_VERSION).contains(&module.interface_version()) {
            return Err(ModuleError::InterfaceMismatch {
                module: name,
                got: module.interface_version(),
                want: INTERFACE_VERSION,
            });
        }
        if self.by_name.contains_key(&name) {
            return Err(ModuleError::Duplicate(name));
        }
        self.by_name.insert(name, module);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&dyn Module> {
        self.by_name.get(name).map(|m| m.as_ref())
    }

    /// The registered names, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }
}

/// A module over a fixed list of choices: the smallest module, and the one the tests use.
pub struct ChoiceList {
    name: String,
    choices: Vec<Choice>,
}

impl ChoiceList {
    pub fn new(name: &str, choices: Vec<Choice>) -> Self {
        ChoiceList {
            name: name.to_owned(),
            choices,
        }
    }
}

impl Module for ChoiceList {
    fn name(&self) -> &str {
        &self.name
    }

    fn list_choices(&self, _needs: &Needs) -> Vec<Choice> {
        self.choices.clone()
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let id = match value {
            Value::String(s) | Value::Enum(s) => s,
            other => {
                return Err(format!(
                    "expected a choice id, found {}",
                    other.type_of().name()
                ));
            }
        };
        if self.choices.iter().any(|c| &c.id == id) {
            Ok(())
        } else {
            let ids: Vec<&str> = self.choices.iter().map(|c| c.id.as_str()).collect();
            Err(format!("`{id}` is not one of: {}", ids.join(", ")))
        }
    }

    fn on_change(&self, old: &Value, new: &Value) -> Plan {
        match (old, new) {
            (Value::String(a) | Value::Enum(a), Value::String(b) | Value::Enum(b)) if a != b => {
                Plan {
                    actions: vec![Action {
                        description: format!("`{}` changed from `{a}` to `{b}`", self.name),
                        kind: ActionKind::Note,
                    }],
                }
            }
            _ => Plan::default(),
        }
    }
}
