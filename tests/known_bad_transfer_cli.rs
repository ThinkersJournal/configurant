// SPDX-License-Identifier: MIT OR Apache-2.0
//! `configurant transfer` (ROADMAP row 6): the command-line way to run the data copy the TUI and GUI run.
//! Known-bad first: a typed URL, the same backend twice, an unknown backend, a missing endpoint, a source
//! that changes, a secret in the output.

#![cfg(feature = "persistant-blocking")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use configurant::persistant::Endpoint;
use persistant::capability::provider::ProviderNeeds;
use persistant::kv::KvBackend;
use persistant::registry;

const SECRET_URL: &str = "postgres://admin:hunter2-pw@db.internal/app";

fn run_cli(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_configurant"));
    cmd.args(args);
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

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(f)
}

fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "configurant-transfer-cli-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

/// An fs backend holding `n` records; returns its directory.
fn fs_with(tag: &str, n: usize) -> PathBuf {
    let dir = scratch_dir(tag);
    block(async {
        let gate = ProviderNeeds::new(Vec::new(), configurant::persistant::Durability::Volatile);
        let h = registry::open_named("fs", &Endpoint::path(&dir), &gate)
            .await
            .expect("open fs");
        for i in 0..n {
            h.kv()
                .put(&format!("rec/{i:04}"), vec![i as u8, 0, 255], None)
                .await
                .unwrap();
        }
    });
    dir
}

fn keys_of(dir: &Path) -> Vec<String> {
    block(async {
        let gate = ProviderNeeds::new(Vec::new(), configurant::persistant::Durability::Volatile);
        let h = registry::open_named("fs", &Endpoint::path(dir), &gate)
            .await
            .expect("open fs");
        h.kv().ordered_scan("", None, 10_000).await.unwrap().keys
    })
}

fn transfer_args<'a>(dir: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut a = vec![
        "transfer",
        "--from",
        "fs",
        "--to",
        "memory",
        "--min-durability",
        "volatile",
        "--endpoint",
        dir,
    ];
    a.extend_from_slice(extra);
    a
}

#[test]
fn a_transfer_copies_verifies_reports_and_leaves_the_source_alone() {
    let dir = fs_with("ok", 300);
    let before = keys_of(&dir);
    let ep = format!("fs={}", dir.display());
    let o = run_cli(&transfer_args(&ep, &[]), &[]);
    let after = keys_of(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let t = text(&o);
    assert!(o.status.success(), "case 1: {t}");
    assert!(t.contains("copied 300 records"), "case 1: {t}");
    assert!(t.contains("verified 300"), "case 1: {t}");
    assert!(
        t.contains("leases"),
        "case 1: the report says what is not copied: {t}"
    );
    assert!(
        before == after && before.len() == 300,
        "case 1: source changed"
    );
}

#[test]
fn a_typed_url_is_refused_and_never_echoed() {
    let o = run_cli(
        &[
            "transfer",
            "--from",
            "fs",
            "--to",
            "memory",
            "--endpoint",
            &format!("fs={SECRET_URL}"),
        ],
        &[],
    );
    let t = text(&o);
    assert_eq!(o.status.code(), Some(2), "case 2: {t}");
    assert!(
        !t.contains("hunter2") && !t.contains("db.internal") && !t.contains(SECRET_URL),
        "case 2: a typed URL was echoed"
    );
}

#[test]
fn the_same_backend_twice_is_refused_before_anything_opens() {
    let dir = fs_with("same", 3);
    let ep = format!("fs={}", dir.display());
    let o = run_cli(
        &["transfer", "--from", "fs", "--to", "fs", "--endpoint", &ep],
        &[],
    );
    let after = keys_of(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(2), "case 3: {}", text(&o));
    assert!(after.len() == 3, "case 3: source changed");
}

#[test]
fn an_unknown_backend_is_refused_naming_it() {
    let o = run_cli(&["transfer", "--from", "nosuchdb", "--to", "memory"], &[]);
    let t = text(&o);
    assert_eq!(o.status.code(), Some(2), "case 4: {t}");
    assert!(t.contains("nosuchdb"), "case 4: {t}");
}

#[test]
fn a_backend_that_needs_a_path_and_gets_none_is_refused() {
    let o = run_cli(&["transfer", "--from", "fs", "--to", "memory"], &[]);
    let t = text(&o);
    assert_eq!(o.status.code(), Some(2), "case 5: {t}");
    assert!(t.contains("fs"), "case 5: {t}");
}

#[test]
fn an_endpoint_for_a_backend_not_on_either_end_is_refused() {
    let dir = fs_with("stray", 2);
    let ep = format!("fs={}", dir.display());
    let o = run_cli(&transfer_args(&ep, &["--endpoint", "sqlite=/x"]), &[]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(2), "case 6: {}", text(&o));
    assert!(text(&o).contains("sqlite"), "case 6: {}", text(&o));
}

#[test]
fn a_failed_open_is_an_error_exit_and_prints_no_secret() {
    let dir = fs_with("fail", 2);
    let ep = format!("fs={}", dir.display());
    // The target needs a capability no listed backend has, so the target cannot open.
    let o = run_cli(
        &transfer_args(&ep, &["--require", "no.such.capability"]),
        &[("APP_DB_URL", SECRET_URL)],
    );
    let _ = std::fs::remove_dir_all(&dir);
    let t = text(&o);
    assert!(!o.status.success(), "case 7: {t}");
    assert!(
        !t.contains("hunter2"),
        "case 7: a secret reached the output"
    );
}
