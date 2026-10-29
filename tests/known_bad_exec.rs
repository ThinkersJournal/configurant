// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad cases for the plan executor, part a (ROADMAP row 28): what a plan may not contain, that
//! `describe` is inert, that the hash moves with the plan, and that no secret value reaches any output.

use configurant::exec::{
    Check, Content, Effect, EnvValue, EnvVar, ExecPlan, INLINE_LIMIT, Problem, SecretRef,
    ServiceState, Step, StepAction, Undo,
};
use configurant::{Action, ActionKind, Plan};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

const CANARY: &str = "hunter2-canary-9f3a";

fn root() -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let p = std::env::temp_dir().join(format!(
        "configurant-exec-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn s(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

fn mkdir(id: &str, path: &str) -> Step {
    Step::new(
        id,
        "Make a directory",
        StepAction::Mkdir {
            path: path.into(),
            mode: None,
        },
    )
}

fn write(id: &str, path: &str, content: Content) -> Step {
    Step::new(
        id,
        "Write a file",
        StepAction::WriteFile {
            path: path.into(),
            mode: None,
            content,
        },
    )
}

fn run(id: &str, argv: &[&str]) -> Step {
    Step::new(
        id,
        "Run a program",
        StepAction::Run {
            argv: argv.iter().map(|a| a.to_string()).collect(),
            cwd: None,
            timeout_secs: 30,
            env: vec![],
        },
    )
}

fn refused(roots: &[&str], steps: Vec<Step>, secrets: &[&str]) -> configurant::exec::ExecError {
    ExecPlan::new(
        roots.iter().map(|r| r.to_string()).collect(),
        steps,
        secrets,
    )
    .expect_err("the plan must be refused")
}

#[test]
fn a_plan_with_no_roots_is_refused() {
    let e = refused(&[], vec![], &[]);
    assert_eq!(e.problem, Problem::NoRoots);
}

#[test]
fn duplicate_ids_are_refused() {
    let r = s(&root());
    let e = refused(&[&r], vec![mkdir("a", &r), mkdir("a", &r)], &[]);
    assert_eq!(e.problem, Problem::DuplicateId);
    assert_eq!(e.step, "a");
}

#[test]
fn a_path_outside_the_roots_is_refused() {
    let r = root();
    let other = root();
    let e = refused(&[&s(&r)], vec![mkdir("a", &s(&other.join("x")))], &[]);
    assert_eq!(e.problem, Problem::OutsideRoots);
}

#[test]
fn dot_dot_is_refused_even_when_it_would_stay_inside() {
    let r = root();
    let sneaky = format!("{}/sub/../x", s(&r));
    let e = refused(&[&s(&r)], vec![mkdir("a", &sneaky)], &[]);
    assert_eq!(e.problem, Problem::ParentDir);
}

#[test]
fn a_dot_dot_that_leaves_the_root_is_refused() {
    let r = root();
    let sneaky = format!("{}/../elsewhere", s(&r));
    let e = refused(
        &[&s(&r)],
        vec![write("a", &sneaky, Content::Inline { text: "x".into() })],
        &[],
    );
    assert_eq!(e.problem, Problem::ParentDir);
}

#[test]
fn inline_content_over_the_limit_is_refused() {
    let r = root();
    let big = "x".repeat(INLINE_LIMIT + 1);
    let e = refused(
        &[&s(&r)],
        vec![write("a", &s(&r.join("f")), Content::Inline { text: big })],
        &[],
    );
    assert_eq!(
        e.problem,
        Problem::TooLarge {
            bytes: INLINE_LIMIT + 1
        }
    );
}

#[test]
fn inline_content_at_the_limit_is_accepted() {
    let r = root();
    let ok = "x".repeat(INLINE_LIMIT);
    ExecPlan::new(
        vec![s(&r)],
        vec![write("a", &s(&r.join("f")), Content::Inline { text: ok })],
        &[],
    )
    .expect("exactly at the limit is allowed");
}

#[test]
fn a_secret_value_inline_is_refused_everywhere_it_could_hide() {
    let r = root();
    let rs = s(&r);
    let cases: Vec<(&str, Step)> = vec![
        (
            "content",
            write(
                "a",
                &s(&r.join("f")),
                Content::Inline {
                    text: format!("password={CANARY}"),
                },
            ),
        ),
        ("argv", run("a", &["tool", &format!("--password={CANARY}")])),
        (
            "env",
            Step::new(
                "a",
                "Run",
                StepAction::Run {
                    argv: vec!["tool".into()],
                    cwd: None,
                    timeout_secs: 5,
                    env: vec![EnvVar {
                        name: "PW".into(),
                        value: EnvValue::Literal {
                            text: CANARY.into(),
                        },
                    }],
                },
            ),
        ),
        ("path", mkdir("a", &s(&r.join(CANARY)))),
        (
            "title",
            Step::new(
                "a",
                &format!("Set {CANARY}"),
                StepAction::Service {
                    name: "svc".into(),
                    state: ServiceState::Up,
                },
            ),
        ),
        (
            "service",
            Step::new(
                "a",
                "Up",
                StepAction::Service {
                    name: CANARY.into(),
                    state: ServiceState::Up,
                },
            ),
        ),
    ];
    for (field, step) in cases {
        let e = refused(&[&rs], vec![step], &[CANARY]);
        assert_eq!(e.problem, Problem::SecretValue, "{field}");
        assert_eq!(e.field, field);
        let shown = format!("{e} {e:?}");
        assert!(
            !shown.contains(CANARY),
            "the error leaked the value for {field}: {shown}"
        );
    }
}

#[test]
fn a_secret_ref_is_accepted_and_nothing_shows_a_value() {
    let r = root();
    let step = Step::new(
        "a",
        "Run the migration",
        StepAction::Run {
            argv: vec!["app".into(), "migrate".into()],
            cwd: None,
            timeout_secs: 60,
            env: vec![EnvVar {
                name: "DB_PASSWORD".into(),
                value: EnvValue::Secret {
                    secret: SecretRef("db.password".into()),
                },
            }],
        },
    );
    let f = write(
        "f",
        &s(&r.join("secret.key")),
        Content::Secret {
            secret: SecretRef("api.key".into()),
        },
    );
    let plan = ExecPlan::new(vec![s(&r)], vec![step, f], &[CANARY]).unwrap();
    let d = plan.describe();
    let all = format!("{d:?} {plan:?} {}", serde_json::to_string(&d).unwrap());
    assert!(!all.contains(CANARY));
    let cmd = d.steps[0].command.as_ref().unwrap().join(" ");
    assert!(
        cmd.contains("DB_PASSWORD=<redacted ref db.password>"),
        "{cmd}"
    );
}

#[test]
fn bad_hashes_modes_timeouts_and_names_are_refused() {
    let r = root();
    let rs = s(&r);
    let f = s(&r.join("f"));
    let e = refused(
        &[&rs],
        vec![write(
            "a",
            &f,
            Content::Hash {
                sha256: "abc".into(),
            },
        )],
        &[],
    );
    assert_eq!(e.problem, Problem::BadHash);
    let e = refused(
        &[&rs],
        vec![Step::new(
            "a",
            "m",
            StepAction::Mkdir {
                path: rs.clone(),
                mode: Some(0o17777),
            },
        )],
        &[],
    );
    assert_eq!(e.problem, Problem::BadMode);
    let e = refused(
        &[&rs],
        vec![Step::new(
            "a",
            "r",
            StepAction::Run {
                argv: vec!["x".into()],
                cwd: None,
                timeout_secs: 0,
                env: vec![],
            },
        )],
        &[],
    );
    assert_eq!(e.problem, Problem::ZeroTimeout);
    let e = refused(&[&rs], vec![run("a", &[])], &[]);
    assert_eq!(e.problem, Problem::Empty);
    let e = refused(
        &[&rs],
        vec![Step::new(
            "a",
            "s",
            StepAction::Service {
                name: "two words".into(),
                state: ServiceState::Up,
            },
        )],
        &[],
    );
    assert_eq!(e.problem, Problem::BadName);
    let e = refused(
        &[&rs],
        vec![Step::new(
            "a",
            "r",
            StepAction::Run {
                argv: vec!["x".into()],
                cwd: None,
                timeout_secs: 1,
                env: vec![EnvVar {
                    name: "A=B".into(),
                    value: EnvValue::Literal { text: "v".into() },
                }],
            },
        )],
        &[],
    );
    assert_eq!(e.problem, Problem::BadName);
}

#[test]
fn checks_and_undo_actions_are_validated_too() {
    let r = root();
    let rs = s(&r);
    let outside = s(&root());
    let mut st = mkdir("a", &rs);
    st.check = Some(Check::PathExists {
        path: outside.clone(),
    });
    assert_eq!(
        refused(&[&rs], vec![st], &[]).problem,
        Problem::OutsideRoots
    );
    let mut st = mkdir("a", &rs);
    st.verify = Some(Check::FileHash {
        path: rs.clone(),
        sha256: "zz".into(),
    });
    assert_eq!(refused(&[&rs], vec![st], &[]).problem, Problem::BadHash);
    let mut st = mkdir("a", &rs);
    st.undo = Undo::Action {
        action: Box::new(StepAction::Mkdir {
            path: outside,
            mode: None,
        }),
    };
    assert_eq!(
        refused(&[&rs], vec![st], &[]).problem,
        Problem::OutsideRoots
    );
}

#[test]
fn the_hash_moves_with_the_plan_and_only_with_the_plan() {
    let r = root();
    let rs = s(&r);
    let mk = |text: &str| {
        ExecPlan::new(
            vec![rs.clone()],
            vec![write(
                "a",
                &s(&r.join("f")),
                Content::Inline { text: text.into() },
            )],
            &[],
        )
        .unwrap()
    };
    assert_eq!(mk("one").hash(), mk("one").hash());
    assert_ne!(mk("one").hash(), mk("two").hash());
    assert_eq!(mk("one").hash().len(), 64);
    let other = ExecPlan::new(
        vec![rs.clone()],
        vec![write(
            "b",
            &s(&r.join("f")),
            Content::Inline { text: "one".into() },
        )],
        &[],
    )
    .unwrap();
    assert_ne!(
        mk("one").hash(),
        other.hash(),
        "the step id is part of the plan"
    );
}

fn snapshot(dir: &std::path::Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    fn walk(d: &std::path::Path, out: &mut Vec<(String, u64)>) {
        for e in std::fs::read_dir(d).unwrap() {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            out.push((
                e.path().to_string_lossy().into_owned(),
                if m.is_file() { m.len() } else { 0 },
            ));
            if m.is_dir() {
                walk(&e.path(), out);
            }
        }
    }
    walk(dir, &mut out);
    out.sort();
    out
}

#[test]
fn describe_changes_nothing_and_says_create_overwrite_unchanged() {
    let r = root();
    std::fs::write(r.join("same"), "hello").unwrap();
    std::fs::write(r.join("diff"), "old").unwrap();
    std::fs::create_dir(r.join("have")).unwrap();
    let rs = s(&r);
    let plan = ExecPlan::new(
        vec![rs.clone()],
        vec![
            mkdir("new-dir", &s(&r.join("new"))),
            mkdir("have-dir", &s(&r.join("have"))),
            mkdir("file-as-dir", &s(&r.join("same"))),
            write(
                "new-file",
                &s(&r.join("fresh")),
                Content::Inline { text: "x".into() },
            ),
            write(
                "same-file",
                &s(&r.join("same")),
                Content::Inline {
                    text: "hello".into(),
                },
            ),
            write(
                "diff-file",
                &s(&r.join("diff")),
                Content::Inline { text: "new".into() },
            ),
            write(
                "dir-as-file",
                &s(&r.join("have")),
                Content::Inline { text: "x".into() },
            ),
            run("cmd", &["app", "--version"]),
            Step::new(
                "svc",
                "Start",
                StepAction::Service {
                    name: "app".into(),
                    state: ServiceState::Up,
                },
            ),
        ],
        &[],
    )
    .unwrap();
    let before = snapshot(&r);
    let d = plan.describe();
    let _ = plan.describe();
    assert_eq!(snapshot(&r), before, "describe wrote to the filesystem");
    let effects: Vec<(&str, Effect)> = d.steps.iter().map(|x| (x.id.as_str(), x.effect)).collect();
    assert_eq!(
        effects,
        vec![
            ("new-dir", Effect::Create),
            ("have-dir", Effect::Unchanged),
            ("file-as-dir", Effect::Conflict),
            ("new-file", Effect::Create),
            ("same-file", Effect::Unchanged),
            ("diff-file", Effect::Overwrite),
            ("dir-as-file", Effect::Conflict),
            ("cmd", Effect::Run),
            ("svc", Effect::Service),
        ]
    );
    assert!(d.notes.iter().any(|n| n.contains("cannot be undone")));
}

#[test]
fn a_mode_on_windows_is_called_advisory() {
    let r = root();
    let plan = ExecPlan::new(
        vec![s(&r)],
        vec![Step::new(
            "a",
            "m",
            StepAction::Mkdir {
                path: s(&r.join("d")),
                mode: Some(0o700),
            },
        )],
        &[],
    )
    .unwrap();
    let advisory = plan.describe().notes.iter().any(|n| n.contains("advisory"));
    assert_eq!(advisory, cfg!(windows));
}

#[test]
fn only_step_actions_of_a_module_plan_are_collected_in_order() {
    let r = root();
    let rs = s(&r);
    let plan = Plan {
        actions: vec![
            Action {
                description: "just a note".into(),
                kind: ActionKind::Note,
            },
            Action {
                description: "one".into(),
                kind: ActionKind::Step(Box::new(mkdir("one", &s(&r.join("a"))))),
            },
            Action {
                description: "two".into(),
                kind: ActionKind::Step(Box::new(mkdir("two", &s(&r.join("b"))))),
            },
        ],
    };
    let ex = ExecPlan::from_plan(vec![rs], &plan, &[]).unwrap();
    let ids: Vec<&str> = ex.steps().iter().map(|x| x.id.as_str()).collect();
    assert_eq!(ids, ["one", "two"]);
}

#[test]
fn the_description_serialises_for_a_web_page() {
    let r = root();
    let plan = ExecPlan::new(vec![s(&r)], vec![mkdir("a", &s(&r.join("d")))], &[]).unwrap();
    let j = serde_json::to_value(plan.describe()).unwrap();
    assert_eq!(j["steps"][0]["effect"], "create");
    assert_eq!(j["steps"][0]["kind"], "mkdir");
    assert_eq!(j["hash"].as_str().unwrap().len(), 64);
}
