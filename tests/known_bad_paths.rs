// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `paths` module (feature `paths`, docs/MODULES.md section 3, rules T1 to T5). Known-bad first: every row
//! of the design is a value that must fail to load, with the accepted control beside it. The platform rules are
//! set explicitly, so the Windows rows are asserted on every host.

#![cfg(feature = "paths")]

use std::collections::BTreeMap;

use configurant::exec::StepAction;
use configurant::modules::paths::{Paths, Platform};
use configurant::{ActionKind, ConfigError, Loader, Module, Modules, Origin, Schema, Value};

fn schema_default(extra: &str, default: &str) -> String {
    format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "where"
type = "path"
{default}
description = "where"
env = "WHERE"
provider = "paths"
{extra}
"#
    )
}

/// The refusal of `value` given as the environment string, or `None` if it loaded. The schema default is `default`
/// (it is checked too, so it must be acceptable to the module under test).
fn refusal_with(p: Paths, default: &str, value: &str) -> Option<String> {
    let s = Schema::parse(&schema_default("", &format!("default = {default:?}"))).unwrap();
    let mut m = Modules::new();
    m.register(Box::new(p)).unwrap();
    let env = BTreeMap::from([("WHERE".to_owned(), value.to_owned())]);
    match Loader::new(&s).modules(&m).env(env).build() {
        Ok(_) => None,
        Err(e) => {
            assert!(
                e.0.iter().all(
                    |x| matches!(x, ConfigError::ModuleRejected { module, .. } if module == "paths")
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
                    .unwrap_or_else(|| panic!("the default was refused, not the value: {e:?}"));
            Some(ours.to_string())
        }
    }
}

fn unix() -> Paths {
    Paths::new().platform(Platform::Unix)
}

fn windows() -> Paths {
    Paths::new().platform(Platform::Windows)
}

fn refused(p: Paths, default: &str, values: &[&str]) {
    for v in values {
        assert!(
            refusal_with(p.clone(), default, v).is_some(),
            "{v:?} loaded"
        );
    }
}

fn accepted(p: Paths, default: &str, values: &[&str]) {
    for v in values {
        if let Some(why) = refusal_with(p.clone(), default, v) {
            panic!("{v:?} refused: {why}");
        }
    }
}

// ---- T1: not empty, no NUL or control character ----

#[test]
fn t1_empty_nul_and_control_characters_are_refused() {
    for p in [unix(), windows()] {
        refused(p.clone(), "a", &["", "a\0b", "a\nb", "a\tb", "a\u{1b}[0m"]);
        accepted(p, "a", &["a/b", "a b", "naïve/ü"]);
    }
}

#[test]
fn t1_a_value_that_is_not_text_is_refused() {
    assert!(unix().validate(&Value::Int(3)).is_err());
    assert!(unix().validate(&Value::Bool(true)).is_err());
    assert!(unix().validate(&Value::Path("a/b".into())).is_ok());
    assert!(unix().validate(&Value::String("a/b".into())).is_ok());
}

// ---- T2: within a root ----

#[test]
fn t2_unix_a_path_that_leaves_the_root_is_refused() {
    let p = || unix().within("/srv/app");
    refused(
        p(),
        "data",
        &[
            "../etc/x",
            "a/../../x",
            "/etc/x",
            "/srv/app/../x",
            "/srv/appx/y",
            "/srv",
            "/",
            "//etc/x",
        ],
    );
    accepted(
        p(),
        "data",
        &[
            "data/x",
            "/srv/app/data",
            "/srv/app",
            "/srv/app/",
            "a/../b",
            "./x",
            "../app/x",
            "a//b",
        ],
    );
}

#[test]
fn t2_windows_a_path_on_another_drive_or_share_is_refused() {
    let p = || windows().within("D:\\app");
    refused(
        p(),
        "logs",
        &[
            "C:\\other",
            // The same folders on another drive.
            "C:\\app\\x",
            "\\\\server\\share\\x",
            "..\\x",
            "logs\\..\\..\\x",
            "D:\\apple",
            "D:\\app\\..\\x",
            // No drive: it cannot be placed.
            "\\x",
            "D:x",
        ],
    );
    accepted(
        p(),
        "logs",
        &[
            "d:\\APP\\log",
            "D:/app/x",
            "logs\\x",
            "D:\\app",
            "D:\\app\\",
        ],
    );
}

#[test]
fn t2_a_unc_root_holds_its_own_share_only() {
    let p = || windows().within("\\\\srv\\data\\app");
    refused(
        p(),
        "x",
        &[
            "\\\\srv\\other\\app\\x",
            "\\\\other\\data\\app\\x",
            "\\\\srv\\data\\x",
        ],
    );
    accepted(p(), "x", &["\\\\SRV\\DATA\\app\\x", "x\\y"]);
}

#[test]
fn t2_a_root_that_is_not_absolute_is_the_programs_mistake_and_says_so() {
    let s = Schema::parse(&schema_default("", "default = \"/srv/app/x\"")).unwrap();
    let mut m = Modules::new();
    m.register(Box::new(unix().within("rel/dir"))).unwrap();
    let e = Loader::new(&s).modules(&m).build().unwrap_err();
    assert!(
        e.0[0].to_string().contains("mistake in the program"),
        "{}",
        e.0[0]
    );
}

// ---- T3: relative or absolute ----

#[test]
fn t3_unix_relative_and_absolute() {
    refused(unix().relative(), "a", &["/x", "//x"]);
    accepted(unix().relative(), "a", &["x", "x/y", "../x"]);
    refused(unix().absolute(), "/a", &["x", "./x", "../x"]);
    accepted(unix().absolute(), "/a", &["/x", "/x/../y"]);
}

#[test]
fn t3_windows_relative_and_absolute() {
    refused(
        windows().relative(),
        "a",
        &["C:\\x", "\\x", "C:x", "\\\\s\\h", "/x"],
    );
    accepted(windows().relative(), "a", &["x", "x\\y", "..\\x"]);
    refused(
        windows().absolute(),
        "C:\\a",
        &["x", "\\x", "C:x", "/x", ".\\x"],
    );
    accepted(
        windows().absolute(),
        "C:\\a",
        &["C:\\x", "c:/x", "\\\\s\\h\\x", "\\\\s\\h"],
    );
}

#[test]
fn t3_the_default_platform_is_the_one_the_program_runs_on() {
    let on_windows = cfg!(windows);
    let drive = refusal_with(
        Paths::new().absolute(),
        if on_windows { "C:\\a" } else { "/a" },
        "C:\\x",
    );
    assert_eq!(drive.is_none(), on_windows, "{drive:?}");
}

// ---- T4: Windows names ----

#[test]
fn t4_windows_names_that_windows_cannot_make_are_refused() {
    refused(
        windows(),
        "a",
        &[
            "logs/CON",
            "logs\\con.txt",
            "nul",
            "COM1",
            "lpt9.log",
            "Aux",
            "prn.",
            "x.",
            "x ",
            "dir. \\y",
            "a:b",
            "a<b",
            "a|b",
            "a?b",
            "a*b",
            "a\"b",
            "C:\\a\\b:stream",
            "\\\\?\\C:\\x",
            "\\\\.\\COM1",
            "\\\\server",
            "\\\\server\\",
        ],
    );
    accepted(
        windows(),
        "a",
        &[
            "logs/CONSOLE",
            "COM10",
            "a.b",
            "..\\x",
            "C:\\a\\b",
            ".hidden",
            "con_x",
            "a b",
        ],
    );
}

#[test]
fn t4_the_same_text_is_fine_where_the_platform_allows_it() {
    accepted(unix(), "a", &["logs/CON", "x.", "a:b", "nul", "a<b", "x "]);
}

// ---- T5: a trailing separator means directory ----

#[test]
fn t5_a_file_may_not_end_in_a_separator() {
    refused(unix().file(), "a", &["out/", ".", "a/..", "a/."]);
    accepted(unix().file(), "a", &["out", "out.txt", "a/b"]);
    refused(windows().file(), "a", &["out\\", "out/"]);
    accepted(unix().directory(), "a", &["out/", "out", "."]);
    accepted(unix(), "a", &["out/"]);
}

// ---- on_change: a Mkdir step, never a removal ----

fn only_mkdir(plan: &configurant::Plan) -> Vec<String> {
    plan.actions
        .iter()
        .map(|a| match &a.kind {
            ActionKind::Step(step) => match &step.action {
                StepAction::Mkdir { path, .. } => path.clone(),
                other => panic!("a paths change must only make a directory, got {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        })
        .collect()
}

fn text(s: &str) -> Value {
    Value::Path(s.into())
}

#[test]
fn a_new_directory_gets_a_mkdir_step_and_the_old_one_is_left_alone() {
    let p = unix().directory();
    let plan = p.on_change(&text("/srv/old"), &text("/srv/new/"));
    assert_eq!(only_mkdir(&plan), ["/srv/new"]);
    assert!(
        plan.actions[0].description.contains("left where it is"),
        "{}",
        plan.actions[0].description
    );
    // Nothing mentions removing the old directory.
    let StepAction::Mkdir { path, .. } = (match &plan.actions[0].kind {
        ActionKind::Step(s) => s.action.clone(),
        other => panic!("{other:?}"),
    }) else {
        panic!("not a mkdir")
    };
    assert_ne!(path, "/srv/old");
}

#[test]
fn windows_directories_are_made_the_same_way() {
    let plan = windows()
        .directory()
        .on_change(&text("D:\\old"), &text("D:\\new\\"));
    assert_eq!(only_mkdir(&plan), ["D:\\new"]);
    let plan = windows()
        .directory()
        .on_change(&text("D:\\a"), &text("\\\\s\\h\\x"));
    assert_eq!(only_mkdir(&plan), ["\\\\s\\h\\x"]);
}

#[test]
fn no_step_is_offered_when_nothing_should_be_made() {
    let none =
        |p: Paths, old: &str, new: &str| p.on_change(&text(old), &text(new)).actions.is_empty();
    // Unchanged.
    assert!(none(unix().directory(), "/a", "/a"));
    // A file, or no declared kind, is not a directory to make.
    assert!(none(unix().file(), "/a", "/b"));
    assert!(none(unix(), "/a", "/b"));
    // A relative path has no place to be made.
    assert!(none(unix().directory(), "/a", "b"));
    // A value the module refuses is never turned into a step.
    assert!(none(windows().directory(), "C:\\a", "C:\\CON"));
    assert!(none(unix().directory().within("/srv"), "/srv/a", "/etc/b"));
    // Not a path at all.
    assert!(
        unix()
            .directory()
            .on_change(&text("/a"), &Value::Int(1))
            .actions
            .is_empty()
    );
}

#[test]
fn a_root_directory_keeps_its_separator() {
    assert_eq!(
        only_mkdir(&unix().directory().on_change(&text("/a"), &text("/"))),
        ["/"]
    );
    assert_eq!(
        only_mkdir(
            &windows()
                .directory()
                .on_change(&text("C:\\a"), &text("C:\\"))
        ),
        ["C:\\"]
    );
}

// ---- a path is never quoted ----

#[test]
fn a_refusal_never_quotes_the_value() {
    for (p, bad) in [
        (unix().within("/srv/app"), "/etc/TOPSECRET"),
        (windows(), "C:\\TOPSECRET\\CON"),
        (unix().relative(), "/TOPSECRET"),
        (unix().file(), "TOPSECRET/"),
        (unix(), "TOPSECRET\0"),
    ] {
        let why = refusal_with(p, "a", bad).unwrap();
        assert!(!why.contains("TOPSECRET"), "{why}");
    }
}

#[test]
fn a_secret_key_leaks_nothing() {
    let s = Schema::parse(&schema_default("secret = true", "")).unwrap();
    let mut m = Modules::new();
    m.register(Box::new(unix().within("/srv"))).unwrap();
    let env = BTreeMap::from([("WHERE".to_owned(), "/TOPSECRET".to_owned())]);
    let errors = Loader::new(&s).modules(&m).env(env).build().unwrap_err();
    let shown = format!("{errors:?} {}", errors.0[0]);
    assert!(!shown.contains("TOPSECRET"), "{shown}");
    assert!(shown.contains("<redacted>"), "{shown}");
}
