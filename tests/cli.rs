// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `configurant` binary, run for real against the example schema.

use std::process::{Command, Output};

const SCHEMA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/configurant.toml");
const SECRET: &str = "s3cr3t-value";

fn run(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_configurant"));
    cmd.args(args);
    // A clean environment apart from what the test sets: the test must not depend on the machine's.
    cmd.env_clear();
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.envs(envs.iter().copied());
    cmd.output().expect("the binary runs")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn check_and_describe_work_and_never_show_a_secret() {
    let o = run(&["check", SCHEMA], &[("DEMO_DB_PASSWORD", SECRET)]);
    assert!(o.status.success(), "{}", text(&o));
    let o = run(&["describe", SCHEMA], &[("DEMO_DB_PASSWORD", SECRET)]);
    assert!(o.status.success());
    let t = text(&o);
    assert!(t.contains("--port") && t.contains("[secret]") && !t.contains(SECRET));
}

#[test]
fn explain_shows_the_layers_and_redacts_the_secret() {
    let o = run(
        &["explain", SCHEMA, "server.port", "--", "--port", "9100"],
        &[("DEMO_PORT", "9000")],
    );
    assert!(o.status.success(), "{}", text(&o));
    let t = text(&o);
    assert!(
        t.contains("8080") && t.contains("9000") && t.contains("9100"),
        "{t}"
    );
    assert!(
        t.contains("9100") && t.rfind("9100") > t.rfind("9000"),
        "cli listed after env: {t}"
    );

    let o = run(
        &["explain", SCHEMA, "db.password"],
        &[("DEMO_DB_PASSWORD", SECRET)],
    );
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("<redacted>") && !text(&o).contains(SECRET));
}

#[test]
fn bad_input_exits_2_with_a_named_error_and_no_secret() {
    let o = run(&["explain", SCHEMA, "server.port"], &[("DEMO_PROT", "1")]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        text(&o).contains("did you mean `DEMO_PORT`"),
        "{}",
        text(&o)
    );

    let flag = format!("--db.password={SECRET}");
    let o = run(&["explain", SCHEMA, "server.port", "--", &flag], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(!text(&o).contains(SECRET), "{}", text(&o));

    let o = run(&["check", "no-such-file.toml"], &[]);
    assert_eq!(o.status.code(), Some(2));
}
