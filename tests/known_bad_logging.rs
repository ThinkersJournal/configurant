// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `logging` module (feature `logging`, docs/MODULES.md section 4, rules L1 to L3). Known-bad first: every
//! row of the design is a value that must fail to load, with the accepted control beside it.

#![cfg(feature = "logging")]

use std::collections::BTreeMap;

use configurant::modules::logging::{Level, Logging};
use configurant::{ConfigError, Loader, Modules, Origin, Schema};

fn schema_with(ty: &str, extra: &str) -> String {
    schema_default(ty, extra, "default = \"info\"")
}

fn schema_default(ty: &str, extra: &str, default: &str) -> String {
    format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "log"
type = "{ty}"
{default}
description = "what to log"
env = "LOG"
provider = "logging"
{extra}
"#
    )
}

fn modules(l: Logging) -> Modules {
    let mut m = Modules::new();
    m.register(Box::new(l)).unwrap();
    m
}

/// The text of the module's refusal of `value` given as the environment string, or `None` if it loaded.
fn refusal_in(schema: &str, l: Logging, value: &str) -> Option<String> {
    let s = Schema::parse(schema).unwrap();
    let m = modules(l);
    let env = BTreeMap::from([("LOG".to_owned(), value.to_owned())]);
    match Loader::new(&s).modules(&m).env(env).build() {
        Ok(_) => None,
        Err(e) => {
            // Every error is the module's; the one that matters is about the value we set in the environment.
            assert!(
                e.0.iter().all(
                    |x| matches!(x, ConfigError::ModuleRejected { module, .. } if module == "logging")
                ),
                "{e:?}"
            );
            let ours =
                e.0.iter()
                    .find(|x| {
                        matches!(
                            x,
                            ConfigError::ModuleRejected {
                                origin: Origin::Env(_),
                                ..
                            }
                        )
                    })
                    .unwrap_or_else(|| panic!("no refusal of the environment value: {e:?}"));
            Some(ours.to_string())
        }
    }
}

fn refusal(l: Logging, value: &str) -> Option<String> {
    refusal_in(&schema_with("string", ""), l, value)
}

// ---- L1: a level ----

#[test]
fn l1_a_bad_level_is_refused() {
    for bad in ["verbose", "warning", "", "inf", "  ", "3", "info!", "inf o"] {
        assert!(refusal(Logging::new(), bad).is_some(), "{bad:?} loaded");
        assert!(
            refusal(Logging::new().levels_only(), bad).is_some(),
            "{bad:?} loaded as a level"
        );
    }
}

#[test]
fn l1_every_level_loads_in_any_case() {
    for ok in [
        "off", "error", "warn", "info", "debug", "trace", "INFO", "Debug", "tRaCe", " warn ",
    ] {
        assert!(refusal(Logging::new(), ok).is_none(), "{ok:?}");
        assert!(
            refusal(Logging::new().levels_only(), ok).is_none(),
            "{ok:?}"
        );
    }
}

#[test]
fn l1_a_refusal_suggests_the_level_it_probably_meant() {
    let why = refusal(Logging::new(), "inf").unwrap();
    assert!(
        why.contains("L1") && why.contains("did you mean `info`"),
        "{why}"
    );
    let why = refusal(Logging::new(), "warn,inf").unwrap();
    assert!(
        why.contains("L1") && why.contains("did you mean `info`"),
        "{why}"
    );
    let why = refusal(Logging::new(), "warning").unwrap();
    assert!(why.contains("did you mean `warn`"), "{why}");
    let why = refusal(Logging::new().levels_only(), "verbose").unwrap();
    assert!(
        why.contains("off, error, warn, info, debug, trace"),
        "{why}"
    );
}

#[test]
fn l1_an_enum_key_is_checked_too_and_other_types_are_refused() {
    let en = schema_with("enum", "choices = [\"info\", \"loud\"]");
    assert!(refusal_in(&en, Logging::new(), "info").is_none());
    // `loud` is a choice of the enum, so the enum accepts it and the module still refuses it.
    assert!(refusal_in(&en, Logging::new(), "loud").is_some());
    let int = "[project]\nname=\"d\"\nversion=\"0.1.0\"\nsummary=\"\"\nschema_version=1\n\
               [[value]]\nkey=\"log\"\ntype=\"int\"\ndefault=3\ndescription=\"d\"\nenv=\"LOG\"\nprovider=\"logging\"\n";
    let why = refusal_in(int, Logging::new(), "5").unwrap();
    assert!(
        why.contains("expected a level or a filter as text"),
        "{why}"
    );
}

// ---- L2: a filter ----

#[test]
fn l2_a_bad_filter_is_refused() {
    for bad in [
        "synapse=loud",
        "=debug",
        "a b=info",
        "info,debug",
        "warn,,info=debug",
        "warn,",
        ",warn",
        "synapse",
        "a::=info",
        "::a=info",
        "a::::b=info",
        "9a=info",
        "a-b=info",
        "a=",
        "a=info=debug",
        "a::b=info,c d=warn",
        "info,warn,error",
    ] {
        assert!(refusal(Logging::new(), bad).is_some(), "{bad:?} loaded");
    }
}

#[test]
fn l2_good_filters_load() {
    for ok in [
        "warn,my_crate=debug",
        "my_crate::net=trace",
        "off,synapse::auth=info,synapse::net=WARN",
        "a=info,b=debug",
        " info , a::b_c = debug ",
        "_x::y1=error",
    ] {
        assert!(refusal(Logging::new(), ok).is_none(), "{ok:?}");
    }
}

#[test]
fn l2_a_filter_is_refused_when_only_a_level_is_wanted() {
    assert!(refusal(Logging::new(), "warn,a=debug").is_none());
    assert!(refusal(Logging::new().levels_only(), "warn,a=debug").is_some());
    assert!(refusal(Logging::new().levels_only(), "a=debug").is_some());
}

#[test]
fn l2_a_bare_word_that_is_a_near_level_is_corrected() {
    let why = refusal(Logging::new(), "warn,warning").unwrap();
    assert!(why.contains("did you mean `warn`"), "{why}");
    let why = refusal(Logging::new(), "synapse").unwrap();
    assert!(why.contains("L2") && !why.contains("did you mean"), "{why}");
}

// ---- L3: a ceiling ----

#[test]
fn l3_a_level_above_the_ceiling_is_refused() {
    let cap = || Logging::new().max_level(Level::Info);
    for bad in [
        "trace",
        "debug",
        "TRACE",
        "warn,a=trace",
        "debug,a=info",
        "a=debug",
    ] {
        let why = refusal(cap(), bad).unwrap_or_else(|| panic!("{bad:?} loaded"));
        assert!(why.contains("L3"), "{bad:?}: {why}");
    }
    for ok in ["info", "warn", "off", "error", "info,a=warn", "a::b=info"] {
        assert!(refusal(cap(), ok).is_none(), "{ok:?}");
    }
    // The same text loads without the ceiling.
    assert!(refusal(Logging::new(), "trace").is_none());
}

#[test]
fn l3_the_ceiling_holds_for_a_single_level_key_too() {
    let cap = || Logging::new().levels_only().max_level(Level::Warn);
    let schema = schema_default("string", "", "default = \"warn\"");
    assert!(refusal_in(&schema, cap(), "info").is_some());
    assert!(refusal_in(&schema, cap(), "warn").is_none());
}

// ---- choices ----

#[test]
fn choices_run_from_least_to_most_verbose_each_with_a_reason() {
    use configurant::{Module, Needs};
    let all = Logging::new().list_choices(&Needs::default());
    let ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["off", "error", "warn", "info", "debug", "trace"]);
    assert!(all.iter().all(|c| c.reason.starts_with("what you see:")));
    // The ceiling hides what the key would refuse.
    let capped = Logging::new()
        .max_level(Level::Info)
        .list_choices(&Needs::default());
    assert_eq!(capped.last().unwrap().id, "info");
    assert_eq!(capped.len(), 4);
}

// ---- a secret never leaks ----

#[test]
fn a_secret_key_holding_a_bad_value_leaks_nothing() {
    let schema = schema_default("string", "secret = true", "");
    for bad in [
        "TOPSECRET-hunter2",
        "a=TOPSECRET",
        "warn,a=TOPSECRET",
        "TOPSECRET::x y=info",
    ] {
        let s = Schema::parse(&schema).unwrap();
        let m = modules(Logging::new());
        let env = BTreeMap::from([("LOG".to_owned(), bad.to_owned())]);
        let errors = Loader::new(&s).modules(&m).env(env).build().unwrap_err();
        let shown = format!("{errors:?} {} {:?}", errors.0[0], errors.0[0]);
        assert!(!shown.contains("TOPSECRET"), "{bad}: {shown}");
        assert!(shown.contains("<redacted>"), "{shown}");
    }
}

#[test]
fn a_refusal_cannot_carry_a_control_character_to_a_terminal() {
    let why = refusal(Logging::new(), "a\u{1b}[31m=info").unwrap();
    assert!(!why.contains('\u{1b}'), "{why:?}");
    // The same when the text is long enough to be cut short.
    let why = refusal(Logging::new(), &format!("\u{1b}[31m{}", "x".repeat(100))).unwrap();
    assert!(!why.contains('\u{1b}'), "{why:?}");
    let long = "x".repeat(500);
    let why = refusal(Logging::new(), &long).unwrap();
    assert!(why.len() < 300, "{} bytes", why.len());
}

#[test]
fn the_documented_refusal_text_is_what_a_user_sees() {
    let cap = Logging::new().max_level(Level::Debug);
    assert_eq!(
        refusal(cap, "trace").unwrap(),
        "`log` from environment variable `LOG` was rejected by module `logging`: logging L3: level `trace` is more \
         verbose than this key allows (at most `debug`)"
    );
}
