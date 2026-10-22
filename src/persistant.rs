// SPDX-License-Identifier: MIT OR Apache-2.0
//! PersistAnt as a configurant [`Module`] (cargo feature `persistant`).
//!
//! This is the five-step contract from PersistAnt's registry, seen from the configuration side: the
//! project's needs go in ([`Needs::required`] are capability ids that must all hold, [`Needs::order`] is
//! what matters most, most important first), the module returns the backends this build offers, best
//! first, each with the reasons ([`Module::list_choices`]); the operator's pick is stored as the value of
//! a key whose schema entry says `provider = "<this module>"`; a change of that value returns a
//! [`Plan`] holding a data transfer between the two backends. The module owns **no list of backends**:
//! every id comes from `persistant::registry`, so a PersistAnt upgrade (and a rebuild) reaches every
//! project whose needs the new backend meets. Which backends exist is decided by the application's own
//! `persistant` features (`registry-sqlite`, ...); this crate only turns on the base registry.
//!
//! The module never opens a backend or copies data: a [`Plan`] is data and the front end executes it
//! after confirmation. The endpoint (path or URL, possibly a secret) is not this module's business.

use ::persistant::capability::{CapabilityId, provider::ProviderNeeds};
use ::persistant::registry;

pub use ::persistant::capability::choose::Durability;

use crate::error::suggestion;
use crate::module::{Action, ActionKind, Choice, Module, Needs, Plan};
use crate::value::Value;

/// Why the needs could not be turned into a list of backends.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PersistantError {
    /// A string in the needs is not a PersistAnt capability id.
    #[error("`{id}` is not a capability id: {reason}")]
    BadCapability { id: String, reason: String },
    /// No backend in this build meets the whole need (PersistAnt names, per backend, what it lacks).
    #[error("{0}")]
    Unmet(String),
}

/// The PersistAnt module.
pub struct PersistantModule {
    name: String,
    min_durability: Durability,
    needs: Option<Needs>,
}

impl Default for PersistantModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistantModule {
    /// Named `persistant`, asking for at least process-crash durability (a backend that forgets on exit
    /// is not persistence unless the project says so with [`Self::min_durability`]).
    pub fn new() -> Self {
        PersistantModule {
            name: "persistant".to_owned(),
            min_durability: Durability::Process,
            needs: None,
        }
    }

    /// The name a schema's `provider` refers to (default `persistant`).
    #[must_use]
    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_owned();
        self
    }

    /// The lowest durability a listed backend may have.
    #[must_use]
    pub fn min_durability(mut self, durability: Durability) -> Self {
        self.min_durability = durability;
        self
    }

    /// The project's needs, so `validate` can refuse a stored backend id that no longer meets them.
    #[must_use]
    pub fn needs(mut self, needs: Needs) -> Self {
        self.needs = Some(needs);
        self
    }

    /// As [`Module::list_choices`], but says why when there is nothing to list. The trait method has
    /// no error channel and returns an empty list; a setup UI should call this one.
    pub fn try_list_choices(&self, needs: &Needs) -> Result<Vec<Choice>, PersistantError> {
        let wanted = self.provider_needs(needs)?;
        let listings =
            registry::providers(&wanted).map_err(|u| PersistantError::Unmet(u.to_string()))?;
        Ok(listings
            .into_iter()
            .map(|l| Choice {
                id: l.id.to_owned(),
                reason: format!("{}: {}", l.title, l.why().join("; ")),
            })
            .collect())
    }

    fn provider_needs(&self, needs: &Needs) -> Result<ProviderNeeds, PersistantError> {
        let ids = |list: &[String]| -> Result<Vec<CapabilityId>, PersistantError> {
            list.iter()
                .map(|s| {
                    CapabilityId::new(s.as_str()).map_err(|e| PersistantError::BadCapability {
                        id: s.clone(),
                        reason: e.to_string(),
                    })
                })
                .collect()
        };
        Ok(
            ProviderNeeds::new(ids(&needs.required)?, self.min_durability)
                .prefer(ids(&needs.order)?),
        )
    }
}

impl Module for PersistantModule {
    fn name(&self) -> &str {
        &self.name
    }

    fn list_choices(&self, needs: &Needs) -> Vec<Choice> {
        self.try_list_choices(needs).unwrap_or_default()
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let id = match value {
            Value::String(s) | Value::Enum(s) => s,
            other => {
                return Err(format!(
                    "expected a backend id, found {}",
                    other.type_of().name()
                ));
            }
        };
        let known = registry::known_ids();
        if !known.contains(&id.as_str()) {
            let hint = suggestion(id, known.iter().copied())
                .map(|s| format!(" (did you mean `{s}`?)"))
                .unwrap_or_default();
            return Err(format!(
                "`{id}` is not a backend this build offers{hint}; it offers: {}",
                known.join(", ")
            ));
        }
        if let Some(needs) = &self.needs {
            let offered = self
                .try_list_choices(needs)
                .map_err(|e| format!("the project's needs cannot be met: {e}"))?;
            if !offered.iter().any(|c| &c.id == id) {
                let ids: Vec<&str> = offered.iter().map(|c| c.id.as_str()).collect();
                return Err(format!(
                    "`{id}` does not meet the project's needs; backends that do: {}",
                    ids.join(", ")
                ));
            }
        }
        Ok(())
    }

    fn on_change(&self, old: &Value, new: &Value) -> Plan {
        let (Value::String(a) | Value::Enum(a), Value::String(b) | Value::Enum(b)) = (old, new)
        else {
            return Plan::default();
        };
        if a == b {
            return Plan::default();
        }
        let known = registry::known_ids();
        let action = if known.contains(&a.as_str()) && known.contains(&b.as_str()) {
            Action {
                description: format!(
                    "copy the stored records from backend `{a}` to backend `{b}` (needs both endpoints; \
                     nothing is copied until you confirm)"
                ),
                kind: ActionKind::DataTransfer {
                    from: a.clone(),
                    to: b.clone(),
                },
            }
        } else {
            Action {
                description: format!(
                    "backend changed from `{a}` to `{b}`; one of them is not offered by this build, \
                     so no automatic copy is possible"
                ),
                kind: ActionKind::Note,
            }
        };
        Plan {
            actions: vec![action],
        }
    }
}
