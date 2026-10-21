// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad inputs. Written before the code they test: each must be REFUSED, with a named error,
//! and none may leak a secret.

use std::collections::BTreeMap;

use configurant::{
    Choice, ChoiceList, Config, ConfigError, DEFAULT_ORDER, Errors, Layer, Loader, Modules, Origin,
    Schema, SchemaError,
};

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1
env_prefix = "DEMO_"

[[value]]
key = "storage.backend"
type = "enum"
choices = ["sqlite", "redb"]
default = "sqlite"
description = "where records live"
env = "DEMO_BACKEND"
flag = "--backend"
restart = true

[[value]]
key = "server.port"
type = "int"
default = 8080
min = 1
max = 65535
description = "listen port"
env = "DEMO_PORT"
flag = "--port"

[[value]]
key = "db.password"
type = "string"
secret = true
description = "database password"
env = "DEMO_DB_PASSWORD"

[[value]]
key = "log.verbose"
type = "bool"
default = false
description = "chatty logs"
flag = "--verbose"
"#;

const SECRET: &str = "hunter2-very-secret";

fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("the demo schema is valid")
}

fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

fn errors_of(r: Result<Config, Errors>) -> Vec<ConfigError> {
    r.expect_err("the input must be refused").0
}

fn text_of(errors: &[ConfigError]) -> String {
    errors.iter().map(|e| format!("{e} | {e:?}\n")).collect()
}

// ---- unknown keys -------------------------------------------------------------------------

#[test]
fn unknown_key_in_a_file_is_an_error_with_a_suggestion() {
    let s = schema();
    let errs = errors_of(
        Loader::new(&s)
            .file("app.toml", "[server]\nprot = 9000\n")
            .build(),
    );
    assert!(
        errs.iter().any(|e| matches!(e,
            ConfigError::UnknownKey { key, suggestion: Some(sug), origin: Origin::File(f) }
            if key == "server.prot" && sug == "server.port" && f == "app.toml")),
        "{errs:?}"
    );
}

#[test]
fn unknown_prefixed_env_var_is_an_error_but_unrelated_vars_are_ignored() {
    let s = schema();
    let errs = errors_of(
        Loader::new(&s)
            .env(env(&[("DEMO_PROT", "1"), ("PATH", "/bin")]))
            .build(),
    );
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(
        matches!(&errs[0], ConfigError::UnknownKey { key, suggestion: Some(sug), .. }
        if key == "DEMO_PROT" && sug == "DEMO_PORT")
    );
}

#[test]
fn unknown_cli_flag_is_an_error_with_a_suggestion() {
    let s = schema();
    let errs = errors_of(Loader::new(&s).cli(args(&["--prot=9"])).build());
    assert!(
        matches!(&errs[0], ConfigError::UnknownKey { key, suggestion: Some(sug), origin: Origin::Cli(_) }
        if key == "--prot" && sug == "--port"),
        "{errs:?}"
    );
}

// ---- wrong type, range, choices ------------------------------------------------------------

#[test]
fn wrong_type_is_named_for_every_source() {
    let s = schema();
    let from_file = errors_of(
        Loader::new(&s)
            .file("a.toml", "[server]\nport = \"eighty\"\n")
            .build(),
    );
    assert!(
        matches!(&from_file[0], ConfigError::WrongType { key, expected, .. }
        if key == "server.port" && *expected == "int")
    );
    let from_env = errors_of(Loader::new(&s).env(env(&[("DEMO_PORT", "abc")])).build());
    assert!(
        matches!(&from_env[0], ConfigError::WrongType { origin: Origin::Env(n), .. } if n == "DEMO_PORT")
    );
    let from_cli = errors_of(Loader::new(&s).cli(args(&["--port", "abc"])).build());
    assert!(matches!(
        &from_cli[0],
        ConfigError::WrongType {
            origin: Origin::Cli(_),
            ..
        }
    ));
}

#[test]
fn out_of_range_and_not_allowed_are_refused() {
    let s = schema();
    let errs = errors_of(Loader::new(&s).env(env(&[("DEMO_PORT", "70000")])).build());
    assert!(matches!(&errs[0], ConfigError::OutOfRange { key, .. } if key == "server.port"));
    let errs = errors_of(
        Loader::new(&s)
            .env(env(&[("DEMO_BACKEND", "mysql")]))
            .build(),
    );
    assert!(
        matches!(&errs[0], ConfigError::NotAllowed { key, choices, .. }
        if key == "storage.backend" && choices == &["sqlite".to_string(), "redb".to_string()])
    );
}

#[test]
fn every_error_is_reported_not_just_the_first() {
    let s = schema();
    let errs = errors_of(
        Loader::new(&s)
            .file("a.toml", "[server]\nprot = 1\n")
            .env(env(&[("DEMO_PORT", "abc")]))
            .cli(args(&["--nope"]))
            .build(),
    );
    assert_eq!(errs.len(), 3, "{errs:?}");
}

// ---- secrets -------------------------------------------------------------------------------

#[test]
fn a_secret_in_a_file_is_refused_and_the_value_is_not_in_the_error() {
    let s = schema();
    let errs = errors_of(
        Loader::new(&s)
            .file("a.toml", &format!("[db]\npassword = \"{SECRET}\"\n"))
            .build(),
    );
    assert!(
        matches!(&errs[0], ConfigError::SecretInForbiddenSource { key, .. } if key == "db.password")
    );
    assert!(!text_of(&errs).contains(SECRET));
}

#[test]
fn a_secret_on_a_command_line_is_refused_and_the_value_is_not_in_the_error() {
    let s = schema();
    for a in [
        vec![format!("--db.password={SECRET}")],
        vec!["--db.password".to_string(), SECRET.to_string()],
        vec![SECRET.to_string()],
    ] {
        let errs = errors_of(Loader::new(&s).cli(a).build());
        assert!(!errs.is_empty());
        assert!(!text_of(&errs).contains(SECRET), "{errs:?}");
    }
}

#[test]
fn a_wrong_typed_secret_does_not_leak_through_the_type_error() {
    let s = SCHEMA.replace(
        "key = \"db.password\"\ntype = \"string\"",
        "key = \"db.password\"\ntype = \"int\"",
    );
    let s = Schema::parse(&s).unwrap();
    let errs = errors_of(
        Loader::new(&s)
            .env(env(&[("DEMO_DB_PASSWORD", SECRET)]))
            .build(),
    );
    assert!(matches!(&errs[0], ConfigError::WrongType { .. }));
    assert!(!text_of(&errs).contains(SECRET), "{errs:?}");
}

#[test]
fn a_secret_from_the_environment_works_and_never_appears_in_any_output() {
    let s = schema();
    let cfg = Loader::new(&s)
        .env(env(&[("DEMO_DB_PASSWORD", SECRET)]))
        .build()
        .unwrap();
    assert_eq!(
        cfg.get::<String>("db.password").unwrap(),
        SECRET,
        "the application can read it"
    );
    let exp = cfg.explain("db.password").unwrap();
    let shown = [
        format!("{cfg:?}"),
        format!("{exp}"),
        format!("{exp:?}"),
        s.help(),
        format!("{s:?}"),
    ]
    .join("\n");
    assert!(!shown.contains(SECRET), "{shown}");
    assert!(format!("{exp}").contains("<redacted>"));
}

// ---- precedence ----------------------------------------------------------------------------

fn precedence_holds(build: impl Fn(&Schema) -> Config) -> Result<(), String> {
    let s = schema();
    let cfg = build(&s);
    let got = cfg.get::<i64>("server.port").map_err(|e| e.to_string())?;
    if got == 3 {
        Ok(())
    } else {
        Err(format!(
            "cli (3) must win over env, file and default, got {got}"
        ))
    }
}

fn all_layers(s: &Schema, order: &[Layer]) -> Config {
    Loader::new(s)
        .file("a.toml", "[server]\nport = 1\n")
        .env(env(&[("DEMO_PORT", "2")]))
        .cli(args(&["--port", "3"]))
        .build_with_order(order)
        .unwrap()
}

#[test]
fn cli_beats_env_beats_file_beats_default() {
    precedence_holds(|s| all_layers(s, &DEFAULT_ORDER)).unwrap();
    let s = schema();
    let only = |l: Loader<'_>| l.build().unwrap().get::<i64>("server.port").unwrap();
    assert_eq!(only(Loader::new(&s)), 8080);
    assert_eq!(
        only(Loader::new(&s).file("a.toml", "[server]\nport = 1\n")),
        1
    );
    assert_eq!(
        only(
            Loader::new(&s)
                .file("a.toml", "[server]\nport = 1\n")
                .env(env(&[("DEMO_PORT", "2")]))
        ),
        2
    );
}

#[test]
fn an_inverted_loader_fails_the_precedence_check() {
    // The control: the same check, run against a deliberately inverted order, must report failure.
    let inverted = [Layer::Cli, Layer::Env, Layer::File, Layer::Default];
    assert!(precedence_holds(|s| all_layers(s, &inverted)).is_err());
}

// ---- explain, help --------------------------------------------------------------------------

#[test]
fn explain_lists_every_layer_that_set_the_key_and_marks_the_winner() {
    let s = schema();
    let cfg = Loader::new(&s)
        .file("a.toml", "[server]\nport = 1\n")
        .env(env(&[("DEMO_PORT", "2")]))
        .build()
        .unwrap();
    let e = cfg.explain("server.port").unwrap();
    let origins: Vec<_> = e.chain.iter().map(|l| (l.origin.clone(), l.wins)).collect();
    assert_eq!(
        origins,
        vec![
            (Origin::Default, false),
            (Origin::File("a.toml".into()), false),
            (Origin::Env("DEMO_PORT".into()), true),
        ]
    );
    assert!(format!("{e}").contains("DEMO_PORT"));
    assert!(matches!(
        cfg.explain("server.prot"),
        Err(ConfigError::UnknownKey { .. })
    ));
}

#[test]
fn help_lists_flags_and_never_a_secret() {
    let h = schema().help();
    assert!(h.contains("--port") && h.contains("DEMO_PORT") && h.contains("8080"));
    assert!(h.contains("DEMO_DB_PASSWORD") && h.contains("secret"));
}

// ---- schema errors ---------------------------------------------------------------------------

fn schema_err(body: &str) -> SchemaError {
    let text = format!("[project]\nname='x'\nversion='1'\nsummary=''\nschema_version=1\n{body}");
    Schema::parse(&text).expect_err("the schema must be refused")
}

#[test]
fn bad_schemas_are_refused() {
    let v = |extra: &str| format!("[[value]]\nkey='a.b'\ntype='int'\ndescription='d'\n{extra}\n");
    assert!(matches!(
        schema_err(&format!("{}{}", v(""), v(""))),
        SchemaError::DuplicateKey(_)
    ));
    assert!(matches!(
        schema_err(&v("secret=true\ndefault=1")),
        SchemaError::SecretWithDefault(_)
    ));
    assert!(matches!(
        schema_err(&v("secret=true\nflag='--a'")),
        SchemaError::SecretWithFlag(_)
    ));
    assert!(matches!(
        schema_err(&v("default='x'")),
        SchemaError::BadDefault { .. }
    ));
    assert!(matches!(
        schema_err(&v("min=5\nmax=1")),
        SchemaError::BadRange(_)
    ));
    assert!(matches!(
        schema_err("[[value]]\nkey='a'\ntype='enum'\ndescription='d'\n"),
        SchemaError::EnumNeedsChoices(_)
    ));
    assert!(matches!(schema_err(&v("bogus=1")), SchemaError::Parse(_)));
    assert!(matches!(
        schema_err(&format!(
            "{}[[value]]\nkey='a.b.c'\ntype='int'\ndescription='d'\n",
            v("")
        )),
        SchemaError::KeyConflict { .. }
    ));
    assert!(matches!(
        Schema::parse("[project]\nname='x'\nversion='1'\nsummary=''\nschema_version=2\n"),
        Err(SchemaError::UnsupportedVersion(2))
    ));
}

// ---- modules -----------------------------------------------------------------------------------

const PROVIDER_SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "storage.backend"
type = "string"
default = "sqlite"
description = "chosen from the storage module's list"
env = "BACKEND"
provider = "storage"
"#;

#[test]
fn a_provider_value_needs_its_module_and_is_checked_by_it() {
    let s = Schema::parse(PROVIDER_SCHEMA).unwrap();

    // Absent module: named error, and the default alone still loads.
    assert!(Loader::new(&s).build().is_ok());
    let errs = errors_of(Loader::new(&s).env(env(&[("BACKEND", "redb")])).build());
    assert!(matches!(&errs[0], ConfigError::ModuleMissing { module, .. } if module == "storage"));

    let mut modules = Modules::new();
    modules
        .register(Box::new(ChoiceList::new(
            "storage",
            vec![
                Choice::new("sqlite", "embedded, durable"),
                Choice::new("redb", "pure Rust"),
            ],
        )))
        .unwrap();
    let ok = Loader::new(&s)
        .modules(&modules)
        .env(env(&[("BACKEND", "redb")]))
        .build()
        .unwrap();
    assert_eq!(ok.get::<String>("storage.backend").unwrap(), "redb");
    let errs = errors_of(
        Loader::new(&s)
            .modules(&modules)
            .env(env(&[("BACKEND", "mysql")]))
            .build(),
    );
    assert!(matches!(&errs[0], ConfigError::ModuleRejected { module, .. } if module == "storage"));
}
