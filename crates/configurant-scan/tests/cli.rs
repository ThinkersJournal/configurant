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

/// The fake server built beside the test binary (`target/<profile>/examples/fake_lsp`).
fn fake_server() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.push("examples");
    p.push(format!("fake_lsp{}", std::env::consts::EXE_SUFFIX));
    assert!(
        p.exists(),
        "build the example first (cargo test builds it): {}",
        p.display()
    );
    p
}

fn project_reading_an_env_var(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("cfg-scan-proj-{name}-{}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    let _ = std::env::var(\"APP_HOST\");\n}\n",
    )
    .unwrap();
    root
}

fn run_with_fake(name: &str) -> (i32, String, String) {
    let s = schema_file(name);
    let root = project_reading_an_env_var(name);
    let out = Command::new(env!("CARGO_BIN_EXE_configurant-scan"))
        .args([
            s.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
            "--server",
            fake_server().to_str().unwrap(),
            "--wait",
            "3",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&s);
    let _ = std::fs::remove_dir_all(&root);
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_server_that_cannot_resolve_makes_the_scan_partial_with_exit_2() {
    let (code, out, err) = run_with_fake("partial");
    assert!(code == 2, "exit code was {code}; stderr: {err}");
    assert!(out.contains("PARTIAL"));
    // The unresolved read is not proposed as if it were known.
    assert!(!out.contains("APP_HOST ->"));
}

#[test]
fn server_args_reach_the_server() {
    // The same fake server that gives exit 2 above dies at start when handed `--die`: a dash-led value
    // given to `--server-arg` reaches it, and a server that cannot start is an error (1), not a partial scan.
    let s = schema_file("args");
    let root = project_reading_an_env_var("args");
    let out = Command::new(env!("CARGO_BIN_EXE_configurant-scan"))
        .args([
            s.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
            "--server",
            fake_server().to_str().unwrap(),
            "--server-arg",
            "--die",
            "--wait",
            "5",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&s);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(out.status.code(), Some(1));
}
