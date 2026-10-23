// SPDX-License-Identifier: MIT OR Apache-2.0
//! "Add from scan" (feature `scan`): nothing changes until the schema review's `y`, a partial scan says so, a
//! failed scan is shown and changes nothing, and a secret never gets a default on screen or in the new text.
#![cfg(feature = "scan")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use configurant::edit::Session;
use configurant::{Loader, Schema};
use configurant_scan::{Resolver, ScanError, scan};
use configurant_tui::{App, Effect, ScanSetup};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

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
"#;

const SOURCE: &str = "fn main() {\n    let _ = std::env::var(\"APP_HOST\").unwrap_or(\"localhost\".into());\n    let _ = std::env::var(\"DB_PASSWORD\").unwrap_or(\"hunter2-canary\".into());\n}\n";

/// Answers every definition request with `std::env`, or with nothing at all.
struct Scripted(bool);

impl Resolver for Scripted {
    fn definition(
        &mut self,
        _file: &Path,
        _text: &str,
        _line: u32,
        _col: u32,
    ) -> Result<Option<String>, ScanError> {
        Ok(self.0.then(|| "/rust/library/std/src/env.rs".to_owned()))
    }
}

fn project(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cfg-tui-scan-{name}-{}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src").join("main.rs"), SOURCE).unwrap();
    root
}

fn found(name: &str, resolves: bool) -> configurant_scan::Proposals {
    let root = project(name);
    let schema = Schema::parse(SCHEMA).unwrap();
    let out = scan(&root, &schema, &mut Scripted(resolves)).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    out
}

fn press(app: &mut App, code: KeyCode) -> Effect {
    app.handle_key(KeyEvent::from(code))
}

fn screen(app: &App) -> String {
    let mut t = Terminal::new(TestBackend::new(110, 24)).unwrap();
    t.draw(|f| app.draw(f)).unwrap();
    t.backend().to_string()
}

fn with_scan(config: &configurant::Config) -> App<'_> {
    App::new(Session::new(config, None)).with_scan(ScanSetup {
        schema_path: PathBuf::from("configurant.toml"),
        schema_text: SCHEMA.to_owned(),
        root: PathBuf::from("."),
        server: "unused".to_owned(),
        wait: Duration::from_secs(1),
    })
}

fn config() -> configurant::Config {
    let schema = Schema::parse(SCHEMA).unwrap();
    Loader::new(&schema).env(BTreeMap::new()).build().unwrap()
}

#[test]
fn the_schema_is_written_only_after_the_review_is_confirmed() {
    let config = config();
    let mut app = with_scan(&config);
    assert!(matches!(
        press(&mut app, KeyCode::Char('a')),
        Effect::Scan(_)
    ));
    app.scanned(Ok(found("flow", true)));
    assert!(screen(&app).contains("APP_HOST -> app.host"));

    // Nothing ticked: Enter says so and goes no further.
    assert!(matches!(press(&mut app, KeyCode::Enter), Effect::None));
    assert!(screen(&app).contains("tick at least one"));

    // Tick the first proposal, review it, go back, review again; only `y` yields the write.
    press(&mut app, KeyCode::Char(' '));
    assert!(screen(&app).contains("[x] APP_HOST"));
    assert!(matches!(press(&mut app, KeyCode::Enter), Effect::None));
    assert!(screen(&app).contains("the schema is not changed until you confirm"));
    press(&mut app, KeyCode::Esc);
    assert!(screen(&app).contains("proposed entries"));
    press(&mut app, KeyCode::Enter);
    let Effect::WriteSchema(text) = press(&mut app, KeyCode::Char('y')) else {
        panic!("y did not yield the schema write");
    };
    let new = Schema::parse(&text).unwrap();
    assert!(new.get("app.host").is_some());
    // Only what was ticked was added.
    assert!(new.get("db.password").is_none());

    app.schema_written(Ok(()));
    assert!(screen(&app).contains("start configurant-tui again"));
    assert!(matches!(press(&mut app, KeyCode::Char('x')), Effect::Quit));
}

#[test]
fn a_partial_scan_says_so_before_the_list() {
    let config = config();
    let mut app = with_scan(&config);
    press(&mut app, KeyCode::Char('a'));
    app.scanned(Ok(found("partial", false)));
    let s = screen(&app);
    assert!(s.contains("PARTIAL"));
    assert!(s.contains("may be missing variables"));
}

#[test]
fn a_failed_scan_is_shown_and_changes_nothing() {
    let config = config();
    let mut app = with_scan(&config);
    press(&mut app, KeyCode::Char('a'));
    app.scanned(Err("language server `unused`: not found".to_owned()));
    let s = screen(&app);
    assert!(s.contains("scan failed, nothing was changed"));
    assert!(s.contains("not found"));
    // Back on the value list, with no way to write a schema from here.
    assert!(s.contains("server.port"));
    assert!(matches!(press(&mut app, KeyCode::Char('y')), Effect::None));
}

#[test]
fn a_failed_schema_write_stays_on_the_review_and_says_why() {
    let config = config();
    let mut app = with_scan(&config);
    press(&mut app, KeyCode::Char('a'));
    app.scanned(Ok(found("writefail", true)));
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Enter);
    assert!(matches!(
        press(&mut app, KeyCode::Char('y')),
        Effect::WriteSchema(_)
    ));
    app.schema_written(Err(
        "configurant.toml.bak exists; move it away first".to_owned()
    ));
    let s = screen(&app);
    assert!(s.contains("schema not written"));
    assert!(s.contains("write the schema"));
}

#[test]
fn a_secret_is_proposed_without_a_default_anywhere() {
    let config = config();
    let mut app = with_scan(&config);
    press(&mut app, KeyCode::Char('a'));
    app.scanned(Ok(found("secret", true)));
    let mut seen = screen(&app);
    assert!(seen.contains("DB_PASSWORD"));
    assert!(seen.contains("(no default)"));
    // Tick both, review.
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Char(' '));
    seen += &screen(&app);
    press(&mut app, KeyCode::Enter);
    seen += &screen(&app);
    let Effect::WriteSchema(text) = press(&mut app, KeyCode::Char('y')) else {
        panic!("y did not yield the schema write");
    };
    assert!(!seen.contains("hunter2-canary"));
    assert!(!text.contains("hunter2-canary"));
    // The ordinary default beside the other read is still proposed.
    assert!(text.contains("localhost"));
}

#[test]
fn staged_edits_block_a_scan_and_a_build_without_scan_setup_ignores_the_key() {
    let config = config();
    let mut app = with_scan(&config);
    press(&mut app, KeyCode::Enter);
    for c in "9001".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(matches!(press(&mut app, KeyCode::Char('a')), Effect::None));
    assert!(screen(&app).contains("review or unstage your changes"));

    let mut plain = App::new(Session::new(&config, None));
    assert!(matches!(
        press(&mut plain, KeyCode::Char('a')),
        Effect::None
    ));
}
