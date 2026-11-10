// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fragment migrations (ROADMAP row 31, `docs/SCHEMA-FRAGMENTS.md` item 5): a library that renames an option
//! ships `[[migration]]` steps in its fragment and a `config_version` of its own, kept in the application's
//! file under the mount name, so an existing file still loads.

use configurant::{ConfigError, Loader, Mount, Schema, SchemaError};

const FRAGMENT_V2: &str = r#"
[fragment]
name = "webhost"
version = "2.0.0"
summary = "web host options"
schema_version = 1
config_version = 2

[[value]]
key = "listen.port"
type = "int"
default = 8080
description = "listen port"

[[value]]
key = "tls.token"
type = "string"
secret = true
env = "TOKEN"
description = "certificate api token"

[[migration]]
from = 1
to = 2
ops = [{ op = "rename", from = "port", to = "listen.port" }]
"#;

fn app(config_version: u32) -> String {
    format!(
        r#"
[project]
name = "app"
version = "0.1.0"
summary = "an app"
schema_version = 1
env_prefix = "APP_"
config_version = {config_version}

[[value]]
key = "name"
type = "string"
default = "demo"
description = "app name"
"#
    )
}

fn schema() -> Schema {
    Schema::parse_with(&app(1), &[Mount::new("webhost", FRAGMENT_V2)]).expect("mounts")
}

const OLD_FILE: &str = "# mine\nname = \"x\"\n\n[webhost]\nport = 9000 # the port\n";

#[test]
fn an_old_file_is_migrated_under_the_mount_and_then_loads() {
    let s = schema();
    let m = s.migrate(OLD_FILE, Some(1)).expect("migrates");
    assert!(m.text.contains("# the port"), "comment kept");
    assert!(m.changes.iter().any(|c| c.contains("webhost.port")));
    let t: toml::Table = m.text.parse().unwrap();
    let w = t["webhost"].as_table().unwrap();
    assert_eq!(w["config_version"].as_integer(), Some(2));
    assert_eq!(w["listen"]["port"].as_integer(), Some(9000));
    assert!(!w.contains_key("port"));
    let cfg = Loader::new(&s)
        .file("app.toml", &m.text)
        .build()
        .expect("loads");
    assert_eq!(cfg.get::<i64>("webhost.listen.port").unwrap(), 9000);
}

#[test]
fn a_file_already_at_the_mount_version_is_returned_unchanged() {
    let s = schema();
    let current = "[webhost]\nconfig_version = 2\n\n[webhost.listen]\nport = 7000\n";
    let m = s.migrate(current, Some(1)).expect("nothing to do");
    assert_eq!(m.text, current);
    assert!(m.changes.is_empty());
}

#[test]
fn a_file_without_the_mount_table_gains_nothing() {
    let s = schema();
    let m = s.migrate("name = \"x\"\n", Some(1)).expect("nothing to do");
    assert_eq!(m.text, "name = \"x\"\n");
}

#[test]
fn a_mount_migrated_file_carries_back_to_what_it_was() {
    let s = schema();
    let up = s.migrate(OLD_FILE, Some(1)).unwrap();
    let back = s.migrate_mount_back(&up.text, "webhost", 1).expect("back");
    let a: toml::Table = back.text.parse().unwrap();
    let b: toml::Table = OLD_FILE.parse().unwrap();
    let mut a = a;
    a["webhost"]
        .as_table_mut()
        .unwrap()
        .remove("config_version");
    assert_eq!(a, b);
}

#[test]
fn the_loader_names_an_outdated_or_newer_mount_and_never_ignores_it() {
    let s = schema();
    let old = "[webhost]\nconfig_version = 1\nport = 9000\n";
    let e = Loader::new(&s)
        .file("app.toml", old)
        .build()
        .expect_err("outdated");
    assert!(
        e.0.iter()
            .any(|x| matches!(x, ConfigError::MountFileOutdated { mount, found: 1, current: 2, .. } if mount == "webhost")),
        "{e:?}"
    );
    let newer = "[webhost]\nconfig_version = 3\n";
    let e = Loader::new(&s)
        .file("app.toml", newer)
        .build()
        .expect_err("newer");
    assert!(
        e.0.iter()
            .any(|x| matches!(x, ConfigError::MountFileNewer { mount, found: 3, current: 2, .. } if mount == "webhost")),
        "{e:?}"
    );
}

#[test]
fn a_current_mount_marker_is_not_an_unknown_key() {
    let s = schema();
    let ok = "[webhost]\nconfig_version = 2\n";
    Loader::new(&s).file("app.toml", ok).build().expect("loads");
}

#[test]
fn a_fragment_migration_chain_must_be_complete_and_reversible() {
    // version 3 declared, one step given
    let gap = FRAGMENT_V2.replace("config_version = 2", "config_version = 3");
    let e = Schema::parse_with(&app(1), &[Mount::new("webhost", &gap)]).expect_err("gap");
    assert!(matches!(e, SchemaError::FragmentParse { .. }), "{e:?}");
    // a map that is not one-to-one cannot be carried back
    let bad = FRAGMENT_V2.replace(
        r#"{ op = "rename", from = "port", to = "listen.port" }"#,
        r#"{ op = "map_value", key = "mode", map = { a = "x", b = "x" } }"#,
    );
    let e = Schema::parse_with(&app(1), &[Mount::new("webhost", &bad)]).expect_err("bad map");
    assert!(matches!(e, SchemaError::FragmentParse { .. }), "{e:?}");
}

#[test]
fn a_migrated_secret_never_reaches_a_change_line_or_an_error() {
    let s = schema();
    let file = "[webhost]\nport = 9000\n\n[webhost.tls]\ntoken = \"hunter2-secret\"\n";
    let m = s.migrate(file, Some(1)).unwrap();
    assert!(m.changes.iter().all(|c| !c.contains("hunter2")));
    // a destination collision is an error naming keys only
    let clash = "[webhost]\nport = 1\n\n[webhost.listen]\nport = 2\ntoken = \"hunter2-secret\"\n";
    let e = s.migrate(clash, Some(1)).expect_err("collision");
    assert!(!e.to_string().contains("hunter2"));
}
