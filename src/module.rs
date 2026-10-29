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

use crate::value::Value;

/// Version of this interface. A module built against another version is refused at registration.
pub const INTERFACE_VERSION: u32 = 1;

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

    /// The interface version this module was written against.
    fn interface_version(&self) -> u32 {
        INTERFACE_VERSION
    }

    /// The choices for `needs`, best first, each with a reason.
    fn list_choices(&self, needs: &Needs) -> Vec<Choice>;

    /// Is `value` acceptable? The error text may be shown unless the value is a secret.
    fn validate(&self, value: &Value) -> Result<(), String>;

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
        if module.interface_version() != INTERFACE_VERSION {
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
