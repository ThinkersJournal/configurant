// SPDX-License-Identifier: MIT OR Apache-2.0
//! The serialisable view-model: a renderer on any platform gets `View` as JSON and sends `Event`s back.
//! It must carry what a form needs (prompt, example, hint, choices), never a secret's value, refuse bad
//! edits with a named error, and write nothing until a `confirm`.

use std::collections::BTreeMap;

use configurant::edit::Session;
use configurant::view::{Driver, Event};
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
key = "server.port"
type = "int"
default = 8080
min = 1
max = 65535
prompt = "Which port should the app listen on?"
example = "8080"
hint = "Use 8080 unless another program has it."
description = "listen port"

[[value]]
key = "log.level"
type = "enum"
choices = ["info", "debug"]
default = "info"
description = "how much to log"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "DEMO_API_TOKEN"
description = "api token"
"#;

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("serialises")
}

fn with<R>(f: impl FnOnce(&mut Driver<'_>) -> R) -> R {
    let schema = Schema::parse(SCHEMA).expect("valid");
    let env = BTreeMap::from([("DEMO_API_TOKEN".to_owned(), CANARY.to_owned())]);
    let cfg = Loader::new(&schema).env(env).build().expect("loads");
    let mut d = Driver::new(Session::new(&cfg, None), Needs::default());
    f(&mut d)
}

fn ev(text: &str) -> Event {
    serde_json::from_str(text).expect("event parses")
}

#[test]
fn the_view_carries_what_a_form_needs() {
    with(|d| {
        let v = json(&d.view());
        assert!(v.contains("Which port should the app listen on?"), "{v}");
        assert!(v.contains(r#""example":"8080""#), "{v}");
        assert!(v.contains("Use 8080 unless another program has it."), "{v}");
        assert!(v.contains(r#""id":"debug""#), "{v}");
    });
}

#[test]
fn a_secret_value_is_in_no_view_and_no_reply() {
    with(|d| {
        let v = json(&d.view());
        assert!(v.contains("<redacted>"), "{v}");
        assert!(!v.contains(CANARY), "{v}");
        let r = json(&d.apply(ev(
            r#"{"event":"set","key":"api.token","text":"canary-typed"}"#,
        )));
        assert!(!r.contains(CANARY) && !r.contains("canary-typed"), "{r}");
    });
}

#[test]
fn setting_a_secret_is_refused_and_stages_nothing() {
    with(|d| {
        let r = d.apply(ev(r#"{"event":"set","key":"api.token","text":"x"}"#));
        assert_eq!(
            r.refused.as_ref().map(|f| f.key.as_str()),
            Some("api.token")
        );
        assert!(r.view.review.changes.is_empty());
    });
}

#[test]
fn a_bad_value_is_refused_with_a_named_error_and_the_view_is_unchanged() {
    with(|d| {
        let before = json(&d.view());
        let r = d.apply(ev(r#"{"event":"set","key":"server.port","text":"70000"}"#));
        let refused = r.refused.expect("refused");
        assert_eq!(refused.key, "server.port");
        assert!(refused.error.contains("server.port"), "{}", refused.error);
        assert_eq!(json(&r.view), before);
    });
}

#[test]
fn an_unknown_key_is_refused_with_a_suggestion() {
    with(|d| {
        let r = d.apply(ev(r#"{"event":"set","key":"server.prot","text":"1"}"#));
        assert!(r.refused.expect("refused").error.contains("server.port"));
    });
}

#[test]
fn set_then_unset_shows_then_clears_the_change() {
    with(|d| {
        let r = d.apply(ev(r#"{"event":"set","key":"server.port","text":"9000"}"#));
        assert!(r.refused.is_none());
        assert_eq!(r.view.review.changes.len(), 1);
        assert_eq!(r.view.review.changes[0].to, "9000");
        let r = d.apply(ev(r#"{"event":"unset","key":"server.port"}"#));
        assert!(r.view.review.changes.is_empty());
    });
}

#[test]
fn nothing_is_confirmed_until_a_confirm_event_with_a_change() {
    with(|d| {
        assert!(d.take_confirmed().is_none());
        let r = d.apply(ev(r#"{"event":"confirm"}"#));
        assert!(
            !r.confirmed && r.refused.is_some(),
            "an empty review cannot be confirmed"
        );
        assert!(d.take_confirmed().is_none());
        d.apply(ev(r#"{"event":"set","key":"server.port","text":"9000"}"#));
        assert!(
            d.take_confirmed().is_none(),
            "staging alone confirms nothing"
        );
        let r = d.apply(ev(r#"{"event":"confirm"}"#));
        assert!(r.confirmed);
        let text = d
            .take_confirmed()
            .expect("confirmed")
            .file_text("")
            .expect("text");
        assert!(text.contains("port = 9000"), "{text}");
        assert!(d.take_confirmed().is_none(), "taken once");
    });
}

#[test]
fn an_unknown_event_does_not_parse() {
    assert!(serde_json::from_str::<Event>(r#"{"event":"explode"}"#).is_err());
    assert!(serde_json::from_str::<Event>(r#"{"event":"set","key":"a"}"#).is_err());
}
