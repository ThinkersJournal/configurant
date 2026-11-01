// SPDX-License-Identifier: MIT OR Apache-2.0
//! Running a data transfer from the TUI. Known-bad first: a failed copy must write nothing, a typed URL
//! must be refused and leave no trace on screen, writing without copying must take a deliberate key and
//! a yes, and the needs and the endpoint answers must reach the module.

use std::collections::BTreeMap;
use std::path::PathBuf;

use configurant::edit::Session;
use configurant::persistant::{Durability, PersistantModule, TransferError};
use configurant::{Loader, Modules, Needs, Schema};
use configurant_tui::{App, Effect, TransferSetup};
use persistant::capability::provider::ProviderNeeds;
use persistant::kv::KvBackend;
use persistant::registry::{self, Endpoint};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "storage.backend"
type = "string"
default = "memory"
description = "where records are kept"
provider = "persistant"
"#;

fn module() -> PersistantModule {
    PersistantModule::new().min_durability(Durability::Volatile)
}

fn setup(endpoints: &[(&str, &str)]) -> TransferSetup {
    TransferSetup {
        module: module(),
        needs: Needs::default(),
        endpoints: endpoints
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// An app editing a file whose backend is `fs`, with the module registered; `f` drives it.
fn with_app<R>(
    transfer: Option<TransferSetup>,
    module: PersistantModule,
    f: impl FnOnce(&mut App) -> R,
) -> R {
    let schema = Schema::parse(SCHEMA).unwrap();
    let mut modules = Modules::new();
    modules.register(Box::new(module)).unwrap();
    let config = Loader::new(&schema)
        .modules(&modules)
        .file("demo.toml", "[storage]\nbackend = \"fs\"\n")
        .build()
        .unwrap();
    let mut app = App::new(Session::new(&config, Some(&modules)));
    if let Some(t) = transfer {
        app = app.with_transfer(t);
    }
    f(&mut app)
}

fn press(app: &mut App, code: KeyCode) -> Effect {
    app.handle_key(KeyEvent::from(code))
}

fn ctrl(app: &mut App, c: char) -> Effect {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

/// The screen as words, so a phrase that wrapped still matches.
fn screen(app: &App) -> String {
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| app.draw(f)).unwrap();
    t.backend()
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Stage `fs` -> `memory`, review, confirm: the app should now be asking for endpoints.
fn confirm_the_switch(app: &mut App) -> Effect {
    press(app, KeyCode::Enter);
    type_text(app, "memory");
    press(app, KeyCode::Enter);
    press(app, KeyCode::Char('r'));
    press(app, KeyCode::Char('y'))
}

fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "configurant-tui-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn a_changed_backend_asks_for_endpoints_and_writes_nothing_yet() {
    with_app(Some(setup(&[])), module(), |app| {
        let effect = confirm_the_switch(app);
        assert!(matches!(effect, Effect::None), "{effect:?}");
        let s = screen(app);
        assert!(s.contains("Move the data: copy `fs` -> `memory`"), "{s}");
        assert!(s.contains("source endpoint of `fs`"), "{s}");
        assert!(s.contains("Ctrl-W write without copying"), "{s}");
    });
}

#[test]
fn without_a_setup_the_plan_is_only_listed_and_the_write_goes_ahead() {
    with_app(None, module(), |app| {
        let effect = confirm_the_switch(app);
        let Effect::Write(confirmed) = effect else {
            panic!("expected a write, got {effect:?}");
        };
        app.wrote(Ok(()), &confirmed);
        assert!(screen(app).contains("NOT DONE"), "{}", screen(app));
    });
}

#[test]
fn a_typed_url_is_refused_and_leaves_no_trace_on_screen() {
    with_app(Some(setup(&[])), module(), |app| {
        confirm_the_switch(app);
        type_text(app, "postgres://appuser:hunter2@db.internal/prod");
        press(app, KeyCode::Enter); // on to the target (memory: nothing to give)
        let effect = press(app, KeyCode::Enter); // try to start
        assert!(matches!(effect, Effect::None), "{effect:?}");
        let s = screen(app);
        assert!(
            s.contains("source endpoint: this backend opens from a path"),
            "{s}"
        );
        assert!(
            !s.contains("hunter2") && !s.contains("db.internal"),
            "leaked: {s}"
        );
    });
}

#[test]
fn a_failed_copy_writes_nothing_and_keeps_the_staged_edit() {
    let dir = scratch_dir("fail");
    let answer = dir.display().to_string();
    with_app(Some(setup(&[("fs", &answer)])), module(), |app| {
        confirm_the_switch(app);
        press(app, KeyCode::Enter);
        let Effect::Transfer(job) = press(app, KeyCode::Enter) else {
            panic!("expected a transfer to start");
        };
        assert_eq!((job.from_id(), job.to_id()), ("fs", "memory"));
        assert!(
            screen(app).contains("Copying `fs` -> `memory`"),
            "{}",
            screen(app)
        );

        let effect = app.transferred(Err(TransferError::TargetNotEmpty));
        assert!(
            matches!(effect, Effect::None),
            "no write after a failed copy: {effect:?}"
        );
        assert!(screen(app).contains("copy failed"), "{}", screen(app));
        // The edit is still staged: review is available again.
        press(app, KeyCode::Char('r'));
        assert!(
            screen(app).contains(r#""fs" -> "memory""#),
            "{}",
            screen(app)
        );
    });
}

#[test]
fn writing_without_copying_takes_a_deliberate_key_and_a_yes() {
    with_app(Some(setup(&[])), module(), |app| {
        confirm_the_switch(app);
        // The plain keys do not skip the copy.
        assert!(matches!(press(app, KeyCode::Char('w')), Effect::None));
        press(app, KeyCode::Backspace);
        assert!(matches!(ctrl(app, 'w'), Effect::None));
        assert!(
            screen(app).contains("Write the file WITHOUT copying?"),
            "{}",
            screen(app)
        );
        // `n` goes back to the questions.
        press(app, KeyCode::Char('n'));
        assert!(screen(app).contains("Move the data"), "{}", screen(app));
        // Ctrl-W then `y` writes, and the result says plainly that nothing was copied.
        ctrl(app, 'w');
        let Effect::Write(confirmed) = press(app, KeyCode::Char('y')) else {
            panic!("expected the write");
        };
        app.wrote(Ok(()), &confirmed);
        assert!(screen(app).contains("NOT COPIED"), "{}", screen(app));
    });
}

#[test]
fn keys_do_nothing_while_the_copy_runs() {
    let dir = scratch_dir("busy");
    let answer = dir.display().to_string();
    with_app(Some(setup(&[("fs", &answer)])), module(), |app| {
        confirm_the_switch(app);
        press(app, KeyCode::Enter);
        assert!(matches!(press(app, KeyCode::Enter), Effect::Transfer(_)));
        for code in [
            KeyCode::Char('y'),
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char('q'),
        ] {
            assert!(matches!(press(app, code), Effect::None));
        }
        assert!(screen(app).contains("Copying"), "{}", screen(app));
        assert!(
            matches!(ctrl(app, 'c'), Effect::Quit),
            "Ctrl-C still leaves"
        );
    });
}

#[test]
fn the_needs_reach_the_choice_list() {
    // Control: with no needs, Tab walks on to `memory` (the module's floor is volatile).
    let mut seen = String::new();
    with_app(Some(setup(&[])), module(), |app| {
        press(app, KeyCode::Enter);
        for _ in 0..3 {
            press(app, KeyCode::Tab);
            seen.push_str(&screen(app));
        }
    });
    assert!(seen.contains("new value> memory"), "{seen}");

    // A need no backend meets empties the list, so Tab can reach nothing.
    let mut s = setup(&[]);
    s.needs = Needs {
        required: vec!["kv.no_such_capability".into()],
        order: Vec::new(),
    };
    let mut seen = String::new();
    with_app(Some(s), module(), |app| {
        press(app, KeyCode::Enter);
        for _ in 0..3 {
            press(app, KeyCode::Tab);
            seen.push_str(&screen(app));
        }
    });
    assert!(!seen.contains("new value> memory"), "{seen}");
}

#[test]
fn a_real_copy_runs_before_the_write_and_the_result_says_what_was_copied() {
    let dir = scratch_dir("real");
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    rt.block_on(async {
        let gate = ProviderNeeds::new(Vec::new(), Durability::Volatile);
        let h = registry::open_named("fs", &Endpoint::path(&dir), &gate)
            .await
            .unwrap();
        for i in 0..3u8 {
            h.kv()
                .put(&format!("k{i}"), vec![i, 255], None)
                .await
                .unwrap();
        }
    });
    let answer = dir.display().to_string();
    let outcome = with_app(Some(setup(&[("fs", &answer)])), module(), |app| {
        confirm_the_switch(app);
        press(app, KeyCode::Enter);
        let Effect::Transfer(job) = press(app, KeyCode::Enter) else {
            panic!("expected a transfer to start");
        };
        let result = job.run();
        let Effect::Write(confirmed) = app.transferred(result) else {
            panic!("a finished copy must release the write");
        };
        app.wrote(Ok(()), &confirmed);
        screen(app)
    });
    let _ = std::fs::remove_dir_all(&dir);
    assert!(outcome.contains("Copied `fs` -> `memory`"), "{outcome}");
    assert!(outcome.contains("copied 3 records"), "{outcome}");
    assert!(outcome.contains("verified 3"), "{outcome}");
}

#[test]
fn the_copy_screen_shows_how_far_the_copy_got_in_place_of_a_bare_spinner() {
    let dir = scratch_dir("progress");
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    rt.block_on(async {
        let gate = ProviderNeeds::new(Vec::new(), Durability::Volatile);
        let h = registry::open_named("fs", &Endpoint::path(&dir), &gate)
            .await
            .unwrap();
        for i in 0..3u8 {
            h.kv()
                .put(&format!("k{i}"), vec![i, 255], None)
                .await
                .unwrap();
        }
    });
    let answer = dir.display().to_string();
    let (before, after) = with_app(Some(setup(&[("fs", &answer)])), module(), |app| {
        confirm_the_switch(app);
        press(app, KeyCode::Enter);
        let Effect::Transfer(job) = press(app, KeyCode::Enter) else {
            panic!("expected a transfer to start");
        };
        let before = screen(app);
        let result = job.run();
        // Still on the copy screen: the result has not been handed back yet.
        let after = screen(app);
        let _ = app.transferred(result);
        (before, after)
    });
    let _ = std::fs::remove_dir_all(&dir);
    assert!(before.contains("Copying `fs` -> `memory`"));
    assert!(before.contains("starting"));
    assert!(after.contains("checking the copy: 3 of 3 records"));
}
