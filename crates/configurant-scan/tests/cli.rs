// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `configurant-scan` binary's refusals; the run against a real server is in `real_rust_analyzer.rs`.

use std::process::Command;

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_configurant-scan"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn schema_file(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("cfg-scan-cli-{name}-{}.toml", std::process::id()));
    std::fs::write(
        &p,
        "[project]\nname = \"t\"\nversion = \"1.0.0\"\nschema_version = 1\n",
    )
    .unwrap();
    p
}

#[test]
fn a_missing_schema_is_an_error() {
    let (code, err) = run(&["no-such-schema.toml"]);
    assert_eq!(code, 1);
    assert!(err.contains("no-such-schema.toml"));
}

#[test]
fn an_absent_language_server_is_an_error_naming_it_not_an_empty_result() {
    let s = schema_file("absent");
    let (code, err) = run(&[
        s.to_str().unwrap(),
        "--root",
        ".",
        "--server",
        "no-such-language-server",
        "--wait",
        "5",
    ]);
    let _ = std::fs::remove_file(&s);
    assert_eq!(code, 1);
    assert!(err.contains("no-such-language-server"));
}

#[test]
fn write_without_accept_is_refused() {
    let s = schema_file("write");
    let (code, _) = run(&[s.to_str().unwrap(), "--write"]);
    let _ = std::fs::remove_file(&s);
    assert_ne!(code, 0);
}
