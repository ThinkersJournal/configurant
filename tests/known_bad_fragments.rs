// SPDX-License-Identifier: MIT OR Apache-2.0
//! Schema fragments: a library ships its own options as a schema fragment and an application mounts it
//! under a name (`docs/SCHEMA-FRAGMENTS.md`). A mount that collides, changes a type or clears a secret is
//! a named error, never a silent merge.

use std::collections::BTreeMap;

use configurant::{Loader, Mount, Schema, SchemaError};

const FRAGMENT: &str = r#"
[fragment]
name = "webhost"
version = "1.0.0"
summary = "web host options"
schema_version = 1

[[value]]
key = "port"
type = "int"
default = 8080
min = 1
max = 65535
env = "PORT"
flag = "--port"
description = "listen port"

[[value]]
key = "tls.token"
type = "string"
secret = true
env = "TOKEN"
description = "certificate api token"

[[table]]
key = "site"
description = "hosted sites"

[[table.field]]
key = "host"
type = "string"
description = "host name"
"#;

fn app(extra: &str) -> String {
    format!(
        r#"
[project]
name = "app"
version = "0.1.0"
summary = "an app"
schema_version = 1
env_prefix = "APP_"

[[value]]
key = "name"
type = "string"
default = "demo"
description = "app name"
{extra}
"#
    )
}

fn mounted(extra: &str) -> Result<Schema, SchemaError> {
    Schema::parse_with(&app(extra), &[Mount::new("webhost", FRAGMENT)])
}

#[test]
fn mounted_keys_live_under_the_mount_name_with_derived_env_and_flag() {
    let s = mounted("").expect("mounts");
    let port = s.get("webhost.port").expect("mounted key");
    assert_eq!(port.env.as_deref(), Some("APP_WEBHOST_PORT"));
    assert_eq!(port.flag.as_deref(), Some("--webhost-port"));
    assert!(s.get("webhost.tls.token").is_some());
    assert!(s.table("webhost.site").is_some());
    let env = BTreeMap::from([("APP_WEBHOST_PORT".to_owned(), "9000".to_owned())]);
    let cfg = Loader::new(&s).env(env).build().expect("loads");
    assert_eq!(cfg.get::<i64>("webhost.port").unwrap(), 9000);
    let cfg = Loader::new(&s)
        .cli(vec!["--webhost-port=7000".into()])
        .build()
        .expect("loads");
    assert_eq!(cfg.get::<i64>("webhost.port").unwrap(), 7000);
    let cfg = Loader::new(&s).build().expect("loads");
    assert_eq!(cfg.get::<i64>("webhost.port").unwrap(), 8080);
}

#[test]
fn a_mounted_range_still_applies() {
    let s = mounted("").expect("mounts");
    let env = BTreeMap::from([("APP_WEBHOST_PORT".to_owned(), "70000".to_owned())]);
    assert!(Loader::new(&s).env(env).build().is_err());
}

#[test]
fn the_application_may_override_a_default_and_nothing_else() {
    let s = mounted(
        r#"
[[value]]
key = "webhost.port"
type = "int"
default = 3000
description = "this app listens on 3000"
"#,
    )
    .expect("override of a default");
    let cfg = Loader::new(&s).build().expect("loads");
    assert_eq!(cfg.get::<i64>("webhost.port").unwrap(), 3000);
    // the fragment's range and env survive the override
    assert!(s.get("webhost.port").unwrap().max.is_some());
    assert_eq!(
        s.get("webhost.port").unwrap().env.as_deref(),
        Some("APP_WEBHOST_PORT")
    );
}

#[test]
fn an_override_may_not_change_the_type() {
    let e = mounted(
        r#"
[[value]]
key = "webhost.port"
type = "string"
default = "x"
description = "now text"
"#,
    )
    .expect_err("refused");
    assert!(matches!(e, SchemaError::MountOverride { .. }), "{e:?}");
}

#[test]
fn an_override_may_not_change_the_range_or_the_env() {
    for extra in ["max = 9", r#"env = "OTHER""#, "secret = true"] {
        let e = mounted(&format!(
            r#"
[[value]]
key = "webhost.port"
type = "int"
description = "x"
{extra}
"#
        ))
        .expect_err("refused");
        assert!(
            matches!(e, SchemaError::MountOverride { .. }),
            "{extra}: {e:?}"
        );
    }
}

#[test]
fn a_mounted_secret_stays_secret_and_cannot_get_a_default() {
    let s = mounted("").expect("mounts");
    let canary = "canary-s3cr3t-value";
    let env = BTreeMap::from([("APP_WEBHOST_TOKEN".to_owned(), canary.to_owned())]);
    let cfg = Loader::new(&s).env(env).build().expect("loads");
    let shown = format!(
        "{:?} {} {}",
        cfg.explain("webhost.tls.token").unwrap(),
        cfg.explain("webhost.tls.token").unwrap(),
        s.help()
    );
    assert!(!shown.contains(canary), "{shown}");
    assert!(shown.contains("<redacted>"), "{shown}");
    let e = mounted(
        r#"
[[value]]
key = "webhost.tls.token"
type = "string"
default = "x"
description = "a default"
"#,
    )
    .expect_err("refused");
    assert!(matches!(e, SchemaError::SecretWithDefault(_)), "{e:?}");
}

#[test]
fn a_collision_with_an_application_table_is_refused() {
    let e = mounted(
        r#"
[[table]]
key = "webhost.site"
description = "clash"
"#,
    )
    .expect_err("refused");
    assert!(matches!(e, SchemaError::DuplicateKey(_)), "{e:?}");
}

#[test]
fn the_same_mount_name_twice_is_refused() {
    let e = Schema::parse_with(
        &app(""),
        &[
            Mount::new("webhost", FRAGMENT),
            Mount::new("webhost", FRAGMENT),
        ],
    )
    .expect_err("refused");
    assert!(matches!(e, SchemaError::DuplicateMount(_)), "{e:?}");
}

#[test]
fn two_mounts_of_one_fragment_under_different_names_both_work() {
    let s = Schema::parse_with(
        &app(""),
        &[
            Mount::new("public", FRAGMENT),
            Mount::new("admin", FRAGMENT),
        ],
    )
    .expect("mounts");
    assert!(s.get("public.port").is_some() && s.get("admin.port").is_some());
    assert_eq!(
        s.get("admin.port").unwrap().env.as_deref(),
        Some("APP_ADMIN_PORT")
    );
}

#[test]
fn a_bad_mount_name_is_refused() {
    let e = Schema::parse_with(&app(""), &[Mount::new("web host", FRAGMENT)]).expect_err("refused");
    assert!(matches!(e, SchemaError::BadKey(_)), "{e:?}");
}

#[test]
fn a_fragment_that_is_a_project_or_a_newer_version_is_refused() {
    let project = FRAGMENT.replace("[fragment]", "[project]");
    let e = Schema::parse_with(&app(""), &[Mount::new("w", &project)]).expect_err("refused");
    assert!(matches!(e, SchemaError::FragmentParse { .. }), "{e:?}");
    let newer = FRAGMENT.replace("schema_version = 1", "schema_version = 2");
    let e = Schema::parse_with(&app(""), &[Mount::new("w", &newer)]).expect_err("refused");
    assert!(matches!(e, SchemaError::UnsupportedVersion(2)), "{e:?}");
}

#[test]
fn a_fragment_with_migrations_is_refused_for_now() {
    let with = format!("{FRAGMENT}\n[[migration]]\nfrom = 1\nto = 2\n");
    let e = Schema::parse_with(&app(""), &[Mount::new("w", &with)]).expect_err("refused");
    assert!(matches!(e, SchemaError::FragmentParse { .. }), "{e:?}");
}

#[test]
fn a_typo_in_a_mounted_key_is_an_unknown_key_with_a_suggestion() {
    let s = mounted("").expect("mounts");
    let file = "[webhost]\nprot = 1\n";
    let e = Loader::new(&s)
        .file("app.toml", file)
        .build()
        .expect_err("refused");
    assert!(e.to_string().contains("webhost.port"), "{e}");
}
