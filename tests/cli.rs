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

#[test]
fn migrate_prints_by_default_and_writes_only_when_told_keeping_a_backup() {
    let dir = std::env::temp_dir().join(format!("configurant-migrate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let schema = dir.join("schema.toml");
    let file = dir.join("app.toml");
    let old = "# mine\n[server]\nlisten = 9000\n";
    std::fs::write(
        &schema,
        std::fs::read_to_string(SCHEMA).unwrap().replace(
            "env_prefix = \"DEMO_\"",
            "env_prefix = \"DEMO_\"\nconfig_version = 2",
        ) + "\n[[migration]]\nfrom = 1\nto = 2\nops = [{ op = \"rename\", from = \"server.listen\", to = \"server.port\" }]\n",
    )
    .unwrap();
    std::fs::write(&file, old).unwrap();
    let (s, f) = (schema.to_str().unwrap(), file.to_str().unwrap());

    let o = run(&["migrate", s, f, "--from", "1"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let t = text(&o);
    assert!(t.contains("port = 9000") && t.contains("# mine"), "{t}");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        old,
        "printing must not write"
    );

    let o = run(&["migrate", s, f, "--from", "1", "--write"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let new = std::fs::read_to_string(&file).unwrap();
    assert!(
        new.contains("port = 9000") && new.contains("config_version = 2"),
        "{new}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("app.toml.bak")).unwrap(),
        old
    );

    // Without a version in the file or `--from`, it is refused with a named error.
    std::fs::write(&file, old).unwrap();
    let o = run(&["migrate", s, f], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o).contains("--from"), "{}", text(&o));
    std::fs::remove_dir_all(&dir).ok();
}
