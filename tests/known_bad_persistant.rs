// SPDX-License-Identifier: MIT OR Apache-2.0
//! The PersistAnt module (feature `persistant`). Known-bad first: a need no backend meets, a capability id
//! that is not one, a backend id nobody offers, a durability the backend cannot give.

#![cfg(feature = "persistant")]

use configurant::persistant::{Durability, PersistantError, PersistantModule};
use configurant::{
    ActionKind, ConfigError, INTERFACE_VERSION, Loader, Module, Modules, Needs, Origin, Schema,
    Value,
};

fn needs(required: &[&str], order: &[&str]) -> Needs {
    Needs {
        required: required.iter().map(|s| s.to_string()).collect(),
        order: order.iter().map(|s| s.to_string()).collect(),
    }
}

fn ids(m: &PersistantModule, n: &Needs) -> Vec<String> {
    m.list_choices(n).into_iter().map(|c| c.id).collect()
}

#[test]
fn the_choice_list_is_what_this_build_offers_for_the_needs_with_reasons() {
    let n = needs(&["kv.get", "kv.put"], &["kv.ttl"]);
    let m = PersistantModule::new().min_durability(Durability::Volatile);
    let choices = m.list_choices(&n);
    let got: Vec<&str> = choices.iter().map(|c| c.id.as_str()).collect();
    assert!(got.contains(&"memory") && got.contains(&"fs"), "{got:?}");
    // The order of importance reached PersistAnt: its reasons say how each backend fares on `kv.ttl`.
    assert!(
        choices.iter().all(|c| c.reason.contains("kv.ttl")),
        "{choices:?}"
    );
    assert!(
        choices.iter().all(|c| c.reason.contains("durability")),
        "{choices:?}"
    );
    // The module is a view of the registry, not a list of its own: every id it returns is one the
    // registry knows, and nothing the registry does not know is invented.
    let known = ::persistant::registry::known_ids();
    assert!(
        got.iter().all(|id| known.contains(id)),
        "{got:?} vs {known:?}"
    );
}

#[test]
fn a_durability_floor_removes_the_backends_that_cannot_meet_it() {
    let n = needs(&["kv.get", "kv.put"], &[]);
    let volatile = ids(
        &PersistantModule::new().min_durability(Durability::Volatile),
        &n,
    );
    let process = ids(
        &PersistantModule::new().min_durability(Durability::Process),
        &n,
    );
    assert!(volatile.contains(&"memory".to_string()), "{volatile:?}");
    assert!(!process.contains(&"memory".to_string()), "{process:?}");
    assert!(process.contains(&"fs".to_string()), "{process:?}");
}

#[test]
fn a_need_no_backend_meets_is_a_named_error_and_an_empty_list() {
    let n = needs(&["kv.get", "kv.no_such_capability"], &[]);
    let m = PersistantModule::new().min_durability(Durability::Volatile);
    let e = m.try_list_choices(&n).expect_err("nothing offers it");
    assert!(
        matches!(&e, PersistantError::Unmet(t) if t.contains("kv.no_such_capability")),
        "{e}"
    );
    assert!(
        m.list_choices(&n).is_empty(),
        "no closest match is invented"
    );
}

#[test]
fn a_string_that_is_not_a_capability_id_is_refused_by_name() {
    let m = PersistantModule::new();
    let e = m
        .try_list_choices(&needs(&["Not A Capability!"], &[]))
        .expect_err("not an id");
    assert!(
        matches!(&e, PersistantError::BadCapability { id, .. } if id == "Not A Capability!"),
        "{e}"
    );
    let e = m
        .try_list_choices(&needs(&["kv.get"], &["also bad"]))
        .expect_err("not an id in the order either");
    assert!(matches!(e, PersistantError::BadCapability { .. }));
}

#[test]
fn validate_accepts_an_offered_id_and_refuses_the_rest_with_a_suggestion() {
    let m = PersistantModule::new();
    assert!(m.validate(&Value::String("fs".into())).is_ok());
    let e = m.validate(&Value::String("memry".into())).unwrap_err();
    assert!(e.contains("did you mean `memory`"), "{e}");
    let e = m.validate(&Value::String("zzzzzzzz".into())).unwrap_err();
    assert!(
        e.contains("memory") && e.contains("fs"),
        "the offered ids are named: {e}"
    );
    assert!(m.validate(&Value::Int(1)).is_err());
}

#[test]
fn with_needs_configured_validate_refuses_a_backend_that_does_not_meet_them() {
    let n = needs(&["kv.get", "kv.put"], &[]);
    let m = PersistantModule::new()
        .min_durability(Durability::Process)
        .needs(n);
    assert!(m.validate(&Value::String("fs".into())).is_ok());
    let e = m.validate(&Value::String("memory".into())).unwrap_err();
    assert!(e.contains("memory") && e.contains("needs"), "{e}");
}

#[test]
fn a_changed_backend_returns_a_data_transfer_plan_and_nothing_else_happens() {
    let m = PersistantModule::new();
    let plan = m.on_change(&Value::String("fs".into()), &Value::String("memory".into()));
    assert_eq!(plan.actions.len(), 1, "{plan:?}");
    assert_eq!(
        plan.actions[0].kind,
        ActionKind::DataTransfer {
            from: "fs".into(),
            to: "memory".into()
        }
    );
    // Not changed: no plan.
    assert!(
        m.on_change(&Value::String("fs".into()), &Value::String("fs".into()))
            .actions
            .is_empty()
    );
    // The old id is not one this build offers: a note, never a transfer it cannot perform.
    let plan = m.on_change(&Value::String("gone".into()), &Value::String("fs".into()));
    assert!(
        plan.actions.iter().all(|a| a.kind == ActionKind::Note),
        "{plan:?}"
    );
    assert!(!plan.actions.is_empty());
}

#[test]
fn it_plugs_into_the_loader_through_the_schema_provider_key() {
    const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "storage.backend"
type = "string"
default = "fs"
description = "chosen from PersistAnt's list"
env = "BACKEND"
provider = "storage"
"#;
    let s = Schema::parse(SCHEMA).unwrap();
    let mut modules = Modules::new();
    modules
        .register(Box::new(PersistantModule::new().named("storage")))
        .unwrap();
    assert_eq!(
        PersistantModule::new().interface_version(),
        INTERFACE_VERSION
    );
    let env = |v: &str| {
        [("BACKEND".to_string(), v.to_string())]
            .into_iter()
            .collect()
    };
    let ok = Loader::new(&s)
        .modules(&modules)
        .env(env("memory"))
        .build()
        .unwrap();
    assert_eq!(ok.get::<String>("storage.backend").unwrap(), "memory");
    let errs = Loader::new(&s)
        .modules(&modules)
        .env(env("memry"))
        .build()
        .unwrap_err()
        .0;
    assert!(
        matches!(&errs[0], ConfigError::ModuleRejected { module, origin: Origin::Env(_), reason, .. }
            if module == "storage" && reason.contains("did you mean `memory`")),
        "{errs:?}"
    );
}
