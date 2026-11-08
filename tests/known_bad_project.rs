// SPDX-License-Identifier: MIT OR Apache-2.0
//! Projection of resolved values into a host format (ROADMAP row 33). Known-bad first: a secret's value in
//! any format, a value that breaks out of its quotes, a list that does not read back, a `$` that Compose
//! would interpolate, a value dropped without being named.

use std::collections::BTreeMap;
use std::process::Command;

use configurant::{HostFormat, Loader, Schema};

const CANARY: &str = "hunter2-canary";

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "demo"
schema_version = 1

[[value]]
key = "server.port"
type = "int"
default = 8080
env = "APP_PORT"
description = "listen port"

[[value]]
key = "server.name"
type = "string"
env = "APP_NAME"
description = "a name"

[[value]]
key = "tags"
type = "list"
env = "APP_TAGS"
description = "labels"

[[value]]
key = "debug"
type = "bool"
default = false
env = "APP_DEBUG"
description = "debug"

[[value]]
key = "db.password"
type = "string"
secret = true
env = "APP_DB_PASSWORD"
source = "vault:APP_DB_PASSWORD"
description = "TOPSECRET-DESCRIPTION"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "APP_TOKEN"
description = "a token"

[[value]]
key = "no.env"
type = "string"
default = "x"
description = "has no env name"

[[value]]
key = "unset.one"
type = "string"
env = "APP_UNSET"
description = "never set"
"#;

const FILE: &str = r#"
config_version = 1
tags = ["a,b", "c\\d"]
[server]
name = "say \"hi\" $HOME\nline2"
"#;

fn project(secret_in_env: bool) -> configurant::Projection {
    let schema = Schema::parse(SCHEMA).unwrap();
    let mut env = BTreeMap::new();
    if secret_in_env {
        env.insert("APP_DB_PASSWORD".to_owned(), CANARY.to_owned());
        env.insert("APP_TOKEN".to_owned(), CANARY.to_owned());
    }
    let cfg = Loader::new(&schema)
        .env(env)
        .file("c.toml", FILE)
        .build()
        .unwrap();
    cfg.project()
}

fn all_formats(p: &configurant::Projection) -> Vec<String> {
    [HostFormat::Env, HostFormat::Wrangler, HostFormat::Compose]
        .into_iter()
        .map(|f| p.render(f))
        .collect()
}

#[test]
fn case_1_a_secret_value_reaches_no_format_and_its_description_neither() {
    let p = project(true);
    let leaked = all_formats(&p)
        .iter()
        .any(|t| t.contains(CANARY) || t.contains("TOPSECRET"));
    assert!(!leaked);
    let names = all_formats(&p)
        .iter()
        .all(|t| t.contains("APP_DB_PASSWORD"));
    assert!(names, "the secret's variable is named in every format");
}

#[test]
fn case_2_a_secret_is_a_reference_and_the_source_is_shown() {
    let p = project(true);
    assert_eq!(p.secrets.len(), 2);
    assert_eq!(
        p.secrets[0].source.as_deref(),
        Some("vault:APP_DB_PASSWORD")
    );
    assert_eq!(p.secrets[1].source, None);
    let compose = p.render(HostFormat::Compose);
    assert!(compose.contains("APP_DB_PASSWORD: ${APP_DB_PASSWORD:?"));
    assert!(compose.contains("# source vault:APP_DB_PASSWORD"));
    let wr = p.render(HostFormat::Wrangler);
    assert!(wr.contains("wrangler secret put APP_TOKEN"));
}

#[test]
fn case_3_the_vars_are_the_resolved_values() {
    let p = project(false);
    let names: Vec<&str> = p.vars.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["APP_PORT", "APP_NAME", "APP_TAGS", "APP_DEBUG"]);
    assert_eq!(p.vars[0].value, "8080");
    assert_eq!(p.vars[3].value, "false");
}

#[test]
fn case_4_a_list_is_written_in_the_loaders_escaping() {
    let p = project(false);
    assert_eq!(p.vars[2].value, "a\\,b,c\\\\d");
    // The loader reads it back as the same list.
    let schema = Schema::parse(SCHEMA).unwrap();
    let mut env = BTreeMap::new();
    env.insert("APP_TAGS".to_owned(), p.vars[2].value.clone());
    let cfg = Loader::new(&schema).env(env).build().unwrap();
    let back: Vec<String> = cfg.get("tags").unwrap();
    assert_eq!(back, ["a,b", "c\\d"]);
}

#[test]
fn case_5_quotes_and_newlines_stay_inside_their_string() {
    let p = project(false);
    let env = p.render(HostFormat::Env);
    assert!(env.contains("APP_NAME=\"say \\\"hi\\\" $HOME\\nline2\"\n"));
    let wr = p.render(HostFormat::Wrangler);
    let table: toml::Table = wr.parse().unwrap();
    let name = table["vars"]["APP_NAME"].as_str().unwrap();
    assert_eq!(name, "say \"hi\" $HOME\nline2");
}

#[test]
fn case_6_compose_doubles_a_dollar() {
    let p = project(false);
    let compose = p.render(HostFormat::Compose);
    assert!(compose.contains("$$HOME"));
    assert!(!compose.contains(" $HOME"));
}

#[test]
fn case_7_what_is_left_out_is_named_with_a_reason_and_no_value() {
    let p = project(false);
    let keys: Vec<&str> = p.skipped.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, ["no.env", "unset.one"]);
    let env = p.render(HostFormat::Env);
    assert!(env.contains("# skipped no.env: no `env` name in the schema"));
    assert!(env.contains("# skipped unset.one: not set"));
}

#[test]
fn case_8_the_cli_prints_a_format_and_refuses_an_unknown_one() {
    let dir = std::env::temp_dir().join("configurant-project-case8");
    std::fs::create_dir_all(&dir).unwrap();
    let schema = dir.join("s.toml");
    std::fs::write(&schema, SCHEMA).unwrap();
    let file = dir.join("c.toml");
    std::fs::write(&file, FILE).unwrap();
    let exe = env!("CARGO_BIN_EXE_configurant");
    let ok = Command::new(exe)
        .args(["project"])
        .arg(&schema)
        .args(["--format", "wrangler", "--file"])
        .arg(&file)
        .env("APP_DB_PASSWORD", CANARY)
        .output()
        .unwrap();
    assert!(ok.status.success());
    let out = String::from_utf8_lossy(&ok.stdout);
    assert!(out.starts_with("[vars]\n"));
    assert!(!out.contains(CANARY));
    // The process environment is not layered in unless asked: a stray APP_PORT does not leak in.
    let stray = Command::new(exe)
        .args(["project"])
        .arg(&schema)
        .args(["--format", "env"])
        .env("APP_PORT", "9999")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&stray.stdout).contains("APP_PORT=\"8080\""));
    let bad = Command::new(exe)
        .args(["project"])
        .arg(&schema)
        .args(["--format", "yaml"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("expected env, wrangler or compose"));
}
