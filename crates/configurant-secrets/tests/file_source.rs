// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `file` secret source (`docs/SECRET-SOURCES.md` slice 3). Every failure case plants the canary in the file, so
//! a leak into an error would show. Assertions are on booleans: a failing assert must never print the canary.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use configurant::{Loader, Schema};
use configurant_secrets::{
    FileSource, ResolveError, SecretSource, SourceFailure, Sources, resolve,
};

const CANARY: &str = "hunter2-canary";

fn put(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    private(&path);
}

#[cfg(unix)]
fn private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[cfg(not(unix))]
fn private(_path: &Path) {}

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
source = "file:{reference}"
description = "api token"
"#
    ))
    .expect("valid")
}

fn fetch(dir: &Path, reference: &str) -> Result<String, SourceFailure> {
    FileSource::new(dir)
        .fetch(reference)
        .map(|v| v.expose().to_owned())
}

fn failure(dir: &Path, reference: &str) -> Option<SourceFailure> {
    FileSource::new(dir).fetch(reference).err()
}

fn file_sources(dir: &Path) -> Sources {
    let mut sources = Sources::new();
    sources.register(Box::new(FileSource::new(dir))).unwrap();
    sources
}

#[test]
fn a_file_in_the_directory_is_the_secret_with_one_trailing_newline_removed() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "plain", CANARY.as_bytes());
    put(d.path(), "lf", format!("{CANARY}\n").as_bytes());
    put(d.path(), "crlf", format!("{CANARY}\r\n").as_bytes());
    put(d.path(), "two", format!("{CANARY}\n\n").as_bytes());
    assert!(fetch(d.path(), "plain").is_ok_and(|s| s == CANARY));
    assert!(fetch(d.path(), "lf").is_ok_and(|s| s == CANARY));
    assert!(fetch(d.path(), "crlf").is_ok_and(|s| s == CANARY));
    // Only one newline goes: a second one is part of the secret.
    assert!(fetch(d.path(), "two").is_ok_and(|s| s == format!("{CANARY}\n")));
}

#[test]
fn a_reference_that_is_not_a_bare_file_name_is_refused_before_any_read() {
    let d = tempfile::tempdir().unwrap();
    let secrets = d.path().join("secrets");
    std::fs::create_dir(&secrets).unwrap();
    // A real, readable secret one level above the directory: a traversal would reach it.
    put(d.path(), "top", CANARY.as_bytes());
    let src = FileSource::new(&secrets);
    let abs = d.path().join("top").display().to_string();
    for bad in [
        "../top",
        "..\\top",
        "..",
        ".",
        "sub/x",
        "sub\\x",
        "C:top",
        "C:\\top",
        "/etc/passwd",
        "a\0b",
        abs.as_str(),
    ] {
        assert!(!src.check_reference(bad), "{bad:?} must be refused");
        assert!(
            src.fetch(bad).err() == Some(SourceFailure::Denied),
            "{bad:?} must not be read"
        );
    }
    assert!(src.check_reference("db-password_1.txt"));

    // `resolve` refuses it by key before any read, and even when the environment sets the key.
    let sources = file_sources(&secrets);
    for env in [
        BTreeMap::new(),
        BTreeMap::from([("DEMO_API_TOKEN".to_owned(), "x".to_owned())]),
    ] {
        let e = resolve(
            &schema_for("../top"),
            &sources,
            &env,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(matches!(
            &e.0[0],
            ResolveError::BadReference { key, source_name, reference }
                if key == "api.token" && source_name == "file" && reference == "../top"
        ));
    }
}

#[test]
fn a_missing_file_and_a_directory_are_named_failures() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("adir")).unwrap();
    // Owner-only, so only the "is a regular file" check can refuse it.
    private(&d.path().join("adir"));
    assert!(failure(d.path(), "absent") == Some(SourceFailure::NotFound));
    assert!(failure(d.path(), "adir") == Some(SourceFailure::Denied));
    let gone = d.path().join("no-such-dir");
    assert!(failure(&gone, "x") == Some(SourceFailure::NotFound));
}

#[test]
fn a_file_over_64_kib_is_refused_and_exactly_64_kib_is_read() {
    let d = tempfile::tempdir().unwrap();
    let mut big = CANARY.as_bytes().to_vec();
    big.resize(64 * 1024 + 1, b'x');
    put(d.path(), "big", &big);
    big.pop();
    put(d.path(), "edge", &big);
    assert!(failure(d.path(), "big") == Some(SourceFailure::Invalid));
    assert!(fetch(d.path(), "edge").is_ok_and(|s| s.len() == 64 * 1024));
}

#[test]
fn a_file_that_is_not_utf8_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut bytes = CANARY.as_bytes().to_vec();
    bytes.extend_from_slice(&[0xff, 0xfe]);
    put(d.path(), "bin", &bytes);
    assert!(failure(d.path(), "bin") == Some(SourceFailure::Invalid));
}

#[cfg(unix)]
#[test]
fn a_file_readable_by_group_or_others_is_refused_and_owner_only_is_read() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    for (mode, ok) in [
        (0o600, true),
        (0o400, true),
        (0o640, false),
        (0o604, false),
        (0o644, false),
        (0o660, false),
        (0o666, false),
    ] {
        let name = format!("m{mode:o}");
        put(d.path(), &name, CANARY.as_bytes());
        std::fs::set_permissions(d.path().join(&name), std::fs::Permissions::from_mode(mode))
            .unwrap();
        assert!(fetch(d.path(), &name).is_ok() == ok, "mode {mode:o}");
        if !ok {
            assert!(failure(d.path(), &name) == Some(SourceFailure::Denied));
        }
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_that_leaves_the_directory_is_refused_and_one_inside_it_is_followed() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("secrets");
    let elsewhere = root.path().join("elsewhere");
    std::fs::create_dir(&dir).unwrap();
    std::fs::create_dir(&elsewhere).unwrap();
    put(&elsewhere, "target", CANARY.as_bytes());
    symlink(elsewhere.join("target"), dir.join("out-abs")).unwrap();
    symlink("../elsewhere/target", dir.join("out-rel")).unwrap();
    assert!(failure(&dir, "out-abs") == Some(SourceFailure::Denied));
    assert!(failure(&dir, "out-rel") == Some(SourceFailure::Denied));

    // A container mount links `name -> ..data/name`, all inside the directory.
    std::fs::create_dir(dir.join("..data")).unwrap();
    put(&dir.join("..data"), "token", CANARY.as_bytes());
    symlink("..data/token", dir.join("token")).unwrap();
    assert!(fetch(&dir, "token").is_ok_and(|s| s == CANARY));

    // A dangling link is not found.
    symlink("nowhere", dir.join("dangling")).unwrap();
    assert!(failure(&dir, "dangling") == Some(SourceFailure::NotFound));
}

#[test]
fn the_directory_comes_from_the_environment_and_nothing_else() {
    assert!(FileSource::from_env(&BTreeMap::new()).is_none());
    let empty = BTreeMap::from([(FileSource::DIR_ENV.to_owned(), String::new())]);
    assert!(FileSource::from_env(&empty).is_none());
    assert_eq!(FileSource::DIR_ENV, "CONFIGURANT_SECRETS_DIR");
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "tok", CANARY.as_bytes());
    let env = BTreeMap::from([(
        FileSource::DIR_ENV.to_owned(),
        d.path().display().to_string(),
    )]);
    let src = FileSource::from_env(&env).expect("set");
    assert!(src.fetch("tok").is_ok_and(|v| v.expose() == CANARY));
}

#[test]
fn a_secret_loaded_from_a_file_is_redacted_everywhere_and_failures_never_carry_it() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "tok", format!("{CANARY}\n").as_bytes());
    let schema = schema_for("tok");
    let sources = file_sources(d.path());
    let env = BTreeMap::new();
    let resolved = resolve(&schema, &sources, &env, Duration::from_secs(1)).unwrap();
    assert!(!format!("{resolved:?}").contains(CANARY));
    let c = resolved
        .into_loader(Loader::new(&schema).env(env))
        .build()
        .unwrap();
    assert!(c.get::<String>("api.token").unwrap() == CANARY);
    let e = c.explain("api.token").unwrap();
    assert!(e.chain.iter().any(|l| l.origin.to_string()
        == "secret source `file` (reference `tok`)"
        && l.shown == "<redacted>"));
    assert!(!format!("{e:?}").contains(CANARY));
    assert!(!format!("{c:?}").contains(CANARY));

    // Failing reads: the error names the key, source and reference, and nothing of the file or the directory.
    let mut big = CANARY.as_bytes().to_vec();
    big.resize(64 * 1024 + 1, b'x');
    put(d.path(), "big", &big);
    put(d.path(), "bin", &[CANARY.as_bytes(), &[0xff]].concat());
    for reference in ["big", "bin", "absent", "../tok"] {
        let e = resolve(
            &schema_for(reference),
            &sources,
            &BTreeMap::new(),
            Duration::from_secs(1),
        )
        .unwrap_err();
        let shown = format!("{e} {e:?}");
        assert!(!shown.contains(CANARY));
        assert!(!shown.contains(&d.path().display().to_string()), "no path");
        assert!(shown.contains("api.token") && shown.contains(reference));
    }
}
