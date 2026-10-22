// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad edits. A front end changes values through `edit::Session`; each bad edit must be
//! REFUSED with a named error, no secret may be accepted or leak, and nothing is written or planned
//! until the user has confirmed.

use std::collections::BTreeMap;

use configurant::edit::Session;
use configurant::{
    ActionKind, Choice, ChoiceList, Config, ConfigError, Loader, Modules, Needs, Origin, Schema,
    Value,
};

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1
env_prefix = "DEMO_"

[[value]]
key = "storage.backend"
type = "string"
default = "sqlite"
description = "where records live"
provider = "store"
restart = true

[[value]]
key = "server.port"
type = "int"
default = 8080
min = 1
max = 65535
description = "listen port"

[[value]]
key = "log.level"
type = "enum"
choices = ["debug", "info"]
default = "info"
description = "verbosity"

[[value]]
key = "db.password"
type = "string"
secret = true
description = "database password"
env = "DEMO_DB_PASSWORD"
"#;

const FILE: &str = "# my settings\n[server]\nport = 9000 # keep\n";

fn schema() -> Schema {
    Schema::parse(SCHEMA).unwrap()
}

fn modules() -> Modules {
    let mut m = Modules::new();
    m.register(Box::new(ChoiceList::new(
        "store",
        vec![
            Choice::new("sqlite", "embedded"),
            Choice::new("redb", "pure rust"),
        ],
    )))
    .unwrap();
    m
}

fn config<'a>(s: &'a Schema, m: &'a Modules) -> Config {
    Loader::new(s)
        .modules(m)
        .file("demo.toml", FILE)
        .env(BTreeMap::from([(
            "DEMO_DB_PASSWORD".into(),
            "hunter2-secret".into(),
        )]))
        .build()
        .unwrap()
}

#[test]
fn unknown_key_is_refused_with_a_suggestion() {
    let (s, m) = (schema(), modules());
    let c = config(&s, &m);
    let mut session = Session::new(&c, Some(&m));
    let e = session.stage("server.prot", "1").unwrap_err();
    assert!(
        matches!(e, ConfigError::UnknownKey { suggestion: Some(ref s), .. } if s == "server.port")
    );
}

#[test]
fn wrong_type_out_of_range_and_not_allowed_are_refused() {
    let (s, m) = (schema(), modules());
    let c = config(&s, &m);
    let mut session = Session::new(&c, Some(&m));
    assert!(matches!(
        session.stage("server.port", "abc"),
        Err(ConfigError::WrongType { .. })
    ));
    assert!(matches!(
        session.stage("server.port", "70000"),
        Err(ConfigError::OutOfRange { .. })
    ));
    assert!(matches!(
        session.stage("log.level", "loud"),
        Err(ConfigError::NotAllowed { .. })
    ));
    assert!(matches!(
        session.stage("storage.backend", "oracle"),
        Err(ConfigError::ModuleRejected { .. })
    ));
    assert!(
        session.review().is_empty(),
        "a refused edit must stage nothing"
    );
}

#[test]
fn a_secret_cannot_be_staged_and_never_appears() {
    let (s, m) = (schema(), modules());
    let c = config(&s, &m);
    let mut session = Session::new(&c, Some(&m));
    let e = session
        .stage("db.password", "typed-secret-xyz")
        .unwrap_err();
    assert!(matches!(
        e,
        ConfigError::SecretInForbiddenSource {
            origin: Origin::Edit,
            ..
        }
    ));
    session.stage("server.port", "9001").unwrap();
    let mut seen = format!("{e} {e:?} {session:?}");
    for f in session.fields() {
        seen += &format!("{f:?} {:?} {:?}", f.current, f.staged);
    }
    let review = session.review();
    seen += &format!("{review:?}");
    let confirmed = review.confirm();
    seen += &format!("{confirmed:?} {}", confirmed.file_text(FILE).unwrap());
    for secret in ["typed-secret-xyz", "hunter2-secret"] {
        assert!(!seen.contains(secret), "a secret value leaked into output");
    }
    let f = session
        .fields()
        .into_iter()
        .find(|f| f.key == "db.password")
        .unwrap();
    assert_eq!(f.current.as_deref(), Some("<redacted>"));
    assert!(!f.editable);
}

#[test]
fn staging_the_current_value_is_not_a_change() {
    let (s, m) = (schema(), modules());
    let c = config(&s, &m);
    let mut session = Session::new(&c, Some(&m));
    session.stage("server.port", "9000").unwrap();
    assert!(session.review().is_empty());
}

#[test]
fn an_edit_is_written_only_through_a_confirmation_and_keeps_the_file() {
    let (s, m) = (schema(), modules());
    let c = config(&s, &m);
    let mut session = Session::new(&c, Some(&m));
    session.stage("server.port", "9001").unwrap();
    session.stage("log.level", "debug").unwrap();
    let text = session.review().confirm().file_text(FILE).unwrap();
    assert!(text.contains("# my settings"), "comment lost: {text}");
    assert!(
        text.contains("9001 # keep"),
        "trailing comment lost: {text}"
    );
    let reloaded = Loader::new(&s)
        .modules(&m)
        .file("demo.toml", &text)
        .build()
        .unwrap();
    assert_eq!(reloaded.get::<i64>("server.port").unwrap(), 9001);
    assert_eq!(reloaded.get::<String>("log.level").unwrap(), "debug");
    // Unstaging forgets the edit.
    session.unstage("server.port");
    session.unstage("log.level");
    assert!(session.review().is_empty());
}

#[test]
fn the_review_lists_changes_restarts_and_the_modules_plan() {
    let (s, m) = (schema(), modules());
    let c = config(&s, &m);
    let mut session = Session::new(&c, Some(&m));
    session.stage("storage.backend", "redb").unwrap();
    let review = session.review();
    assert_eq!(review.changes().len(), 1);
    assert_eq!(review.changes()[0].key, "storage.backend");
    assert_eq!(review.changes()[0].from.as_deref(), Some("\"sqlite\""));
    assert_eq!(review.changes()[0].to, "\"redb\"");
    assert_eq!(review.restart_needed(), ["storage.backend"]);
    assert!(
        review
            .plan()
            .actions
            .iter()
            .any(|a| matches!(a.kind, ActionKind::Note))
    );
    assert_eq!(review.confirm().plan().actions.len(), 1);
}

#[test]
fn choices_come_from_the_schema_or_the_module() {
    let (s, m) = (schema(), modules());
    let c = config(&s, &m);
    let session = Session::new(&c, Some(&m));
    let by_key = |k: &str| session.fields().into_iter().find(|f| f.key == k).unwrap();
    assert_eq!(by_key("log.level").choices, ["debug", "info"]);
    let ids: Vec<_> = session.choices("storage.backend", &Needs::default());
    assert_eq!(
        ids.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["sqlite", "redb"]
    );
    assert!(matches!(Value::Int(1), Value::Int(_)));
}
