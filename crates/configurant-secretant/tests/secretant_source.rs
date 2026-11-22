// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `secretant` secret source (`docs/SECRET-SOURCES.md` slice 4). Every failing case plants the canary in the
//! store, so a leak into an error would show. Assertions are on booleans: a failing assert must never print the
//! canary.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use configurant::{Loader, Schema};
use configurant_secretant::SecretAntSource;
use configurant_secrets::{ResolveError, SecretSource, SourceFailure, Sources, resolve};
use secretant::{FileStore, KdfParams, ReadConfig, ReadUnlock, SecretValue, Store};

const CANARY: &str = "hunter2-canary";
const PASS: &str = "a long passphrase";
const WRONG: &str = "wrong-pass-canary";

fn pass(text: &str) -> SecretValue {
    SecretValue::from_string(text.to_owned())
}

/// A store under `dir` holding `entries`, made with cheap key-derivation costs (tests only).
fn store(dir: &Path, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
    let path = dir.join("store.json");
    let cheap = KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    };
    let mut s = FileStore::create_with_params(&path, &pass(PASS), cheap).unwrap();
    for (name, bytes) in entries {
        s.put(name, SecretValue::new(bytes.to_vec()), BTreeMap::new())
            .unwrap();
    }
    path
}

fn config(path: &Path, passphrase: &str) -> ReadConfig {
    ReadConfig {
        path: path.to_owned(),
        unlock: ReadUnlock::Passphrase(pass(passphrase)),
        timeout: Duration::from_secs(30),
    }
}

fn source(path: &Path) -> SecretAntSource {
    SecretAntSource::open(config(path, PASS)).unwrap()
}

fn schema_for(reference: &str) -> Schema {
    Schema::parse(&format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1
env_prefix = "DEMO_"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "DEMO_API_TOKEN"
source = "secretant:{reference}"
description = "api token"
"#
    ))
    .expect("valid")
}

fn sources(src: SecretAntSource) -> Sources {
    let mut s = Sources::new();
    s.register(Box::new(src)).unwrap();
    s
}

#[test]
fn a_store_entry_is_the_secret_and_loads_redacted() {
    let d = tempfile::tempdir().unwrap();
    let path = store(d.path(), &[("API_TOKEN", CANARY.as_bytes())]);
    let src = source(&path);
    assert!(src.name() == "secretant");
    assert!(src.fetch("API_TOKEN").is_ok_and(|v| v.expose() == CANARY));

    let schema = schema_for("API_TOKEN");
    let env = BTreeMap::new();
    let resolved = resolve(&schema, &sources(src), &env, Duration::from_secs(30)).unwrap();
    assert!(!format!("{resolved:?}").contains(CANARY));
    let c = resolved
        .into_loader(Loader::new(&schema).env(env))
        .build()
        .unwrap();
    assert!(c.get::<String>("api.token").unwrap() == CANARY);
    let e = c.explain("api.token").unwrap();
    assert!(e.chain.iter().any(|l| l.origin.to_string()
        == "secret source `secretant` (reference `API_TOKEN`)"
        && l.shown == "<redacted>"));
    assert!(!format!("{e:?}").contains(CANARY));
    assert!(!format!("{c:?}").contains(CANARY));
}

#[test]
fn a_rotated_value_is_seen_on_the_next_fetch() {
    let d = tempfile::tempdir().unwrap();
    let path = store(d.path(), &[("K", b"first")]);
    let src = source(&path);
    assert!(src.fetch("K").is_ok_and(|v| v.expose() == "first"));
    let mut s = FileStore::open(&path, &pass(PASS)).unwrap();
    s.put("K", SecretValue::new(b"second".to_vec()), BTreeMap::new())
        .unwrap();
    assert!(src.fetch("K").is_ok_and(|v| v.expose() == "second"));
}

#[test]
fn the_environment_still_wins_and_the_store_is_not_asked() {
    let d = tempfile::tempdir().unwrap();
    // The entry would be a failure (binary) if it were read.
    let path = store(d.path(), &[("API_TOKEN", &[0xff, 0xfe])]);
    let schema = schema_for("API_TOKEN");
    let env = BTreeMap::from([("DEMO_API_TOKEN".to_owned(), "from-env".to_owned())]);
    let resolved = resolve(
        &schema,
        &sources(source(&path)),
        &env,
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(resolved.is_empty());
}

#[test]
fn every_failure_is_a_named_kind_and_never_carries_the_canary() {
    let d = tempfile::tempdir().unwrap();
    let path = store(
        d.path(),
        &[("good", CANARY.as_bytes()), ("binary", &[0xff, 0xfe, 0x00])],
    );
    let src = source(&path);

    assert!(src.fetch("absent").err() == Some(SourceFailure::NotFound));
    assert!(src.fetch("binary").err() == Some(SourceFailure::Invalid));

    // A wrong passphrase: refused at open, a kind only.
    let denied = SecretAntSource::open(config(&path, WRONG)).err();
    assert!(denied == Some(SourceFailure::Denied));

    // A store that is not there, and one that is damaged.
    let gone = SecretAntSource::open(config(&d.path().join("nope.json"), PASS)).err();
    assert!(gone == Some(SourceFailure::Unreachable));
    let bad = d.path().join("bad.json");
    std::fs::write(&bad, format!("{{\"junk\": \"{CANARY}\"}}")).unwrap();
    assert!(SecretAntSource::open(config(&bad, PASS)).err() == Some(SourceFailure::Unreachable));

    // Through `resolve`: the error names key, source and reference, and nothing of the store.
    let schema = schema_for("absent");
    let e = resolve(
        &schema,
        &sources(source(&path)),
        &BTreeMap::new(),
        Duration::from_secs(30),
    )
    .unwrap_err();
    assert!(matches!(
        &e.0[0],
        ResolveError::Failed { key, source_name, reference, kind: SourceFailure::NotFound }
            if key == "api.token" && source_name == "secretant" && reference == "absent"
    ));
    let text = format!("{e} {e:?}");
    assert!(!text.contains(CANARY));
    assert!(!text.contains(PASS));
    assert!(!text.contains(WRONG));
}

#[test]
fn a_malformed_name_is_refused_before_any_read() {
    let d = tempfile::tempdir().unwrap();
    let path = store(d.path(), &[("good", CANARY.as_bytes())]);
    let src = source(&path);
    for bad in ["", "a\nb", "a\0b", "tab\there"] {
        assert!(!src.check_reference(bad), "{bad:?} must be refused");
        assert!(src.fetch(bad).err() == Some(SourceFailure::NotFound));
    }
    assert!(src.check_reference("good"));
    assert!(src.check_reference("db/password with spaces"));

    let e = resolve(
        &schema_for("a\\u0000b"),
        &sources(source(&path)),
        &BTreeMap::new(),
        Duration::from_secs(30),
    )
    .unwrap_err();
    assert!(matches!(&e.0[0], ResolveError::BadReference { .. }));
}

#[test]
fn debug_shows_nothing_of_the_store_or_its_unlock() {
    let d = tempfile::tempdir().unwrap();
    let path = store(d.path(), &[("good", CANARY.as_bytes())]);
    let src = source(&path);
    let shown = format!("{src:?}");
    assert!(shown == "SecretAntSource(<redacted>)");
    assert!(!shown.contains(PASS));
    assert!(!shown.contains(&path.display().to_string()));
}
