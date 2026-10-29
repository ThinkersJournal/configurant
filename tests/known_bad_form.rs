// SPDX-License-Identifier: MIT OR Apache-2.0
//! Form projection: the view groups values into sections (the key's first segment, or the schema's own
//! `section`), keeps schema order, marks `advanced` values, and lists the file-only tables and maps so a
//! form can say where they are edited. It carries keys, never values, in the grouping.

use std::collections::BTreeMap;

use configurant::edit::Session;
use configurant::view::Driver;
use configurant::{Loader, Needs, Schema, SchemaError};

const CANARY: &str = "canary-s3cr3t-value";

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1
env_prefix = "DEMO_"

[[value]]
key = "server.port"
type = "int"
default = 8080
description = "listen port"

[[value]]
key = "name"
type = "string"
default = "demo"
description = "app name"

[[value]]
key = "log.level"
type = "string"
default = "info"
section = "Logging"
description = "how much to log"

[[value]]
key = "server.workers"
type = "int"
default = 4
advanced = true
description = "worker threads"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "DEMO_API_TOKEN"
description = "api token"

[[table]]
key = "mount"
description = "mounted stores"

[[table.field]]
key = "path"
type = "path"
description = "where"

[[map]]
key = "job.env"
value_type = "string"
description = "job environment"
"#;

fn view() -> configurant::view::View {
    let schema = Schema::parse(SCHEMA).expect("valid");
    let env = BTreeMap::from([("DEMO_API_TOKEN".to_owned(), CANARY.to_owned())]);
    let cfg = Loader::new(&schema).env(env).build().expect("loads");
    Driver::new(Session::new(&cfg, None), Needs::default()).view()
}

#[test]
fn sections_follow_first_appearance_and_group_by_key_prefix_or_the_schemas_own() {
    let v = view();
    let titles: Vec<&str> = v.sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["server", "General", "Logging", "api"]);
    let server = &v.sections[0];
    assert_eq!(server.keys, ["server.port", "server.workers"]);
    assert_eq!(v.sections[1].keys, ["name"]);
    assert_eq!(v.sections[2].keys, ["log.level"]);
}

#[test]
fn every_field_is_in_exactly_one_section_and_names_it() {
    let v = view();
    for f in &v.fields {
        let homes: Vec<_> = v
            .sections
            .iter()
            .filter(|s| s.keys.contains(&f.key))
            .collect();
        assert_eq!(homes.len(), 1, "{}", f.key);
        assert_eq!(homes[0].title, f.section, "{}", f.key);
    }
}

#[test]
fn advanced_values_are_marked() {
    let v = view();
    let adv: Vec<&str> = v
        .fields
        .iter()
        .filter(|f| f.advanced)
        .map(|f| f.key.as_str())
        .collect();
    assert_eq!(adv, ["server.workers"]);
}

#[test]
fn file_only_tables_and_maps_are_listed_with_where_to_edit_them() {
    let v = view();
    let kinds: Vec<(&str, &str)> = v
        .file_only
        .iter()
        .map(|f| (f.key.as_str(), f.kind))
        .collect();
    assert_eq!(kinds, [("mount", "table"), ("job.env", "map")]);
    assert_eq!(v.file_only[0].description, "mounted stores");
}

#[test]
fn the_grouping_never_carries_a_value() {
    let v = serde_json::to_string(&view().sections).expect("serialises");
    assert!(
        !v.contains(CANARY) && !v.contains("8080") && !v.contains("<redacted>"),
        "{v}"
    );
}

#[test]
fn an_empty_section_is_refused() {
    let text = SCHEMA.replace(r#"section = "Logging""#, r#"section = "  ""#);
    let e = Schema::parse(&text).expect_err("refused");
    assert!(matches!(e, SchemaError::EmptySection(_)), "{e:?}");
}
