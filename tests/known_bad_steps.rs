// SPDX-License-Identifier: MIT OR Apache-2.0
//! Wizard steps in the schema (`docs/STEP-ENGINE.md`, part a): `[[step]]` entries order and group the
//! values, a `when` hides a step by data alone, and a step that names nothing real is a named error.

use configurant::{Schema, SchemaError};

fn schema(steps: &str) -> String {
    format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "mode"
type = "enum"
choices = ["server", "client"]
default = "server"
description = "what to run"

[[value]]
key = "server.port"
type = "int"
default = 8080
description = "listen port"

[[value]]
key = "server.host"
type = "string"
default = "localhost"
description = "listen host"

[[value]]
key = "log.level"
type = "string"
default = "info"
description = "how much to log"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "DEMO_API_TOKEN"
description = "api token"

[[table]]
key = "mount"
description = "mounts"

[[table.field]]
key = "path"
type = "path"
description = "where"
{steps}
"#
    )
}

fn err(steps: &str) -> SchemaError {
    Schema::parse(&schema(steps)).expect_err("refused")
}

const STEPS: &str = r#"
[[step]]
id = "mode"
title = "What do you want to run?"
keys = ["mode"]

[[step]]
id = "network"
title = "Where should it listen?"
intro = "Most people keep the defaults."
keys = ["server.port", "server.host"]
when = { key = "mode", equals = "server" }
"#;

#[test]
fn declared_steps_keep_their_order_and_the_rest_go_to_a_final_step() {
    let s = Schema::parse(&schema(STEPS)).expect("parses");
    let steps = s.effective_steps();
    let ids: Vec<&str> = steps.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(ids, ["mode", "network", "more"]);
    assert_eq!(steps[1].keys, ["server.port", "server.host"]);
    assert_eq!(steps[1].intro, "Most people keep the defaults.");
    assert_eq!(steps[2].title, "More settings");
    assert_eq!(steps[2].keys, ["log.level", "api.token"]);
}

#[test]
fn with_no_steps_each_section_is_a_step() {
    let s = Schema::parse(&schema("")).expect("parses");
    let steps = s.effective_steps();
    let ids: Vec<&str> = steps.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(ids, ["general", "server", "log", "api"]);
    assert_eq!(steps[1].title, "server");
    assert_eq!(steps[1].keys, ["server.port", "server.host"]);
    assert!(steps.iter().all(|x| x.when.is_none()));
}

#[test]
fn a_step_naming_an_unknown_key_or_a_table_is_refused() {
    for key in ["server.prot", "mount"] {
        let e = err(&format!(
            "[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"{key}\"]\n"
        ));
        assert!(
            matches!(e, SchemaError::StepUnknownKey { .. }),
            "{key}: {e:?}"
        );
    }
}

#[test]
fn a_key_in_two_steps_is_refused() {
    let e = err(
        "[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"mode\"]\n[[step]]\nid = \"b\"\ntitle = \"B\"\nkeys = [\"mode\"]\n",
    );
    assert!(matches!(e, SchemaError::StepKeyTwice(_)), "{e:?}");
}

#[test]
fn an_empty_title_or_a_bad_or_repeated_id_is_refused() {
    let e = err("[[step]]\nid = \"a\"\ntitle = \" \"\nkeys = [\"mode\"]\n");
    assert!(matches!(e, SchemaError::EmptyStepTitle(_)), "{e:?}");
    let e = err("[[step]]\nid = \"a b\"\ntitle = \"A\"\nkeys = [\"mode\"]\n");
    assert!(matches!(e, SchemaError::BadStepId(_)), "{e:?}");
    let e = err(
        "[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"mode\"]\n[[step]]\nid = \"a\"\ntitle = \"B\"\nkeys = [\"server.port\"]\n",
    );
    assert!(matches!(e, SchemaError::DuplicateStep(_)), "{e:?}");
}

#[test]
fn a_when_must_name_a_real_key_and_exactly_one_test() {
    let e = err(
        "[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"mode\"]\nwhen = { key = \"nope\", equals = \"x\" }\n",
    );
    assert!(matches!(e, SchemaError::BadWhen { .. }), "{e:?}");
    let e = err(
        "[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"mode\"]\nwhen = { key = \"mode\", equals = \"x\", not_equals = \"y\" }\n",
    );
    assert!(matches!(e, SchemaError::BadWhen { .. }), "{e:?}");
    let e =
        err("[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"mode\"]\nwhen = { key = \"mode\" }\n");
    assert!(matches!(e, SchemaError::BadWhen { .. }), "{e:?}");
}

#[test]
fn visibility_is_decided_by_data_alone() {
    let s = Schema::parse(&schema(STEPS)).expect("parses");
    let steps = s.effective_steps();
    let network = &steps[1];
    let with = |v: Option<&'static str>| move |_: &str| v.map(str::to_owned);
    assert!(network.visible(&with(Some("server"))));
    assert!(!network.visible(&with(Some("client"))));
    assert!(
        !network.visible(&with(None)),
        "equals on an unset key is false"
    );
    assert!(steps[0].visible(&with(None)), "no when is always visible");
}

#[test]
fn not_equals_and_set_conditions() {
    let text = schema(
        "[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"server.port\"]\nwhen = { key = \"mode\", not_equals = \"client\" }\n\
         [[step]]\nid = \"b\"\ntitle = \"B\"\nkeys = [\"log.level\"]\nwhen = { key = \"api.token\", set = true }\n",
    );
    let s = Schema::parse(&text).expect("parses");
    let steps = s.effective_steps();
    let a = |v: Option<&'static str>| move |_: &str| v.map(str::to_owned);
    assert!(
        steps[0].visible(&a(None)),
        "not_equals on an unset key is true"
    );
    assert!(!steps[0].visible(&a(Some("client"))));
    assert!(steps[1].visible(&a(Some("x"))));
    assert!(!steps[1].visible(&a(None)));
}

#[test]
fn a_secret_key_may_be_in_a_step() {
    let e = Schema::parse(&schema(
        "[[step]]\nid = \"a\"\ntitle = \"A\"\nkeys = [\"api.token\"]\n",
    ));
    assert!(e.is_ok(), "{e:?}");
}
