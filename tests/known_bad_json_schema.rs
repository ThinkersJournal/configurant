// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Schema::json_schema` and `configurant json-schema` (ROADMAP row 13). Known-bad first: a secret in the
//! export, a description that breaks the JSON, a required key that does not reach its parent, a dotted key
//! that does not nest, a key the schema does not declare.

use std::process::Command;

use configurant::Schema;
use serde_json::Value;

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "An \"example\" schema"
schema_version = 1

[[value]]
key = "server.port"
type = "int"
default = 8080
min = 1
max = 65535
description = "listen port"

[[value]]
key = "server.mode"
type = "enum"
choices = ["fast", "safe"]
default = "safe"
description = "line one\nline two with a \"quote\", a \\ and a tab\t."

[[value]]
key = "db.url"
type = "string"
required = true
description = "where the database is"

[[value]]
key = "db.password"
type = "string"
secret = true
description = "TOPSECRET-DESCRIPTION"

[[value]]
key = "tags"
type = "list"
description = "labels"

[[value]]
key = "ratio"
type = "float"
min = 0.5
max = 2.5
default = 1.0
description = "a ratio"

[[table]]
key = "mount"
description = "mounts"

[[table.field]]
key = "path"
type = "path"
required = true
description = "where"

[[map]]
key = "job.env"
value_type = "string"
description = "environment for the job"
"#;

fn export() -> Value {
    let s = Schema::parse(SCHEMA).expect("schema parses");
    serde_json::from_str(&s.json_schema()).expect("the export is valid JSON")
}

#[test]
fn the_export_is_valid_json_and_names_the_draft_and_the_project() {
    let v = export();
    assert!(
        v["$schema"] == "https://json-schema.org/draft/2020-12/schema",
        "case 1: draft"
    );
    assert!(v["title"] == "demo 0.1.0", "case 1: title");
    assert!(
        v["description"] == "An \"example\" schema",
        "case 1: summary"
    );
}

#[test]
fn a_dotted_key_nests_and_carries_its_type_bounds_default_and_description() {
    let v = export();
    let port = &v["properties"]["server"]["properties"]["port"];
    assert!(port["type"] == "integer", "case 2: type");
    assert!(
        port["minimum"] == 1 && port["maximum"] == 65535,
        "case 2: bounds"
    );
    assert!(port["default"] == 8080, "case 2: default");
    assert!(port["description"] == "listen port", "case 2: description");
    let ratio = &v["properties"]["ratio"];
    assert!(
        ratio["type"] == "number" && ratio["minimum"] == 0.5 && ratio["maximum"] == 2.5,
        "case 2: float bounds"
    );
}

#[test]
fn an_enum_lists_its_choices_and_a_list_holds_strings() {
    let v = export();
    let mode = &v["properties"]["server"]["properties"]["mode"];
    assert!(
        mode["enum"] == serde_json::json!(["fast", "safe"]),
        "case 3: enum"
    );
    let tags = &v["properties"]["tags"];
    assert!(
        tags["type"] == "array" && tags["items"]["type"] == "string",
        "case 3: list"
    );
}

#[test]
fn a_secret_is_in_the_export_nowhere() {
    let s = Schema::parse(SCHEMA).unwrap();
    let text = s.json_schema();
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(
        v["properties"]["db"]["properties"]
            .get("password")
            .is_none(),
        "case 4: the secret is a property"
    );
    assert!(
        !text.contains("TOPSECRET-DESCRIPTION") && !text.contains("password"),
        "case 4: the secret's text reached the export"
    );
}

#[test]
fn every_object_is_closed_so_an_unknown_key_is_flagged() {
    let v = export();
    assert!(v["additionalProperties"] == false, "case 5: root");
    assert!(
        v["properties"]["server"]["additionalProperties"] == false,
        "case 5: nested"
    );
    assert!(
        v["properties"]["mount"]["items"]["additionalProperties"] == false,
        "case 5: table row"
    );
}

#[test]
fn a_required_key_makes_each_parent_required_too() {
    let v = export();
    assert!(
        v["properties"]["db"]["required"] == serde_json::json!(["url"]),
        "case 6: db requires url"
    );
    assert!(
        v["required"] == serde_json::json!(["db"]),
        "case 6: the root requires db"
    );
    assert!(
        v["properties"]["server"].get("required").is_none(),
        "case 6: nothing required under server"
    );
}

#[test]
fn a_table_is_an_array_of_closed_objects_and_a_map_has_one_value_type() {
    let v = export();
    let mount = &v["properties"]["mount"];
    assert!(mount["type"] == "array", "case 7: table type");
    assert!(
        mount["items"]["properties"]["path"]["type"] == "string"
            && mount["items"]["required"] == serde_json::json!(["path"]),
        "case 7: table field"
    );
    let env = &v["properties"]["job"]["properties"]["env"];
    assert!(
        env["type"] == "object" && env["additionalProperties"]["type"] == "string",
        "case 7: map"
    );
}

#[test]
fn the_loaders_own_config_version_is_allowed() {
    let v = export();
    let c = &v["properties"]["config_version"];
    assert!(c["type"] == "integer" && c["minimum"] == 1, "case 8");
}

#[test]
fn a_description_with_a_quote_a_newline_a_backslash_and_a_tab_survives() {
    let v = export();
    let d = &v["properties"]["server"]["properties"]["mode"]["description"];
    assert!(
        d == "line one\nline two with a \"quote\", a \\ and a tab\t.",
        "case 9"
    );
}

#[test]
fn every_exported_property_is_a_key_the_schema_declares_and_every_plain_key_is_exported() {
    let s = Schema::parse(SCHEMA).unwrap();
    let v: Value = serde_json::from_str(&s.json_schema()).unwrap();
    let mut found = Vec::new();
    fn walk(prefix: &str, node: &Value, out: &mut Vec<String>) {
        let Some(props) = node.get("properties").and_then(Value::as_object) else {
            return;
        };
        for (k, child) in props {
            let key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            if child.get("properties").is_some() {
                walk(&key, child, out);
            } else {
                out.push(key);
            }
        }
    }
    walk("", &v, &mut found);
    found.retain(|k| k != "config_version");
    found.sort();
    let mut expected: Vec<String> = s
        .all_keys()
        .filter(|k| *k != "db.password")
        .map(str::to_owned)
        .collect();
    expected.sort();
    assert!(found == expected, "case 10: {found:?} vs {expected:?}");
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_configurant"))
        .args(args)
        .output()
        .expect("the binary runs")
}

#[test]
fn the_command_prints_the_same_json_and_a_bad_schema_exits_two() {
    let schema = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/configurant.toml");
    let o = run_cli(&["json-schema", schema]);
    assert!(o.status.success(), "case 11: exit");
    let expected = Schema::parse(&std::fs::read_to_string(schema).unwrap())
        .unwrap()
        .json_schema();
    assert!(
        String::from_utf8_lossy(&o.stdout) == expected,
        "case 11: output"
    );
    assert!(
        !String::from_utf8_lossy(&o.stdout).contains("DEMO_DB_PASSWORD"),
        "case 11: the secret's environment name reached the export"
    );

    let o = run_cli(&["json-schema", "no-such-file.toml"]);
    assert_eq!(o.status.code(), Some(2), "case 12");
}
