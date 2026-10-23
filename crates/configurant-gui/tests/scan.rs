// SPDX-License-Identifier: MIT OR Apache-2.0
//! "Add from scan" (feature `scan`): nothing changes until the schema review is confirmed, a partial scan says
//! so, a failed scan is shown and changes nothing, and a secret never gets a default on screen or in the new text.
#![cfg(feature = "scan")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use configurant::edit::Session;
use configurant::{Config, Loader, Schema};
use configurant_gui::{App, Effect, ScanSetup};
use configurant_scan::{Proposals, Resolver, ScanError, scan};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

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

fn found(name: &str, resolves: bool) -> Proposals {
    let root = std::env::temp_dir().join(format!("cfg-gui-scan-{name}-{}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src").join("main.rs"), SOURCE).unwrap();
    let schema = Schema::parse(SCHEMA).unwrap();
    let out = scan(&root, &schema, &mut Scripted(resolves)).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    out
}

fn config() -> Config {
    let schema = Schema::parse(SCHEMA).unwrap();
    Loader::new(&schema).env(BTreeMap::new()).build().unwrap()
}

#[derive(Default)]
struct State {
    scan_started: bool,
    schema_text: Option<String>,
}

fn setup() -> ScanSetup {
    ScanSetup {
        schema_path: PathBuf::from("configurant.toml"),
        schema_text: SCHEMA.to_owned(),
        root: PathBuf::from("."),
        server: "unused".to_owned(),
        wait: Duration::from_secs(1),
    }
}

/// The app rides in the harness state beside what the caller would have been handed.
struct Rig<'a> {
    app: App<'a>,
    seen: State,
}

fn harness(config: &Config, with_scan: bool) -> Harness<'_, Rig<'_>> {
    let mut app = App::new(Session::new(config, None));
    if with_scan {
        app = app.with_scan(setup());
    }
    Harness::new_ui_state(
        |ui, r: &mut Rig| match r.app.ui(ui) {
            Effect::Scan(_) => r.seen.scan_started = true,
            Effect::WriteSchema(t) => r.seen.schema_text = Some(t),
            _ => {}
        },
        Rig {
            app,
            seen: State::default(),
        },
    )
}

fn start(h: &mut Harness<'_, Rig<'_>>, result: Proposals) {
    h.get_by_label("Add from scan").click();
    h.step(); // the scanning screen repaints for ever, so `run` would not settle
    assert!(h.state().seen.scan_started, "the button must start a scan");
    h.state_mut().app.scanned(Ok(result));
    h.run();
}

fn text_of(h: &Harness<'_, Rig<'_>>) -> bool {
    h.state().seen.schema_text.is_some()
}

#[test]
fn the_schema_is_written_only_after_the_review_is_confirmed() {
    let config = config();
    let mut h = harness(&config, true);
    start(&mut h, found("flow", true));
    assert!(h.query_by_label_contains("APP_HOST -> app.host").is_some());

    // Nothing ticked: review says so and goes no further.
    h.get_by_label("Review the ticked entries").click();
    h.run();
    assert!(h.query_by_label_contains("tick at least one").is_some());

    // Tick the first proposal, review it, go back, review again; only the confirm button yields the write.
    h.get_by_label_contains("APP_HOST").click();
    h.run();
    h.get_by_label("Review the ticked entries").click();
    h.run();
    assert!(
        h.query_by_label_contains("The schema is not changed until you confirm")
            .is_some()
    );
    assert!(!text_of(&h), "reviewing must not write");
    h.get_by_label("Go back").click();
    h.run();
    assert!(h.query_by_label("Review the ticked entries").is_some());
    h.get_by_label("Review the ticked entries").click();
    h.run();
    h.get_by_label("Write the schema").click();
    h.run();
    let text = h
        .state()
        .seen
        .schema_text
        .clone()
        .expect("confirming must yield the write");
    let new = Schema::parse(&text).unwrap();
    assert!(new.get("app.host").is_some());
    // Only what was ticked was added.
    assert!(new.get("db.password").is_none());

    h.state_mut().app.schema_written(Ok(()));
    h.run();
    assert!(
        h.query_by_label_contains("start configurant-gui again")
            .is_some()
    );
}

#[test]
fn a_partial_scan_says_so_before_the_list() {
    let config = config();
    let mut h = harness(&config, true);
    start(&mut h, found("partial", false));
    assert!(h.query_by_label_contains("PARTIAL").is_some());
    assert!(
        h.query_by_label_contains("may be missing variables")
            .is_some()
    );
}

#[test]
fn a_failed_scan_is_shown_and_changes_nothing() {
    let config = config();
    let mut h = harness(&config, true);
    h.get_by_label("Add from scan").click();
    h.step();
    h.state_mut()
        .app
        .scanned(Err("language server `unused`: not found".to_owned()));
    h.run();
    assert!(
        h.query_by_label_contains("scan failed, nothing was changed")
            .is_some()
    );
    assert!(h.query_by_label_contains("not found").is_some());
    // Back on the value list, with no way to write a schema from here.
    assert!(h.query_by_label("server.port   8080").is_some());
    assert!(h.query_by_label("Write the schema").is_none());
    assert!(!text_of(&h));
}

#[test]
fn a_failed_schema_write_stays_on_the_review_and_says_why() {
    let config = config();
    let mut h = harness(&config, true);
    start(&mut h, found("writefail", true));
    h.get_by_label_contains("APP_HOST").click();
    h.run();
    h.get_by_label("Review the ticked entries").click();
    h.run();
    h.get_by_label("Write the schema").click();
    h.run();
    assert!(text_of(&h));
    h.state_mut().app.schema_written(Err(
        "configurant.toml.bak exists; move it away first".to_owned()
    ));
    h.run();
    assert!(h.query_by_label_contains("schema not written").is_some());
    assert!(h.query_by_label("Write the schema").is_some());
}

#[test]
fn a_secret_is_proposed_without_a_default_anywhere() {
    let config = config();
    let mut h = harness(&config, true);
    start(&mut h, found("secret", true));
    assert!(h.query_by_label_contains("DB_PASSWORD").is_some());
    assert!(h.query_by_label_contains("(no default)").is_some());
    h.get_by_label_contains("APP_HOST").click();
    h.get_by_label_contains("DB_PASSWORD").click();
    h.run();
    h.get_by_label("Review the ticked entries").click();
    h.run();
    h.get_by_label("Write the schema").click();
    h.run();
    let text = h.state().seen.schema_text.clone().expect("a schema write");
    // Bools, never the value in a message: a failing assert must not print the secret.
    assert!(h.query_by_label_contains("hunter2-canary").is_none());
    assert!(!text.contains("hunter2-canary"));
    // The ordinary default beside the other read is still proposed.
    assert!(text.contains("localhost"));
}

#[test]
fn staged_edits_block_a_scan_and_a_build_without_scan_setup_has_no_button() {
    let config = config();
    let mut h = harness(&config, true);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("New value").type_text("9001");
    h.run();
    h.get_by_label("Stage").click();
    h.run();
    h.get_by_label("Add from scan").click();
    h.run();
    assert!(!h.state().seen.scan_started);
    assert!(
        h.query_by_label_contains("review or unstage your changes")
            .is_some()
    );

    let plain = harness(&config, false);
    assert!(plain.query_by_label("Add from scan").is_none());
}
