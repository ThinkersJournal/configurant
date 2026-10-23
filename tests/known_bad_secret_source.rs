// SPDX-License-Identifier: MIT OR Apache-2.0
//! A secret's source is shown wherever the secret is described, as the reference only: `describe`/`help`,
//! `explain`, a front end's field, and `check`. The value never is.

use std::collections::BTreeMap;
use std::process::Command;

use configurant::edit::Session;
use configurant::{Loader, Schema};

const CANARY: &str = "hunter2-canary";

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "api.token"
type = "string"
secret = true
env = "DEMO_API_TOKEN"
source = "vault:DEMO_API_TOKEN_ENTRY"
description = "api token"

[[value]]
key = "db.password"
type = "string"
secret = true
env = "DEMO_DB_PASSWORD"
description = "database password"
"#;

fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("valid")
}

#[test]
fn help_shows_the_reference_only_for_the_secret_that_declares_one() {
    let help = schema().help();
    assert!(
        help.contains("[source: vault:DEMO_API_TOKEN_ENTRY]"),
        "{help}"
    );
    assert_eq!(help.matches("[source:").count(), 1);
    assert!(!help.contains(CANARY));
}

#[test]
fn explain_names_the_source_and_still_redacts_the_value() {
    let schema = schema();
    let config = Loader::new(&schema)
        .resolved(BTreeMap::from([(
            "api.token".to_owned(),
            CANARY.to_owned(),
        )]))
        .build()
        .unwrap();
    let e = config.explain("api.token").unwrap();
    assert_eq!(e.source.as_deref(), Some("vault:DEMO_API_TOKEN_ENTRY"));
    let shown = e.to_string();
    assert!(
        shown.contains("[source: vault:DEMO_API_TOKEN_ENTRY]"),
        "{shown}"
    );
    assert!(shown.contains("secret source `vault` (reference `DEMO_API_TOKEN_ENTRY`): <redacted>"));
    assert!(!shown.contains(CANARY) && !format!("{e:?}").contains(CANARY));
    assert!(
        config.explain("db.password").is_err()
            || config.explain("db.password").unwrap().source.is_none()
    );
}

#[test]
fn a_front_ends_field_carries_the_reference_and_never_the_value() {
    let schema = schema();
    let config = Loader::new(&schema)
        .resolved(BTreeMap::from([(
            "api.token".to_owned(),
            CANARY.to_owned(),
        )]))
        .build()
        .unwrap();
    let fields = Session::new(&config, None).fields();
    let token = fields.iter().find(|f| f.key == "api.token").unwrap();
    assert_eq!(token.source.as_deref(), Some("vault:DEMO_API_TOKEN_ENTRY"));
    assert!(!token.editable);
    assert_eq!(token.current.as_deref(), Some("<redacted>"));
    let pw = fields.iter().find(|f| f.key == "db.password").unwrap();
    assert!(pw.source.is_none());
    assert!(!format!("{fields:?}").contains(CANARY));
}

#[test]
fn check_says_which_sources_the_schema_names_and_that_this_binary_has_none() {
    let path = std::env::temp_dir().join(format!("cfg-srcs-{}.toml", std::process::id()));
    std::fs::write(&path, SCHEMA).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_configurant"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{said}");
    assert!(said.contains("`api.token` names secret source `vault:DEMO_API_TOKEN_ENTRY`: not built into this binary"), "{said}");
    assert!(!said.contains("db.password"), "{said}");
}
