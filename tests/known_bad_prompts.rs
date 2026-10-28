// SPDX-License-Identifier: MIT OR Apache-2.0
//! Plain-language prompts: a value may carry a `prompt` (the question a wizard asks) and an `example`.
//! A wrong example would mislead the person typing, so it is held to the same rules as a default; a
//! secret may not have one (an example of a secret looks like a secret).

use configurant::edit::Session;
use configurant::{Config, Loader, Schema, SchemaError};

fn schema(extra: &str) -> String {
    format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "server.port"
type = "int"
min = 1
max = 65535
description = "listen port"
{extra}
"#
    )
}

fn err(extra: &str) -> SchemaError {
    Schema::parse(&schema(extra)).expect_err("schema must be refused")
}

#[test]
fn an_example_of_the_wrong_type_is_refused() {
    assert!(matches!(
        err(r#"example = "http""#),
        SchemaError::BadExample { .. }
    ));
}

#[test]
fn an_example_outside_min_max_is_refused() {
    assert!(matches!(
        err(r#"example = "70000""#),
        SchemaError::BadExample { .. }
    ));
}

#[test]
fn an_example_outside_choices_is_refused() {
    let text = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "log.level"
type = "enum"
choices = ["info", "debug"]
example = "loud"
description = "how much to log"
"#;
    assert!(matches!(
        Schema::parse(text).expect_err("refused"),
        SchemaError::BadExample { .. }
    ));
}

#[test]
fn a_secret_may_not_have_an_example() {
    let text = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "api.token"
type = "string"
secret = true
example = "sk-live-1234"
description = "api token"
"#;
    let e = Schema::parse(text).expect_err("refused");
    assert!(matches!(e, SchemaError::SecretWithExample(_)), "{e:?}");
    assert!(!e.to_string().contains("sk-live"));
}

fn config(s: &Schema) -> Config {
    Loader::new(s).build().expect("loads")
}

#[test]
fn a_prompt_and_example_reach_the_field_and_help() {
    let s = Schema::parse(&schema(
        r#"prompt = "Which port should the app listen on?"
example = "8080""#,
    ))
    .expect("parses");
    let cfg = config(&s);
    let f = Session::new(&cfg, None).fields().remove(0);
    assert_eq!(f.prompt, "Which port should the app listen on?");
    assert_eq!(f.example.as_deref(), Some("8080"));
    let help = s.help();
    assert!(help.contains("[example: 8080]"), "{help}");
}

#[test]
fn without_a_prompt_the_description_is_the_question() {
    let s = Schema::parse(&schema("")).expect("parses");
    let cfg = config(&s);
    let f = Session::new(&cfg, None).fields().remove(0);
    assert_eq!(f.prompt, "listen port");
    assert_eq!(f.example, None);
    assert!(!s.help().contains("example"));
}

#[test]
fn an_empty_prompt_is_refused() {
    assert!(matches!(
        err(r#"prompt = "  ""#),
        SchemaError::EmptyPrompt(_)
    ));
}
