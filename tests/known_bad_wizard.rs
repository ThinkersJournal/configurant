// SPDX-License-Identifier: MIT OR Apache-2.0
//! The wizard (`docs/STEP-ENGINE.md`, part b): `next`, `back` and `goto` walk the schema's steps, a hidden
//! step is skipped, a required value stops `next`, the review is the only place to confirm, and neither the
//! view nor the saved position ever holds a secret or a value.

use std::collections::BTreeMap;

use configurant::edit::Session;
use configurant::view::{Driver, Event};
use configurant::wizard::{Wizard, WizardState};
use configurant::{Loader, Needs, Schema};

const CANARY: &str = "canary-s3cr3t-value";

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1
env_prefix = "DEMO_"

[[value]]
key = "app.name"
type = "string"
required = true
description = "app name"

[[value]]
key = "mode"
type = "enum"
choices = ["server", "client"]
default = "client"
description = "what to run"

[[value]]
key = "server.port"
type = "int"
default = 8080
description = "listen port"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "DEMO_API_TOKEN"
description = "api token"

[[step]]
id = "name"
title = "Name it"
keys = ["app.name"]

[[step]]
id = "mode"
title = "What do you want to run?"
keys = ["mode"]

[[step]]
id = "net"
title = "Where should it listen?"
intro = "Most people keep the default."
keys = ["server.port"]
when = { key = "mode", equals = "server" }

[[step]]
id = "secrets"
title = "Credentials"
keys = ["api.token"]
"#;

fn config(schema: &Schema) -> configurant::Config {
    // `app.name` is required with no default, so a file sets it, as an installer's seed file would.
    let env = BTreeMap::from([("DEMO_API_TOKEN".to_owned(), CANARY.to_owned())]);
    Loader::new(schema)
        .env(env)
        .file("app.toml", "[app]\nname = \"seed\"\n")
        .build()
        .expect("loads")
}

fn with<R>(f: impl FnOnce(&mut Wizard<'_>) -> R) -> R {
    let schema = Schema::parse(SCHEMA).expect("valid");
    let cfg = config(&schema);
    let mut w = Wizard::new(Driver::new(Session::new(&cfg, None), Needs::default()));
    f(&mut w)
}

fn ev(text: &str) -> Event {
    serde_json::from_str(text).expect("event parses")
}

fn at(w: &Wizard<'_>) -> Option<String> {
    w.view().wizard.expect("wizard view").current
}

#[test]
fn next_walks_the_visible_steps_to_the_review_and_confirm_is_only_there() {
    with(|w| {
        assert_eq!(at(w).as_deref(), Some("name"));
        let r = w.apply(ev(r#"{"event":"confirm"}"#));
        assert!(
            r.refused.is_some() && !r.confirmed,
            "confirm is for the review"
        );
        w.apply(ev(r#"{"event":"next"}"#));
        assert_eq!(at(w).as_deref(), Some("mode"));
        w.apply(ev(r#"{"event":"next"}"#));
        assert_eq!(
            at(w).as_deref(),
            Some("secrets"),
            "net is hidden: mode is client"
        );
        w.apply(ev(r#"{"event":"next"}"#));
        let v = w.view().wizard.expect("wizard view");
        assert!(v.at_review && v.current.is_none() && !v.can_next);
        w.apply(ev(r#"{"event":"back"}"#));
        assert_eq!(at(w).as_deref(), Some("secrets"));
    });
}

#[test]
fn a_hidden_step_is_skipped_and_shows_when_its_condition_holds() {
    with(|w| {
        w.apply(ev(r#"{"event":"set","key":"mode","text":"server"}"#));
        w.apply(ev(r#"{"event":"next"}"#));
        w.apply(ev(r#"{"event":"next"}"#));
        assert_eq!(at(w).as_deref(), Some("net"));
        w.apply(ev(r#"{"event":"back"}"#));
        w.apply(ev(r#"{"event":"set","key":"mode","text":"client"}"#));
        w.apply(ev(r#"{"event":"next"}"#));
        assert_eq!(at(w).as_deref(), Some("secrets"));
        w.apply(ev(r#"{"event":"back"}"#));
        assert_eq!(
            at(w).as_deref(),
            Some("mode"),
            "back skips the hidden step too"
        );
    });
}

#[test]
fn goto_reaches_only_steps_already_reached_and_an_unknown_id_gets_a_suggestion() {
    with(|w| {
        let r = w.apply(ev(r#"{"event":"goto","step":"secrets"}"#));
        assert!(r.refused.is_some());
        assert_eq!(at(w).as_deref(), Some("name"));
        w.apply(ev(r#"{"event":"next"}"#));
        w.apply(ev(r#"{"event":"next"}"#));
        let r = w.apply(ev(r#"{"event":"goto","step":"name"}"#));
        assert!(r.refused.is_none());
        assert_eq!(at(w).as_deref(), Some("name"));
        let r = w.apply(ev(r#"{"event":"goto","step":"secret"}"#));
        assert!(r.refused.expect("refused").error.contains("secrets"));
    });
}

#[test]
fn back_at_the_first_step_is_refused() {
    with(|w| {
        let r = w.apply(ev(r#"{"event":"back"}"#));
        assert!(r.refused.is_some());
        assert_eq!(at(w).as_deref(), Some("name"));
    });
}

#[test]
fn a_plain_driver_refuses_wizard_events() {
    let schema = Schema::parse(SCHEMA).expect("valid");
    let cfg = config(&schema);
    let mut d = Driver::new(Session::new(&cfg, None), Needs::default());
    assert!(d.apply(ev(r#"{"event":"next"}"#)).refused.is_some());
    assert!(d.view().wizard.is_none());
}

#[test]
fn the_review_confirms_and_writes_the_changes() {
    with(|w| {
        w.apply(ev(r#"{"event":"set","key":"app.name","text":"shop"}"#));
        for _ in 0..3 {
            w.apply(ev(r#"{"event":"next"}"#));
        }
        let r = w.apply(ev(r#"{"event":"confirm"}"#));
        assert!(r.confirmed, "{:?}", r.refused);
        let text = w
            .take_confirmed()
            .expect("confirmed")
            .file_text("")
            .expect("text");
        assert!(text.contains("name = \"shop\""), "{text}");
    });
}

#[test]
fn a_secret_step_passes_and_no_secret_or_value_is_in_the_wizard_json_or_state() {
    with(|w| {
        w.apply(ev(r#"{"event":"next"}"#));
        w.apply(ev(r#"{"event":"next"}"#));
        assert_eq!(at(w).as_deref(), Some("secrets"));
        let r = w.apply(ev(r#"{"event":"next"}"#));
        assert!(
            r.refused.is_none(),
            "an all-secret step shows a note and passes"
        );
        let wz = serde_json::to_string(&w.view().wizard).expect("serialises");
        assert!(!wz.contains(CANARY), "{wz}");
        let state = serde_json::to_string(&w.state()).expect("serialises");
        assert!(
            !state.contains(CANARY) && !state.contains("seed"),
            "{state}"
        );
        let all = serde_json::to_string(&w.view()).expect("serialises");
        assert!(!all.contains(CANARY), "{all}");
    });
}

#[test]
fn resume_returns_to_the_saved_step_and_a_stale_id_lands_on_the_first_with_a_note() {
    let saved = with(|w| {
        w.apply(ev(r#"{"event":"next"}"#));
        serde_json::to_string(&w.state()).expect("serialises")
    });
    let schema = Schema::parse(SCHEMA).expect("valid");
    let cfg = config(&schema);
    let state: WizardState = serde_json::from_str(&saved).expect("state parses");
    let w = Wizard::resume(
        Driver::new(Session::new(&cfg, None), Needs::default()),
        state,
    );
    assert_eq!(at(&w).as_deref(), Some("mode"));
    let stale: WizardState =
        serde_json::from_str(r#"{"current":"gone","reached":["gone"]}"#).expect("parses");
    let w = Wizard::resume(
        Driver::new(Session::new(&cfg, None), Needs::default()),
        stale,
    );
    let v = w.view().wizard.expect("wizard view");
    assert_eq!(v.current.as_deref(), Some("name"));
    assert!(v.note.is_some());
}

#[test]
fn the_step_list_says_done_current_todo_and_hidden() {
    with(|w| {
        w.apply(ev(r#"{"event":"next"}"#));
        let v = w.view().wizard.expect("wizard view");
        let states: Vec<(&str, &str)> = v.steps.iter().map(|s| (s.id.as_str(), s.state)).collect();
        assert_eq!(
            states,
            [
                ("name", "done"),
                ("mode", "current"),
                ("net", "hidden"),
                ("secrets", "todo")
            ]
        );
        assert_eq!(v.intro, "");
        assert_eq!(v.keys, ["mode"]);
    });
}
