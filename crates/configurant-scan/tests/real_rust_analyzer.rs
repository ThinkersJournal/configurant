// SPDX-License-Identifier: MIT OR Apache-2.0
//! One scan against a real rust-analyzer. It needs `rust-analyzer` and the `rust-src` component on the
//! machine (CI runs `rustup component add rust-analyzer rust-src`); when they are missing this fails, it does
//! not skip.

use std::time::Duration;

use configurant::Schema;
use configurant_scan::lsp::LspResolver;
use configurant_scan::scan;

const SCHEMA: &str = r#"
[project]
name = "t"
version = "1.0.0"
schema_version = 1
"#;

const MAIN: &str = r#"mod env {
    pub fn var(_: &str) -> Result<String, ()> {
        Err(())
    }
}

fn main() {
    let _ = std::env::var("REAL_HOST").unwrap_or("localhost".into());
    let _ = std::env::var("REAL_TOKEN");
    let _ = env::var("OWN_THING");
}
"#;

#[test]
fn rust_analyzer_confirms_std_env_and_rejects_a_look_alike() {
    let dir = std::env::temp_dir().join(format!("cfg-scan-real-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/main.rs"), MAIN).unwrap();

    let mut ra = LspResolver::start("rust-analyzer", &[], &dir, Duration::from_secs(180))
        .expect("rust-analyzer must start (rustup component add rust-analyzer rust-src)");
    let p = scan(&dir, &Schema::parse(SCHEMA).unwrap(), &mut ra).unwrap();
    drop(ra);
    let _ = std::fs::remove_dir_all(&dir);

    assert!(!p.is_partial(), "unresolved calls: {}", p.unresolved);
    let envs: Vec<_> = p.proposals.iter().map(|x| x.env.as_str()).collect();
    assert_eq!(envs, ["REAL_HOST", "REAL_TOKEN"]);
    assert_eq!(p.rejected, 1, "the module's own `env::var` is not std");
    let host = &p.proposals[0];
    assert_eq!(host.default_seen.as_deref(), Some("localhost"));
    assert!(p.proposals[1].secret);
}

#[test]
fn the_binary_reports_and_accepts_against_a_real_server() {
    let dir = std::env::temp_dir().join(format!("cfg-scan-real-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]
name = \"fixture\"
version = \"0.1.0\"
edition = \"2021\"
",
    )
    .unwrap();
    std::fs::write(dir.join("src/main.rs"), MAIN).unwrap();
    let schema = dir.join("configurant.toml");
    std::fs::write(&schema, SCHEMA).unwrap();
    let bin = env!("CARGO_BIN_EXE_configurant-scan");
    let base = [
        schema.to_str().unwrap(),
        "--root",
        dir.to_str().unwrap(),
        "--wait",
        "180",
    ];

    let out = std::process::Command::new(bin).args(base).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let ok = out.status.code() == Some(0)
        && text.contains("REAL_HOST -> real.host")
        && text.contains("REAL_TOKEN -> real.token")
        && !text.contains("PARTIAL");
    assert!(ok, "report run failed with code {:?}", out.status.code());

    let out = std::process::Command::new(bin)
        .args(base)
        .args(["--accept", "REAL_HOST", "--write"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let written = std::fs::read_to_string(&schema).unwrap();
    let s = Schema::parse(&written).unwrap();
    assert!(s.get("real.host").is_some() && s.get("real.token").is_none());
    assert_eq!(
        std::fs::read_to_string(dir.join("configurant.toml.bak")).unwrap(),
        SCHEMA
    );
    let _ = std::fs::remove_dir_all(&dir);
}
