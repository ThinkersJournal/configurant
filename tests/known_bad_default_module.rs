// SPDX-License-Identifier: MIT OR Apache-2.0
//! A schema default is checked by its value's module at load, like any other layer: a default its own module
//! rejects is an error naming the key. With no module registered a default still loads (as before).

use std::collections::BTreeMap;

use configurant::{Choice, ChoiceList, ConfigError, Errors, Loader, Modules, Origin, Schema};

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "storage.backend"
type = "string"
default = "mysql"
description = "chosen from the storage module's list"
env = "BACKEND"
provider = "storage"
"#;

fn storage() -> Modules {
    let mut modules = Modules::new();
    modules
        .register(Box::new(ChoiceList::new(
            "storage",
            vec![
                Choice::new("sqlite", "embedded, durable"),
                Choice::new("redb", "pure Rust"),
            ],
        )))
        .unwrap();
    modules
}

fn errors_of<T: std::fmt::Debug>(r: Result<T, Errors>) -> Vec<ConfigError> {
    r.expect_err("expected the load to fail").0
}

#[test]
fn a_default_its_module_rejects_is_an_error_naming_the_key() {
    let s = Schema::parse(SCHEMA).unwrap();
    let modules = storage();
    let errs = errors_of(Loader::new(&s).modules(&modules).build());
    assert!(
        errs.iter().any(
            |e| matches!(e, ConfigError::ModuleRejected { key, module, origin, .. }
            if key == "storage.backend" && module == "storage" && *origin == Origin::Default)
        ),
        "{errs:?}"
    );
}

#[test]
fn an_override_of_a_bad_default_does_not_hide_the_bad_default() {
    // The default layer is still part of the load; the environment winning does not excuse it.
    let s = Schema::parse(SCHEMA).unwrap();
    let modules = storage();
    let env = BTreeMap::from([("BACKEND".to_owned(), "redb".to_owned())]);
    let errs = errors_of(Loader::new(&s).modules(&modules).env(env).build());
    assert!(errs.iter().any(
        |e| matches!(e, ConfigError::ModuleRejected { origin, .. } if *origin == Origin::Default)
    ));
}

#[test]
fn a_default_the_module_accepts_loads_and_with_no_module_a_default_still_loads() {
    let ok = SCHEMA.replace("default = \"mysql\"", "default = \"sqlite\"");
    let s = Schema::parse(&ok).unwrap();
    let modules = storage();
    let cfg = Loader::new(&s).modules(&modules).build().unwrap();
    assert_eq!(cfg.get::<String>("storage.backend").unwrap(), "sqlite");

    let s = Schema::parse(SCHEMA).unwrap();
    assert!(Loader::new(&s).build().is_ok());
}
