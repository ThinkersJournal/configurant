// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad inputs for adding a value to a schema's text (`schema_edit::add_value`): each rule starts here
//! as a failing test.

use configurant::schema_edit::{NewValue, SchemaEditError, add_value};
use configurant::{Schema, ValueType};

const SCHEMA: &str = r#"# the demo schema
[project]
name = "t"
version = "1.0.0"
schema_version = 1

# the port
[[value]]
key = "server.port"
type = "int"
default = 8080
description = "listen port"
env = "T_PORT"
"#;

fn nv(key: &str, ty: ValueType) -> NewValue {
    NewValue::new(key, ty, "found by a scan")
}

#[test]
fn an_added_value_parses_and_comments_survive() {
    let mut n = nv("server.host", ValueType::String);
    n.env = Some("T_HOST".into());
    n.default = Some(toml::Value::String("localhost".into()));
    let out = add_value(SCHEMA, &n).unwrap();
    assert!(out.contains("# the demo schema") && out.contains("# the port"));
    let s = Schema::parse(&out).unwrap();
    assert_eq!(s.get("server.host").unwrap().env.as_deref(), Some("T_HOST"));
    assert!(s.get("server.port").is_some());
}

#[test]
fn an_existing_key_is_refused_and_the_text_is_untouched() {
    let err = add_value(SCHEMA, &nv("server.port", ValueType::Int)).unwrap_err();
    assert!(matches!(err, SchemaEditError::Invalid(_)));
}

#[test]
fn an_existing_env_name_is_refused() {
    let mut n = nv("server.other", ValueType::Int);
    n.env = Some("T_PORT".into());
    assert!(matches!(
        add_value(SCHEMA, &n).unwrap_err(),
        SchemaEditError::Invalid(_)
    ));
}

#[test]
fn a_secret_with_a_default_is_refused_and_the_default_is_never_in_the_error() {
    let mut n = nv("db.password", ValueType::String);
    n.secret = true;
    n.default = Some(toml::Value::String("hunter2-canary".into()));
    let err = add_value(SCHEMA, &n).unwrap_err();
    // No assert message below formats the error: a failure must not print anything near a secret.
    assert!(matches!(err, SchemaEditError::SecretWithDefault(_)));
    let leaked = format!("{err} {err:?}").contains("hunter2-canary");
    assert!(!leaked);
}

#[test]
fn a_secret_is_added_without_a_default_or_flag() {
    let mut n = nv("db.password", ValueType::String);
    n.secret = true;
    n.env = Some("T_DB_PASSWORD".into());
    let out = add_value(SCHEMA, &n).unwrap();
    let s = Schema::parse(&out).unwrap();
    let d = s.get("db.password").unwrap();
    assert!(d.secret && d.default.is_none() && d.flag.is_none());
}

#[test]
fn a_composite_type_is_refused() {
    assert!(matches!(
        add_value(SCHEMA, &nv("x.rows", ValueType::Table)).unwrap_err(),
        SchemaEditError::Invalid(_)
    ));
}

#[test]
fn a_schema_that_does_not_parse_is_refused() {
    assert!(matches!(
        add_value("not = [toml", &nv("a.b", ValueType::Int)).unwrap_err(),
        SchemaEditError::SchemaParse(_)
    ));
}

#[test]
fn an_enum_without_choices_is_refused() {
    assert!(matches!(
        add_value(SCHEMA, &nv("a.mode", ValueType::Enum)).unwrap_err(),
        SchemaEditError::Invalid(_)
    ));
}
