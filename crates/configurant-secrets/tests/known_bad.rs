// SPDX-License-Identifier: MIT OR Apache-2.0
//! Secret sources, each rule of `docs/SECRET-SOURCES.md` section 3 against a fake source that returns a canary.
//! Assertions are on booleans: a failing assert must never print the canary.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use configurant::edit::Session;
use configurant::{Config, ConfigError, Loader, Schema, SchemaError};
use configurant_secrets::{
    ResolveError, SecretSource, SecretValue, SourceFailure, Sources, SourcesError, resolve,
};

const CANARY: &str = "hunter2-canary";

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
description = "listen port"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "DEMO_API_TOKEN"
source = "fake:DEMO_API_TOKEN_ENTRY"
description = "api token"

[[value]]
key = "api.pin"
type = "int"
secret = true
source = "fake:PIN_ENTRY"
description = "a numeric pin"
"#;

#[derive(Clone, Copy)]
enum Mode {
    Value,
    Fail(SourceFailure),
    Hang,
    NotANumber,
}

struct Fake {
    mode: Mode,
    asked: Arc<AtomicUsize>,
}

impl SecretSource for Fake {
    fn name(&self) -> &str {
        "fake"
    }
    fn fetch(&self, reference: &str) -> Result<SecretValue, SourceFailure> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        match (self.mode, reference) {
            (Mode::Value, "PIN_ENTRY") => Ok(SecretValue::new("1234")),
            (Mode::Value, _) => Ok(SecretValue::new(CANARY)),
            (Mode::NotANumber, "PIN_ENTRY") => Ok(SecretValue::new(CANARY)),
            (Mode::NotANumber, _) => Ok(SecretValue::new(CANARY)),
            (Mode::Fail(k), _) => Err(k),
            (Mode::Hang, _) => {
                std::thread::sleep(Duration::from_secs(3));
                Ok(SecretValue::new(CANARY))
            }
        }
    }
}

fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("valid")
}

fn sources(mode: Mode) -> (Sources, Arc<AtomicUsize>) {
    let asked = Arc::new(AtomicUsize::new(0));
    let mut s = Sources::new();
    s.register(Box::new(Fake {
        mode,
        asked: Arc::clone(&asked),
    }))
    .unwrap();
    (s, asked)
}

fn load(mode: Mode, env: &[(&str, &str)]) -> Result<Config, String> {
    let schema = schema();
    let env: BTreeMap<String, String> = env
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let (src, _) = sources(mode);
    let resolved =
        resolve(&schema, &src, &env, Duration::from_secs(1)).map_err(|e| e.to_string())?;
    resolved
        .into_loader(Loader::new(&schema).env(env))
        .build()
        .map_err(|e| e.to_string())
}

#[test]
fn a_secret_comes_from_its_source_and_the_environment_overrides_it() {
    let c = load(Mode::Value, &[]).unwrap();
    assert!(c.get::<String>("api.token").unwrap() == CANARY);
    let e = c.explain("api.token").unwrap();
    assert!(e.chain.iter().any(|l| l.origin.to_string()
        == "secret source `fake` (reference `DEMO_API_TOKEN_ENTRY`)"
        && l.shown == "<redacted>"));

    // The environment wins, and the source is never asked for that key (S9).
    let schema = schema();
    let env = BTreeMap::from([("DEMO_API_TOKEN".to_owned(), "from-env".to_owned())]);
    let (src, asked) = sources(Mode::Value);
    let r = resolve(&schema, &src, &env, Duration::from_secs(1)).unwrap();
    let keys: Vec<&str> = r.keys().collect();
    assert!(keys == ["api.pin"], "only the other secret was fetched");
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    let c = r
        .into_loader(Loader::new(&schema).env(env))
        .build()
        .unwrap();
    assert!(c.get::<String>("api.token").unwrap() == "from-env");
}

#[test]
fn a_secret_is_in_no_debug_display_explain_help_or_error() {
    let c = load(Mode::Value, &[]).unwrap();
    let mut seen = format!("{c:?}");
    seen += &format!("{:?}", c.explain("api.token").unwrap());
    seen += &c.schema().help();
    // A secret of the wrong type: the type error must not quote it (S3).
    let wrong = load(Mode::NotANumber, &[]).unwrap_err();
    seen += &wrong;
    let (src, _) = sources(Mode::Value);
    let r = resolve(&schema(), &src, &BTreeMap::new(), Duration::from_secs(1)).unwrap();
    seen += &format!("{r:?} {:?}", SecretValue::new(CANARY));
    assert!(!seen.contains(CANARY));
    assert!(
        wrong.contains("<redacted>"),
        "the type error says so without the value"
    );
}

#[test]
fn a_confirmed_edit_never_writes_a_sourced_secret_to_the_file() {
    let c = load(Mode::Value, &[]).unwrap();
    let mut session = Session::new(&c, None);
    session.stage("server.port", "9001").unwrap();
    let text = session.review().confirm().file_text("").unwrap();
    assert!(text.contains("port = 9001"));
    assert!(!text.contains(CANARY) && !text.contains("api"));
}

#[test]
fn a_failing_source_is_a_named_error_and_never_a_silent_default() {
    for kind in [
        SourceFailure::NotFound,
        SourceFailure::Denied,
        SourceFailure::Unreachable,
    ] {
        let schema = schema();
        let (src, _) = sources(Mode::Fail(kind));
        let e = resolve(&schema, &src, &BTreeMap::new(), Duration::from_secs(1)).unwrap_err();
        assert_eq!(
            e.0.len(),
            2,
            "every failing secret is reported, not the first"
        );
        assert!(
            matches!(&e.0[0], ResolveError::Failed { kind: k, source_name, .. } if *k == kind && source_name == "fake")
        );
        assert!(e.to_string().contains("api.token"));
    }
}

#[test]
fn a_source_that_never_answers_times_out_within_the_bound() {
    let schema = schema();
    let (src, _) = sources(Mode::Hang);
    let start = Instant::now();
    let e = resolve(&schema, &src, &BTreeMap::new(), Duration::from_millis(100)).unwrap_err();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(e.0.iter().all(|x| matches!(
        x,
        ResolveError::Failed {
            kind: SourceFailure::TimedOut,
            ..
        }
    )));
}

#[test]
fn bad_schemas_and_unknown_sources_are_refused_by_name() {
    let with = |extra: &str| {
        format!(
            "{SCHEMA}\n[[value]]\nkey = \"x.y\"\ntype = \"string\"\ndescription = \"d\"\n{extra}\n"
        )
    };
    // A source on a value that is not secret.
    let e = Schema::parse(&with("source = \"fake:ENTRY\"")).unwrap_err();
    assert!(matches!(e, SchemaError::SourceOnNonSecret(k) if k == "x.y"));
    // A malformed reference, on a secret.
    for bad in ["nocolon", ":ref", "name:"] {
        let e = Schema::parse(&with(&format!("secret = true\nsource = \"{bad}\""))).unwrap_err();
        assert!(
            matches!(e, SchemaError::BadSource(k) if k == "x.y"),
            "{bad}"
        );
    }
    // A source name nothing registered.
    let none = Sources::new();
    let e = resolve(&schema(), &none, &BTreeMap::new(), Duration::from_secs(1)).unwrap_err();
    assert!(
        matches!(&e.0[0], ResolveError::UnknownSource { source_name, registered, .. } if source_name == "fake" && registered == "none")
    );
    // Registration refuses a duplicate.
    let (mut s, _) = sources(Mode::Value);
    let dup = Fake {
        mode: Mode::Value,
        asked: Arc::new(AtomicUsize::new(0)),
    };
    assert_eq!(
        s.register(Box::new(dup)),
        Err(SourcesError::Duplicate("fake".into()))
    );
}

#[test]
fn only_a_secret_that_names_a_source_can_be_resolved_and_a_file_still_cannot_carry_one() {
    let schema = schema();
    // The loader refuses a resolved value for a key that is not a secret with a source.
    let e = Loader::new(&schema)
        .resolved(BTreeMap::from([("server.port".to_owned(), "1".to_owned())]))
        .build()
        .unwrap_err();
    assert!(
        e.0.iter()
            .any(|x| matches!(x, ConfigError::UnknownKey { .. }))
    );
    // Both an environment value and a resolved one for the same key: the environment wins (S9).
    let both = Loader::new(&schema)
        .env(BTreeMap::from([(
            "DEMO_API_TOKEN".to_owned(),
            "from-env".to_owned(),
        )]))
        .resolved(BTreeMap::from([(
            "api.token".to_owned(),
            "from-source".to_owned(),
        )]))
        .build()
        .unwrap();
    assert!(both.get::<String>("api.token").unwrap() == "from-env");
    let chain = both.explain("api.token").unwrap().chain;
    assert!(
        chain.len() == 2 && chain[1].wins && !chain[0].wins,
        "source below env, env wins"
    );
    // A file still may not hold a secret, source or no source (S10).
    let e = Loader::new(&schema)
        .file("app.toml", "[api]\ntoken = \"x\"\n")
        .build()
        .unwrap_err();
    assert!(
        e.0.iter()
            .any(|x| matches!(x, ConfigError::SecretInForbiddenSource { .. }))
    );
}
