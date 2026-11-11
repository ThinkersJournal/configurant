// SPDX-License-Identifier: MIT OR Apache-2.0
//! The editor owns the format version of the files it creates (PM ruling 2026-11-10, option A): a new file gets
//! the application's `config_version` and each mounted library's, so a later migration knows where it stands.
//! A file that already exists is never stamped: the editor cannot know which version an unmarked file is at.

use configurant::edit::Session;
use configurant::{Loader, Mount, Schema};

const FRAGMENT: &str = r#"
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

[[migration]]
from = 1
to = 2
ops = [{ op = "rename", from = "port", to = "listen.port" }]
"#;

const APP: &str = r#"
[project]
name = "app"
version = "0.1.0"
summary = "an app"
schema_version = 1
env_prefix = "APP_"
config_version = 3

[[value]]
key = "name"
type = "string"
default = "demo"
description = "app name"

[[migration]]
from = 1
to = 2

[[migration]]
from = 2
to = 3
"#;

fn schema() -> Schema {
    Schema::parse_with(APP, &[Mount::new("webhost", FRAGMENT)]).expect("mounts")
}

/// `existing` edited so that `key` is `value`.
fn edited(existing: &str, key: &str, value: &str) -> String {
    let s = schema();
    let cfg = Loader::new(&s)
        .file("app.toml", existing)
        .build()
        .expect("loads");
    let mut session = Session::new(&cfg, None);
    session.stage(key, value).expect("stages");
    session
        .review()
        .confirm()
        .file_text(existing)
        .expect("text")
}

fn parsed(text: &str) -> toml::Table {
    text.parse().expect("valid toml")
}

#[test]
fn a_new_file_gets_the_application_and_the_mount_markers_and_loads_back() {
    let text = edited("", "webhost.listen.port", "9001");
    let t = parsed(&text);
    assert_eq!(t["config_version"].as_integer(), Some(3), "{text}");
    assert_eq!(
        t["webhost"]["config_version"].as_integer(),
        Some(2),
        "{text}"
    );
    let s = schema();
    let cfg = Loader::new(&s)
        .file("app.toml", &text)
        .build()
        .expect("the editor's file loads");
    assert_eq!(cfg.get::<i64>("webhost.listen.port").unwrap(), 9001);
}

#[test]
fn a_new_file_that_touches_no_mount_gets_no_mount_marker() {
    let text = edited("", "name", "mine");
    let t = parsed(&text);
    assert_eq!(t["config_version"].as_integer(), Some(3), "{text}");
    assert!(!t.contains_key("webhost"), "{text}");
}

#[test]
fn an_existing_unmarked_file_is_not_stamped() {
    // It might be at any version; stamping it current would hide a migration it still needs.
    let existing = "# mine\nname = \"x\"\n\n[webhost.listen]\nport = 1 # keep\n";
    let text = edited(existing, "webhost.listen.port", "2");
    let t = parsed(&text);
    assert!(!t.contains_key("config_version"), "{text}");
    assert!(
        !t["webhost"]
            .as_table()
            .unwrap()
            .contains_key("config_version"),
        "{text}"
    );
    assert!(text.contains("# mine"), "{text}");
}

#[test]
fn an_existing_marker_is_never_changed() {
    let existing =
        "config_version = 3\n\n[webhost]\nconfig_version = 2\n\n[webhost.listen]\nport = 1\n";
    let text = edited(existing, "webhost.listen.port", "2");
    let t = parsed(&text);
    assert_eq!(t["config_version"].as_integer(), Some(3), "{text}");
    assert_eq!(
        t["webhost"]["config_version"].as_integer(),
        Some(2),
        "{text}"
    );
}

#[test]
fn a_mount_table_the_editor_creates_in_an_existing_file_is_marked_but_the_file_is_not() {
    let existing = "# mine\nname = \"x\"\n";
    let text = edited(existing, "webhost.listen.port", "9");
    let t = parsed(&text);
    assert_eq!(
        t["webhost"]["config_version"].as_integer(),
        Some(2),
        "{text}"
    );
    assert!(!t.contains_key("config_version"), "{text}");
}

#[test]
fn nothing_confirmed_writes_nothing_new() {
    let s = schema();
    let cfg = Loader::new(&s).build().expect("loads");
    let session = Session::new(&cfg, None);
    assert_eq!(session.review().confirm().file_text("").unwrap(), "");
}
