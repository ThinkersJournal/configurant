// SPDX-License-Identifier: MIT OR Apache-2.0
//! Running a data transfer from the GUI. Known-bad first: a failed copy must write nothing, a typed URL
//! must be refused and leave no trace in the accessibility tree, writing without copying must take a
//! deliberate button and a second confirmation, and the needs and endpoint answers must reach the module.

use std::collections::BTreeMap;
use std::path::PathBuf;

use configurant::edit::{Confirmed, Session};
use configurant::persistant::{Durability, PersistantModule, TransferError, TransferJob};
use configurant::{Config, Loader, Modules, Needs, Schema};
use configurant_gui::{App, Effect, TransferSetup};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use persistant::capability::provider::ProviderNeeds;
use persistant::kv::KvBackend;
use persistant::registry::{self, Endpoint};

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

const URL: &str = "postgres://appuser:hunter2@db.internal/prod";

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

struct State<'a> {
    app: App<'a>,
    written: Option<Confirmed>,
    job: Option<TransferJob>,
}

/// A config whose backend is `fs`, with the module registered.
fn world() -> (Config, Modules) {
    let schema = Schema::parse(SCHEMA).unwrap();
    let mut modules = Modules::new();
    modules.register(Box::new(module())).unwrap();
    let config = Loader::new(&schema)
        .modules(&modules)
        .file("demo.toml", "[storage]\nbackend = \"fs\"\n")
        .build()
        .unwrap();
    (config, modules)
}

fn harness<'a>(
    config: &'a Config,
    modules: &'a Modules,
    transfer: Option<TransferSetup>,
) -> Harness<'a, State<'a>> {
    let mut app = App::new(Session::new(config, Some(modules)));
    if let Some(t) = transfer {
        app = app.with_transfer(t);
    }
    Harness::new_ui_state(
        |ui, s: &mut State| match s.app.ui(ui) {
            Effect::None | Effect::Quit => {}
            Effect::Write(c) => s.written = Some(c),
            Effect::Transfer(j) => s.job = Some(j),
            #[cfg(feature = "scan")]
            _ => {}
        },
        State {
            app,
            written: None,
            job: None,
        },
    )
}

fn click(h: &mut Harness<'_, State<'_>>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

/// Click the copy button. The spinner repaints for as long as the copy runs, so the harness is stepped
/// rather than run to idle.
fn start_copy(h: &mut Harness<'_, State<'_>>) {
    h.get_by_label("Copy and write the file").click();
    h.step();
    h.step();
}

/// Stage `fs` -> `memory`, review, confirm: the app should now be asking for endpoints.
fn confirm_the_switch(h: &mut Harness<'_, State<'_>>) {
    click(h, "Edit");
    h.get_by_label("New value").type_text("memory");
    h.run();
    click(h, "Stage");
    click(h, "Review and confirm");
    click(h, "Write the file");
}

fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "configurant-gui-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn a_changed_backend_asks_for_endpoints_and_writes_nothing_yet() {
    let (config, modules) = world();
    let mut h = harness(&config, &modules, Some(setup(&[])));
    confirm_the_switch(&mut h);
    assert!(h.state().written.is_none() && h.state().job.is_none());
    assert!(
        h.query_by_label_contains("Move the data: copy `fs` -> `memory`")
            .is_some()
    );
    assert!(h.query_by_label("Source endpoint of `fs`").is_some());
    assert!(h.query_by_label("Write without copying...").is_some());
}

#[test]
fn without_a_setup_the_plan_is_only_listed_and_the_write_goes_ahead() {
    let (config, modules) = world();
    let mut h = harness(&config, &modules, None);
    confirm_the_switch(&mut h);
    let confirmed = h.state_mut().written.take().expect("a plain write");
    h.state_mut().app.wrote(Ok(()), &confirmed);
    h.run();
    assert!(h.query_by_label_contains("NOT DONE").is_some());
}

#[test]
fn a_typed_url_is_refused_and_leaves_no_trace_in_the_tree() {
    let (config, modules) = world();
    let mut h = harness(&config, &modules, Some(setup(&[])));
    confirm_the_switch(&mut h);
    h.get_by_label("Source endpoint of `fs`").type_text(URL);
    h.run();
    click(&mut h, "Copy and write the file");
    assert!(h.state().job.is_none() && h.state().written.is_none());
    assert!(
        h.query_by_label_contains("source endpoint: this backend opens from a path")
            .is_some()
    );
    for needle in ["hunter2", "db.internal"] {
        assert!(h.query_by_label_contains(needle).is_none(), "{needle}");
    }
    // The box itself was cleared, so the URL is not in any text field's value either.
    assert!(
        h.query_by_value(URL).is_none(),
        "the typed URL is still in its box"
    );
}

#[test]
fn a_failed_copy_writes_nothing_and_keeps_the_staged_edit() {
    let dir = scratch_dir("fail");
    let answer = dir.display().to_string();
    let (config, modules) = world();
    let mut h = harness(&config, &modules, Some(setup(&[("fs", &answer)])));
    confirm_the_switch(&mut h);
    start_copy(&mut h);
    let job = h.state_mut().job.take().expect("a transfer should start");
    assert_eq!((job.from_id(), job.to_id()), ("fs", "memory"));
    assert!(
        h.query_by_label_contains("Copying `fs` -> `memory`")
            .is_some()
    );
    // Nothing can be pressed while it runs.
    assert!(h.query_by_label("Copy and write the file").is_none());
    assert!(h.query_by_label("Write the file").is_none());

    let effect = h
        .state_mut()
        .app
        .transferred(Err(TransferError::TargetNotEmpty));
    assert!(matches!(effect, Effect::None), "{effect:?}");
    h.run();
    assert!(h.state().written.is_none());
    assert!(h.query_by_label_contains("copy failed").is_some());
    // The edit is still staged: review is available again.
    click(&mut h, "Review and confirm");
    assert!(h.query_by_label_contains("\"fs\" -> \"memory\"").is_some());
}

#[test]
fn writing_without_copying_takes_a_deliberate_button_and_a_second_yes() {
    let (config, modules) = world();
    let mut h = harness(&config, &modules, Some(setup(&[])));
    confirm_the_switch(&mut h);
    click(&mut h, "Write without copying...");
    assert!(h.state().written.is_none(), "the first click only asks");
    assert!(
        h.query_by_label_contains("Write the file WITHOUT copying?")
            .is_some()
    );
    // Going back returns to the questions.
    click(&mut h, "Go back");
    assert!(h.query_by_label_contains("Move the data").is_some());
    click(&mut h, "Write without copying...");
    click(&mut h, "Write without copying");
    let confirmed = h.state_mut().written.take().expect("the write");
    h.state_mut().app.wrote(Ok(()), &confirmed);
    h.run();
    assert!(h.query_by_label_contains("NOT COPIED").is_some());
}

#[test]
fn the_needs_reach_the_choice_list() {
    // Control: with no needs, `memory` is offered as a choice.
    let (config, modules) = world();
    let mut h = harness(&config, &modules, Some(setup(&[])));
    click(&mut h, "Edit");
    assert!(h.query_by_label_contains("memory").is_some());

    // A need no backend meets empties the list.
    let mut s = setup(&[]);
    s.needs = Needs {
        required: vec!["kv.no_such_capability".into()],
        order: Vec::new(),
    };
    let mut h = harness(&config, &modules, Some(s));
    click(&mut h, "Edit");
    assert!(h.query_by_label_contains("memory").is_none());
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
    let (config, modules) = world();
    let mut h = harness(&config, &modules, Some(setup(&[("fs", &answer)])));
    confirm_the_switch(&mut h);
    start_copy(&mut h);
    let job = h.state_mut().job.take().expect("a transfer should start");
    let result = job.run();
    let Effect::Write(confirmed) = h.state_mut().app.transferred(result) else {
        panic!("a finished copy must release the write");
    };
    h.state_mut().app.wrote(Ok(()), &confirmed);
    h.run();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        h.query_by_label_contains("Copied `fs` -> `memory`")
            .is_some()
    );
    assert!(h.query_by_label_contains("copied 3 records").is_some());
    assert!(h.query_by_label_contains("verified 3").is_some());
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
    let (config, modules) = world();
    let mut h = harness(&config, &modules, Some(setup(&[("fs", &answer)])));
    confirm_the_switch(&mut h);
    start_copy(&mut h);
    assert!(h.query_by_label_contains("starting").is_some());
    let job = h.state_mut().job.take().expect("a transfer should start");
    let result = job.run();
    h.step();
    h.step();
    let shown = h
        .query_by_label_contains("checking the copy: 3 of 3 records")
        .is_some();
    let _ = h.state_mut().app.transferred(result);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(shown, "the count is on the copy screen");
}
