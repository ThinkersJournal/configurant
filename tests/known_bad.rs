// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad inputs. Written before the code they test: each must be REFUSED, with a named error,
//! and none may leak a secret.

use std::collections::BTreeMap;

use configurant::{
    Choice, ChoiceList, Config, ConfigError, DEFAULT_ORDER, Errors, Layer, Loader, MigrateError,
    Modules, Origin, Schema, SchemaError,
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

// ---- struct binding ----------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct Settings {
    storage: StorageSettings,
    server: ServerSettings,
    db: DbSettings,
    log: LogSettings,
}

#[derive(Debug, serde::Deserialize)]
struct StorageSettings {
    backend: String,
}

#[derive(Debug, serde::Deserialize)]
struct ServerSettings {
    port: u16,
}

#[derive(Debug, serde::Deserialize)]
struct DbSettings {
    password: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct LogSettings {
    verbose: bool,
}

#[test]
fn a_config_binds_to_a_struct_with_the_winning_values() {
    let s = schema();
    let cfg = Loader::new(&s)
        .file("a.toml", "[server]\nport = 1\n")
        .env(env(&[("DEMO_PORT", "2"), ("DEMO_DB_PASSWORD", SECRET)]))
        .build()
        .unwrap();
    let st: Settings = cfg.deserialize().unwrap();
    assert_eq!(st.server.port, 2);
    assert_eq!(st.storage.backend, "sqlite");
    assert!(!st.log.verbose);
    assert_eq!(st.db.password.as_deref(), Some(SECRET));
    // An unset key is absent, so an `Option` field is `None`.
    let none: Settings = Loader::new(&s).build().unwrap().deserialize().unwrap();
    assert_eq!(none.db.password, None);
}

#[test]
fn a_struct_that_does_not_fit_is_refused_by_name() {
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct Wrong {
        server: WrongServer,
    }
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct WrongServer {
        port: String,
    }
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct Missing {
        nonesuch: bool,
    }
    let s = schema();
    let cfg = Loader::new(&s).build().unwrap();
    let e = cfg.deserialize::<Wrong>().expect_err("int into String");
    assert!(matches!(e, ConfigError::Deserialize { .. }));
    assert!(e.to_string().contains("string"), "{e}");
    let e = cfg.deserialize::<Missing>().expect_err("missing field");
    assert!(e.to_string().contains("nonesuch"), "{e}");
}

#[test]
fn binding_never_leaks_a_secret_into_its_error() {
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct Db {
        db: DbPassword,
    }
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct DbPassword {
        password: u16,
    }
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct Variant {
        db: VariantPassword,
    }
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    enum Mode {
        Other,
    }
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct VariantPassword {
        password: Mode,
    }
    let s = schema();
    let cfg = Loader::new(&s)
        .env(env(&[("DEMO_DB_PASSWORD", SECRET)]))
        .build()
        .unwrap();
    // The secret is a string; the target wants a u16, and serde would quote the string it found.
    let e = cfg.deserialize::<Db>().expect_err("string into u16");
    let text = format!("{e} | {e:?}");
    assert!(!text.contains(SECRET), "leaked: {text}");
    assert!(matches!(e, ConfigError::Deserialize { .. }));
    // serde also names an unknown enum variant by its text.
    let e = cfg.deserialize::<Variant>().expect_err("unknown variant");
    let text = format!("{e} | {e:?}");
    assert!(!text.contains(SECRET), "leaked: {text}");
}

// ---- migrations ----------------------------------------------------------------------------------

fn migration_schema_text(config_version: u32, migrations: &str) -> String {
    format!(
        "{}{migrations}",
        SCHEMA.replace(
            "env_prefix = \"DEMO_\"",
            &format!("env_prefix = \"DEMO_\"\nconfig_version = {config_version}")
        )
    )
}

const MIGRATIONS: &str = r#"
[[migration]]
from = 1
to = 2
ops = [{ op = "rename", from = "server.listen", to = "server.port" }]

[[migration]]
from = 2
to = 3
ops = [{ op = "map_value", key = "storage.backend", map = { embedded = "sqlite", pure = "redb" } }]
"#;

fn mig() -> Schema {
    Schema::parse(&migration_schema_text(3, MIGRATIONS)).expect("the migrating schema is valid")
}

const OLD_FILE: &str =
    "# my settings\n[server]\nlisten = 9000 # the port\n\n[storage]\nbackend = \"embedded\"\n";

fn table(text: &str) -> toml::Table {
    text.parse().expect("valid toml")
}

#[test]
fn a_file_is_migrated_up_keeping_comments_and_then_loads() {
    let s = mig();
    let m = s.migrate(OLD_FILE, Some(1)).unwrap();
    assert_eq!((m.from, m.to), (1, 3));
    assert!(m.text.contains("# my settings") && m.text.contains("# the port"));
    assert!(m.text.contains("port = 9000") && !m.text.contains("listen"));
    assert!(m.text.contains("backend = \"sqlite\""), "{}", m.text);
    assert!(m.text.contains("config_version = 3"), "{}", m.text);
    let changes = m.changes.join("\n");
    for key in ["server.listen", "server.port", "storage.backend"] {
        assert!(changes.contains(key), "{changes}");
    }
    assert!(
        !changes.contains("9000") && !changes.contains("embedded"),
        "{changes}"
    );
    let cfg = Loader::new(&s).file("a.toml", &m.text).build().unwrap();
    assert_eq!(cfg.get::<i64>("server.port").unwrap(), 9000);
    assert_eq!(cfg.get::<String>("storage.backend").unwrap(), "sqlite");
}

#[test]
fn a_migrated_file_carries_back_to_what_it_was() {
    let s = mig();
    let up = s.migrate(OLD_FILE, Some(1)).unwrap();
    let back = s.migrate_back(&up.text, 1).unwrap();
    assert_eq!((back.from, back.to), (3, 1));
    let mut got = table(&back.text);
    assert_eq!(got.remove("config_version"), Some(toml::Value::Integer(1)));
    assert_eq!(got, table(OLD_FILE));
    assert!(back.text.contains("# my settings") && back.text.contains("# the port"));
    // One step down, then up again, is where it started.
    let mid = s.migrate_back(&up.text, 2).unwrap();
    assert!(mid.text.contains("backend = \"embedded\"") && mid.text.contains("port = 9000"));
    assert_eq!(
        table(&s.migrate(&mid.text, None).unwrap().text),
        table(&up.text)
    );
}

#[test]
fn the_loader_refuses_a_file_at_another_config_version_by_name() {
    let s = mig();
    let load = |text: &str| Loader::new(&s).file("a.toml", text).build();
    let old = errors_of(load("config_version = 1\n[server]\nlisten = 1\n"));
    assert!(matches!(
        &old[0],
        ConfigError::FileOutdated {
            found: 1,
            current: 3,
            ..
        }
    ));
    assert!(old[0].to_string().contains("migrate"));
    let new = errors_of(load("config_version = 9\n"));
    assert!(matches!(
        &new[0],
        ConfigError::FileNewer {
            found: 9,
            current: 3,
            ..
        }
    ));
    // The current version, and no version at all, load.
    assert!(load("config_version = 3\n[server]\nport = 5\n").is_ok());
    assert!(load("[server]\nport = 5\n").is_ok());
    // A version that is not an integer is a wrong type, not a silent pass.
    let bad = errors_of(load("config_version = \"3\"\n"));
    assert!(matches!(&bad[0], ConfigError::WrongType { key, .. } if key == "config_version"));
}

#[test]
fn version_edges_are_named_errors() {
    let s = mig();
    assert!(matches!(
        s.migrate(OLD_FILE, None),
        Err(MigrateError::VersionMissing)
    ));
    assert!(matches!(
        s.migrate("config_version = 9\n", None),
        Err(MigrateError::Newer {
            found: 9,
            current: 3
        })
    ));
    assert!(matches!(
        s.migrate("config_version = 0\n", None),
        Err(MigrateError::BadVersion)
    ));
    assert!(matches!(
        s.migrate("config_version = \"2\"\n", None),
        Err(MigrateError::BadVersion)
    ));
    assert!(matches!(
        s.migrate_back(OLD_FILE, 1),
        Err(MigrateError::VersionMissing)
    ));
    assert!(matches!(
        s.migrate_back("config_version = 3\n", 7),
        Err(MigrateError::BadTarget { target: 7, .. })
    ));
    assert!(matches!(
        s.migrate("[server\nport=", Some(1)),
        Err(MigrateError::FileParse { .. })
    ));
    // Already current: nothing to do, text untouched.
    let cur = "config_version = 3\n[server]\nport = 1\n";
    let m = s.migrate(cur, None).unwrap();
    assert!(m.changes.is_empty() && m.text == cur);
}

#[test]
fn collisions_and_ambiguity_are_refused_and_no_error_shows_a_value() {
    let s = mig();
    let both = format!("[server]\nlisten = \"{SECRET}\"\nport = \"other-{SECRET}\"\n");
    let e = s
        .migrate(&both, Some(1))
        .expect_err("rename onto an existing key");
    assert!(matches!(&e, MigrateError::DestinationExists { key } if key == "server.port"));
    assert!(!format!("{e} | {e:?}").contains(SECRET));
    // A value the map already produces cannot be told from a migrated one.
    let amb = s.migrate(
        "config_version = 2\n[storage]\nbackend = \"sqlite\"\n",
        None,
    );
    assert!(
        matches!(&amb, Err(MigrateError::Ambiguous { key }) if key == "storage.backend"),
        "{amb:?}"
    );
    // A non-string under a string map.
    let wrong = s
        .migrate("config_version = 2\n[storage]\nbackend = 7\n", None)
        .unwrap_err();
    assert!(
        matches!(
            &wrong,
            MigrateError::WrongType {
                found: "integer",
                ..
            }
        ),
        "{wrong:?}"
    );
    // A value outside the map passes through untouched, both ways.
    let m = s
        .migrate("config_version = 2\n[storage]\nbackend = \"weird\"\n", None)
        .unwrap();
    assert!(m.text.contains("backend = \"weird\""));
    let b = s.migrate_back(&m.text, 2).unwrap();
    assert!(b.text.contains("backend = \"weird\""));
    // A file that does not parse: the error never quotes the file.
    let broken = format!("[server]\nlisten = {SECRET} oops\n");
    let e = s.migrate(&broken, Some(1)).unwrap_err();
    assert!(matches!(e, MigrateError::FileParse { .. }), "{e:?}");
    assert!(!format!("{e} | {e:?}").contains(SECRET), "{e}");
}

#[test]
fn bad_migrations_in_a_schema_are_refused() {
    let parse =
        |cv: u32, m: &str| Schema::parse(&migration_schema_text(cv, m)).expect_err("refused");
    let step = |ops: &str| format!("[[migration]]\nfrom = 1\nto = 2\nops = [{ops}]\n");
    let one = step("{ op = \"rename\", from = \"a\", to = \"b\" }");
    // A gap: config_version 3 but only one step.
    assert!(matches!(parse(3, &one), SchemaError::MigrationChain { .. }));
    // Steps without a version bump, and a version of zero.
    assert!(matches!(parse(1, &one), SchemaError::MigrationChain { .. }));
    assert!(matches!(parse(0, ""), SchemaError::MigrationChain { .. }));
    // Not consecutive.
    let skip = "[[migration]]\nfrom = 1\nto = 3\nops = []\n";
    assert!(matches!(parse(3, skip), SchemaError::BadMigration { .. }));
    // A map that is not one-to-one cannot be carried back.
    let dup = step("{ op = \"map_value\", key = \"a\", map = { x = \"z\", y = \"z\" } }");
    assert!(matches!(parse(2, &dup), SchemaError::BadMigration { .. }));
    let empty = step("{ op = \"map_value\", key = \"a\", map = {} }");
    assert!(matches!(parse(2, &empty), SchemaError::BadMigration { .. }));
    let same = step("{ op = \"rename\", from = \"a\", to = \"a\" }");
    assert!(matches!(parse(2, &same), SchemaError::BadMigration { .. }));
    let badkey = step("{ op = \"rename\", from = \"a..b\", to = \"c\" }");
    assert!(matches!(parse(2, &badkey), SchemaError::BadKey(_)));
    // An operation that loses information is not offered at all.
    let drop = step("{ op = \"drop\", key = \"a\" }");
    assert!(matches!(parse(2, &drop), SchemaError::Parse(_)));
    // The version key is the loader's.
    let reserved = format!(
        "{SCHEMA}\n[[value]]\nkey = \"config_version\"\ntype = \"int\"\ndescription = \"d\"\n"
    );
    assert!(matches!(
        Schema::parse(&reserved),
        Err(SchemaError::ReservedKey(_))
    ));
}

#[test]
fn a_step_with_several_operations_is_undone_in_reverse_order() {
    // a -> b, then b -> c: forward the value ends at c; back it must undo c -> b first, then b -> a.
    let ops = "[[migration]]\nfrom = 1\nto = 2\nops = [{ op = \"rename\", from = \"x.a\", to = \"x.b\" }, { op = \"rename\", from = \"x.b\", to = \"y.c\" }]\n";
    let s = Schema::parse(&migration_schema_text(2, ops)).unwrap();
    let up = s.migrate("[x]\na = 1\n", Some(1)).unwrap();
    assert_eq!(
        table(&up.text),
        table("config_version = 2\n[y]\nc = 1\n"),
        "{}",
        up.text
    );
    let back = s.migrate_back(&up.text, 1).unwrap();
    assert_eq!(
        table(&back.text),
        table("config_version = 1\n[x]\na = 1\n"),
        "{}",
        back.text
    );
}
