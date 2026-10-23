// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad inputs for scan assist, against a scripted resolver (no language server): each rule starts here
//! as a failing test. The real server is in `real_rust_analyzer.rs`.

use std::path::{Path, PathBuf};

use configurant::Schema;
use configurant::schema_edit::add_value;
use configurant_scan::{Resolver, ScanError, scan};

const STD_ENV: &str = "/home/u/.rustup/toolchains/t/lib/rustlib/src/rust/library/std/src/env.rs";

/// Answers every query with one fixed reply.
struct Fixed(Result<Option<String>, ScanError>);

impl Resolver for Fixed {
    fn definition(
        &mut self,
        _: &Path,
        _: &str,
        _: u32,
        _: u32,
    ) -> Result<Option<String>, ScanError> {
        self.0.clone()
    }
}

fn std_env() -> Fixed {
    Fixed(Ok(Some(STD_ENV.to_owned())))
}

struct Tree(PathBuf);

impl Tree {
    fn new(name: &str, files: &[(&str, &str)]) -> Tree {
        let dir = std::env::temp_dir().join(format!("cfg-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, text) in files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        Tree(dir)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const SCHEMA: &str = r#"
[project]
name = "t"
version = "1.0.0"
schema_version = 1

[[value]]
key = "server.port"
type = "int"
default = 8080
description = "listen port"
env = "T_PORT"
"#;

fn schema() -> Schema {
    Schema::parse(SCHEMA).unwrap()
}

#[test]
fn a_literal_read_is_proposed_with_its_site_and_default() {
    let t = Tree::new(
        "literal",
        &[(
            "src/main.rs",
            "fn main() {\n    let h = std::env::var(\"APP_HOST\").unwrap_or(\"localhost\".into());\n}\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    assert!(!p.is_partial());
    assert_eq!(p.proposals.len(), 1, "{p:?}");
    let one = &p.proposals[0];
    assert_eq!(
        (one.env.as_str(), one.key.as_str()),
        ("APP_HOST", "app.host")
    );
    assert_eq!(one.default_seen.as_deref(), Some("localhost"));
    assert_eq!(one.sites[0].line, 2);
}

#[test]
fn a_dynamic_name_is_reported_by_location_not_guessed() {
    let t = Tree::new(
        "dynamic",
        &[(
            "src/main.rs",
            "fn f(n: &str) {\n    let _ = std::env::var(n);\n}\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    assert!(p.proposals.is_empty());
    assert_eq!(p.dynamic.len(), 1);
    assert_eq!(p.dynamic[0].line, 2);
}

#[test]
fn a_secret_name_never_carries_the_default_seen_beside_it() {
    let t = Tree::new(
        "secret",
        &[(
            "src/main.rs",
            "fn f() {\n    let _ = std::env::var(\"DB_PASSWORD\").unwrap_or(\"hunter2-canary\".into());\n}\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    assert_eq!(p.proposals.len(), 1);
    assert!(p.proposals[0].secret && p.proposals[0].default_seen.is_none());
    let nv = p.proposals[0].to_new_value();
    let text = add_value(SCHEMA, &nv).unwrap();
    for shown in [format!("{p:?}"), format!("{nv:?}"), text.clone()] {
        assert!(!shown.contains("hunter2-canary"), "default leaked");
    }
    let s = Schema::parse(&text).unwrap();
    let d = s.get("db.password").unwrap();
    assert!(d.secret && d.default.is_none() && d.flag.is_none());
}

#[test]
fn a_variable_the_schema_declares_is_not_proposed() {
    let t = Tree::new(
        "known",
        &[(
            "src/main.rs",
            "fn f() { let _ = std::env::var(\"T_PORT\"); }\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    assert!(p.proposals.is_empty());
    assert_eq!(p.known, 1);
}

#[test]
fn a_key_clash_with_another_env_is_listed_for_a_human() {
    // SERVER_PORT would be guessed as server.port, which the schema gives to T_PORT.
    let t = Tree::new(
        "clash",
        &[(
            "src/main.rs",
            "fn f() { let _ = std::env::var(\"SERVER_PORT\"); }\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    assert!(p.proposals.is_empty());
    assert_eq!(p.conflicts, vec!["SERVER_PORT".to_owned()]);
}

#[test]
fn a_function_of_the_projects_own_is_rejected_not_proposed() {
    let t = Tree::new(
        "own",
        &[("src/main.rs", "fn f() { let _ = env::var(\"X_Y\"); }\n")],
    );
    let mut own = Fixed(Ok(Some("/work/proj/src/env.rs".into())));
    let p = scan(&t.0, &schema(), &mut own).unwrap();
    assert!(p.proposals.is_empty());
    assert_eq!(p.rejected, 1);
    assert!(!p.is_partial());
}

#[test]
fn a_call_the_server_cannot_resolve_makes_the_scan_partial() {
    let t = Tree::new(
        "unresolved",
        &[(
            "src/main.rs",
            "fn f() { let _ = std::env::var(\"X_Y\"); }\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut Fixed(Ok(None))).unwrap();
    assert!(p.proposals.is_empty());
    assert_eq!(p.unresolved, 1);
    assert!(p.is_partial(), "an empty list must not look complete");
}

#[test]
fn a_server_error_stops_the_scan_naming_the_server() {
    let t = Tree::new(
        "dead",
        &[(
            "src/main.rs",
            "fn f() { let _ = std::env::var(\"X_Y\"); }\n",
        )],
    );
    let dead = ScanError::Server {
        command: "rust-analyzer".into(),
        reason: "exited".into(),
    };
    let e = scan(&t.0, &schema(), &mut Fixed(Err(dead))).unwrap_err();
    assert!(e.to_string().contains("rust-analyzer"));
}

#[test]
fn macros_are_read_without_the_server_and_option_env_is_not_counted_twice() {
    let t = Tree::new(
        "macros",
        &[(
            "src/main.rs",
            "const A: &str = env!(\"BUILD_ID\");\nconst B: Option<&str> = option_env!(\"BUILD_REV\");\n",
        )],
    );
    // A resolver that would reject everything proves the macros never reach it.
    let mut reject = Fixed(Ok(Some("/elsewhere.rs".into())));
    let p = scan(&t.0, &schema(), &mut reject).unwrap();
    assert_eq!(p.candidates, 2, "{p:?}");
    let envs: Vec<_> = p.proposals.iter().map(|x| x.env.as_str()).collect();
    assert_eq!(envs, ["BUILD_ID", "BUILD_REV"]);
}

#[test]
fn a_commented_out_read_is_ignored() {
    let t = Tree::new(
        "comment",
        &[(
            "src/main.rs",
            "// let _ = std::env::var(\"OLD_ONE\");\nfn f() {}\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    assert_eq!(p.candidates, 0);
}

#[test]
fn integer_and_bool_defaults_become_typed_values() {
    let t = Tree::new(
        "typed",
        &[(
            "src/main.rs",
            "fn f() {\n let _ = std::env::var(\"JOB_RETRIES\").ok().unwrap_or(3);\n let _ = std::env::var(\"JOB_VERBOSE\").unwrap_or(false);\n}\n",
        )],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    let mut text = SCHEMA.to_owned();
    for pr in &p.proposals {
        text = add_value(&text, &pr.to_new_value()).unwrap();
    }
    let s = Schema::parse(&text).unwrap();
    assert_eq!(s.get("job.retries").unwrap().ty.name(), "int");
    assert_eq!(s.get("job.verbose").unwrap().ty.name(), "bool");
}

#[test]
fn a_target_directory_and_dot_directories_are_not_scanned() {
    let t = Tree::new(
        "skip",
        &[
            (
                "target/debug/x.rs",
                "fn f() { let _ = std::env::var(\"IN_TARGET\"); }\n",
            ),
            (
                ".git/y.rs",
                "fn f() { let _ = std::env::var(\"IN_GIT\"); }\n",
            ),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );
    let p = scan(&t.0, &schema(), &mut std_env()).unwrap();
    assert_eq!((p.files, p.candidates), (1, 0));
}
