// SPDX-License-Identifier: MIT OR Apache-2.0
//! The compile-time key check: code that asks for a key the schema does not define is found, with a
//! suggestion; keys the schema defines, comments, and calls that are not the library's are not.

use std::path::PathBuf;
use std::process::Command;

use configurant::Schema;
use configurant::keycheck::{check_dir, check_source};

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "server.port"
type = "int"
default = 8080
description = "listen port"

[[value]]
key = "storage.dirs"
type = "list"
description = "dirs"

[[value]]
key = "db.password"
type = "string"
secret = true
description = "database password"
"#;

fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("valid")
}

#[test]
fn an_unknown_key_is_found_with_its_place_and_a_suggestion() {
    let src = "fn f(c: &configurant::Config) {\n    let _ = c.get::<u16>(\"server.prot\");\n    let _ = c.get::<u16>(\"server.port\");\n}\n";
    let r = check_source(&schema(), "src/main.rs", src);
    assert_eq!(r.unknown.len(), 1, "{r:?}");
    let u = &r.unknown[0];
    assert_eq!(
        (u.file.as_str(), u.line, u.key.as_str()),
        ("src/main.rs", 2, "server.prot")
    );
    assert_eq!(u.suggestion.as_deref(), Some("server.port"));
    assert_eq!(r.checked, 2);
}

#[test]
fn every_asking_form_is_checked_and_nested_generics_do_not_hide_the_key() {
    let src = "let a = c.get::<Vec<String>>(\"storage.dir\");\nlet b = c.value(\"nonesuch.one\");\nlet d = c.rows(\"nonesuch.two\");\nlet e = c.explain(\"nonesuch.three\");\n";
    let r = check_source(&schema(), "a.rs", src);
    let keys: Vec<&str> = r.unknown.iter().map(|u| u.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "storage.dir",
            "nonesuch.one",
            "nonesuch.two",
            "nonesuch.three"
        ]
    );
}

#[test]
fn comments_other_apis_and_computed_names_are_not_reported() {
    let src = "// c.get::<u16>(\"server.prot\")\nlet m: HashMap<&str, u8> = HashMap::new();\nlet _ = m.get(\"server.prot\");\nlet _ = c.get::<u16>(name);\nlet _ = c.get::<u16>(&format!(\"server.{x}\"));\nlet _ = xget::<u16>(\"server.prot\");\n";
    let r = check_source(&schema(), "a.rs", src);
    assert!(r.unknown.is_empty(), "{r:?}");
    assert_eq!(r.dynamic, 2, "computed names are counted, never guessed");
}

#[test]
fn a_report_never_shows_a_secret_value_only_key_names() {
    let src = "let _ = c.get::<String>(\"db.passwrd\");\n";
    let r = check_source(&schema(), "a.rs", src);
    assert_eq!(r.unknown[0].suggestion.as_deref(), Some("db.password"));
    assert!(!r.render().contains("hunter2"));
    assert!(
        r.render()
            .contains("a.rs:1: unknown key `db.passwrd` (did you mean `db.password`?)")
    );
}

fn tree(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cfg-keycheck-{name}-{}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(
        root.join("src").join("main.rs"),
        "fn main() { let _ = c.get::<u16>(\"server.prot\"); }\n",
    )
    .unwrap();
    // Build output is never the project's own code.
    std::fs::write(
        root.join("target").join("gen.rs"),
        "fn g() { let _ = c.get::<u16>(\"also.wrong\"); }\n",
    )
    .unwrap();
    root
}

#[test]
fn a_directory_is_walked_without_target_and_the_binary_exits_1_on_an_unknown_key() {
    let root = tree("dir");
    let r = check_dir(&schema(), &root).unwrap();
    assert_eq!(r.unknown.len(), 1, "{r:?}");
    assert!(r.unknown[0].file.ends_with("main.rs"));

    let schema_path = root.join("configurant.toml");
    std::fs::write(&schema_path, SCHEMA).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_configurant"))
        .arg("keys")
        .arg(&schema_path)
        .arg(&root)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(1), "{said}");
    assert!(said.contains("unknown key `server.prot` (did you mean `server.port`?)"));

    // Fixed, it passes.
    std::fs::write(
        root.join("src").join("main.rs"),
        "fn main() { let _ = c.get::<u16>(\"server.port\"); }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_configurant"))
        .arg("keys")
        .arg(&schema_path)
        .arg(&root)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(&root);
}
