// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad cases for the plan executor, part b (ROADMAP row 28): the run is bound to the plan hash, stays
//! inside the roots for real, never leaks a secret, refuses a surprise overwrite, stops at a failure and resumes.

use configurant::exec::{
    Check, Content, EnvValue, EnvVar, ExecPlan, Executor, Failure, Host, RunError, Secret,
    SecretRef, ServiceState, Step, StepAction, StepStatus, sha256_hex,
};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const CANARY: &str = "hunter2-canary-9f3a";

fn root() -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let p = std::env::temp_dir().join(format!(
        "configurant-run-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p.canonicalize().unwrap()
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[derive(Default)]
struct TestHost {
    content_calls: Cell<u32>,
    contents: RefCell<Vec<(String, Vec<u8>)>>,
    acl: bool,
    services: RefCell<Vec<String>>,
    serve: bool,
}

impl Host for TestHost {
    fn secret(&self, name: &str) -> Option<Secret> {
        (name == "db.password" || name == "api.key").then(|| Secret::new(CANARY))
    }
    fn content(&self, sha256: &str) -> Option<Vec<u8>> {
        self.content_calls.set(self.content_calls.get() + 1);
        self.contents
            .borrow()
            .iter()
            .find(|(h, _)| h == sha256)
            .map(|(_, b)| b.clone())
    }
    fn owner_only_acl(&self, _: &Path) -> bool {
        self.acl
    }
    fn service(&self, name: &str, _: ServiceState) -> bool {
        self.services.borrow_mut().push(name.to_owned());
        self.serve
    }
}

fn inline(id: &str, path: &Path, text: &str) -> Step {
    Step::new(
        id,
        "Write a file",
        StepAction::WriteFile {
            path: s(path),
            mode: None,
            content: Content::Inline { text: text.into() },
        },
    )
}

fn mkdir(id: &str, path: &Path) -> Step {
    Step::new(
        id,
        "Make a directory",
        StepAction::Mkdir {
            path: s(path),
            mode: None,
        },
    )
}

/// An argv that runs a short script with the platform's own interpreter (no shell string is split by us).
fn script(unix: &str, windows: &str) -> Vec<String> {
    if cfg!(windows) {
        vec!["cmd".into(), "/C".into(), windows.into()]
    } else {
        vec!["sh".into(), "-c".into(), unix.into()]
    }
}

fn run_step(id: &str, argv: Vec<String>, timeout: u64, env: Vec<EnvVar>) -> Step {
    Step::new(
        id,
        "Run a program",
        StepAction::Run {
            argv,
            cwd: None,
            timeout_secs: timeout,
            env,
        },
    )
}

fn plan(r: &Path, steps: Vec<Step>) -> ExecPlan {
    ExecPlan::new(vec![s(r)], steps, &[CANARY]).unwrap()
}

#[test]
fn a_wrong_or_stale_hash_is_refused_and_nothing_changes() {
    let r = root();
    let p = plan(&r, vec![inline("a", &r.join("f"), "x")]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("state.toml"));
    assert_eq!(
        ex.run("0".repeat(64).as_str(), &[]),
        Err(RunError::HashMismatch)
    );
    assert!(!r.join("f").exists());
    assert!(!r.join("state.toml").exists());
    // a plan edited after it was shown has another hash, so the old hash is refused for it
    let edited = plan(&r, vec![inline("a", &r.join("f"), "y")]);
    let ex2 = Executor::new(&edited, &h, r.join("state.toml"));
    assert_eq!(ex2.run(p.hash(), &[]), Err(RunError::HashMismatch));
    assert!(!r.join("f").exists());
}

#[test]
fn a_plan_runs_and_the_state_file_holds_only_ids_states_and_the_hash() {
    let r = root();
    let p = plan(
        &r,
        vec![
            mkdir("dir", &r.join("data")),
            inline("conf", &r.join("data").join("app.conf"), "port = 9000\n"),
            Step::new(
                "key",
                "Write the key",
                StepAction::WriteFile {
                    path: s(&r.join("data").join("api.key")),
                    mode: Some(0o600),
                    content: Content::Secret {
                        secret: SecretRef("api.key".into()),
                    },
                },
            ),
        ],
    );
    let h = TestHost {
        acl: true,
        ..Default::default()
    };
    let st = r.join("state.toml");
    let rep = Executor::new(&p, &h, &st).run(p.hash(), &[]).unwrap();
    assert!(rep.ok(&p), "{rep:?}");
    assert_eq!(
        std::fs::read_to_string(r.join("data").join("app.conf")).unwrap(),
        "port = 9000\n"
    );
    // the secret file has the value (that is its job); nothing else does
    assert_eq!(
        std::fs::read_to_string(r.join("data").join("api.key")).unwrap(),
        CANARY
    );
    let state = std::fs::read_to_string(&st).unwrap();
    assert!(
        !state.contains(CANARY) && !state.contains("port"),
        "{state}"
    );
    assert!(state.contains(p.hash()) && state.contains("verified"));
    let shown = format!(
        "{rep:?} {:?} {:?}",
        Executor::new(&p, &h, &st),
        Secret::new(CANARY)
    );
    assert!(!shown.contains(CANARY), "{shown}");
}

#[cfg(unix)]
#[test]
fn a_secret_file_is_owner_only_on_unix_whatever_the_step_asks() {
    use std::os::unix::fs::PermissionsExt;
    let r = root();
    let p = plan(
        &r,
        vec![Step::new(
            "key",
            "Write the key",
            StepAction::WriteFile {
                path: s(&r.join("k")),
                mode: Some(0o644),
                content: Content::Secret {
                    secret: SecretRef("api.key".into()),
                },
            },
        )],
    );
    let h = TestHost::default();
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert!(rep.ok(&p));
    let mode = std::fs::metadata(r.join("k")).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode & 0o077,
        0,
        "group/other bits on a secret file: {mode:o}"
    );
}

#[test]
fn a_secret_file_on_windows_needs_the_host_to_report_an_owner_only_acl() {
    let r = root();
    let step = || {
        Step::new(
            "key",
            "Write the key",
            StepAction::WriteFile {
                path: s(&r.join("k")),
                mode: None,
                content: Content::Secret {
                    secret: SecretRef("api.key".into()),
                },
            },
        )
    };
    let p = plan(&r, vec![step()]);
    let no = TestHost::default();
    let rep = Executor::new(&p, &no, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    if cfg!(windows) {
        assert_eq!(rep.steps[0].failure, Some(Failure::AclRequired));
        assert!(!r.join("k").exists(), "refused, not downgraded silently");
        let yes = TestHost {
            acl: true,
            ..Default::default()
        };
        let again = Executor::new(&p, &yes, r.join("st"))
            .run(p.hash(), &[])
            .unwrap();
        assert!(again.ok(&p));
    } else {
        assert!(rep.ok(&p));
    }
}

#[test]
fn a_secret_reaches_a_child_only_as_an_environment_variable_and_no_output_has_it() {
    let r = root();
    let ok = script(
        &format!("test \"$PW\" = \"{CANARY}\""),
        &format!("if \"%PW%\"==\"{CANARY}\" (exit 0) else (exit 3)"),
    );
    // the program echoes the secret; the executor discards output
    let echo = script("echo \"$PW\"; exit 4", "echo %PW% & exit 4");
    let env = vec![EnvVar {
        name: "PW".into(),
        value: EnvValue::Secret {
            secret: SecretRef("db.password".into()),
        },
    }];
    let p = ExecPlan::new(
        vec![s(&r)],
        vec![
            run_step("ok", ok.clone(), 30, env.clone()),
            run_step("echo", echo, 30, env),
        ],
        &[],
    );
    // the check script embeds the canary in argv, which the plan builder must reject when told the secret
    assert!(
        ExecPlan::new(
            vec![s(&r)],
            vec![run_step("x", ok.clone(), 5, vec![])],
            &[CANARY]
        )
        .is_err()
    );
    let p = p.unwrap();
    let h = TestHost::default();
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(rep.steps[0].status, StepStatus::Verified, "{rep:?}");
    assert_eq!(
        rep.steps[1].failure,
        Some(Failure::CommandFailed { code: Some(4) })
    );
    let shown = format!("{rep:?} {}", std::fs::read_to_string(r.join("st")).unwrap());
    assert!(!shown.contains(CANARY), "{shown}");
}

#[test]
fn a_missing_secret_fails_the_step_by_name_without_a_value() {
    let r = root();
    let p = plan(
        &r,
        vec![run_step(
            "a",
            script("exit 0", "exit 0"),
            5,
            vec![EnvVar {
                name: "X".into(),
                value: EnvValue::Secret {
                    secret: SecretRef("nope".into()),
                },
            }],
        )],
    );
    let h = TestHost::default();
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(
        rep.steps[0].failure,
        Some(Failure::SecretUnavailable {
            name: "nope".into()
        })
    );
}

#[test]
fn an_overwrite_needs_the_approval_of_its_step() {
    let r = root();
    std::fs::write(r.join("f"), "mine").unwrap();
    let p = plan(&r, vec![inline("a", &r.join("f"), "theirs")]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    let rep = ex.run(p.hash(), &[]).unwrap();
    assert_eq!(rep.steps[0].failure, Some(Failure::OverwriteRefused));
    assert_eq!(std::fs::read_to_string(r.join("f")).unwrap(), "mine");
    let rep = ex.run(p.hash(), &["a"]).unwrap();
    assert!(rep.ok(&p), "{rep:?}");
    assert_eq!(std::fs::read_to_string(r.join("f")).unwrap(), "theirs");
}

#[test]
fn a_file_with_the_wanted_content_is_not_an_overwrite() {
    let r = root();
    std::fs::write(r.join("f"), "same").unwrap();
    let p = plan(&r, vec![inline("a", &r.join("f"), "same")]);
    let h = TestHost::default();
    assert!(
        Executor::new(&p, &h, r.join("st"))
            .run(p.hash(), &[])
            .unwrap()
            .ok(&p)
    );
}

/// Make `link` point at `target` without needing a privilege: a symlink on Unix, a junction on Windows.
fn link(link: &Path, target: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let st = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &s(link), &s(target)])
            .output()
            .unwrap();
        assert!(st.status.success(), "{st:?}");
    }
}

#[test]
fn a_path_that_leaves_the_roots_through_a_link_is_refused() {
    let r = root();
    let outside = root();
    link(&r.join("door"), &outside);
    let p = plan(
        &r,
        vec![
            mkdir("m", &r.join("door").join("made")),
            inline("w", &r.join("door").join("f"), "x"),
        ],
    );
    let h = TestHost::default();
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(rep.steps[0].failure, Some(Failure::OutsideRoots));
    assert!(!outside.join("made").exists());
    assert!(!outside.join("f").exists());
}

#[cfg(unix)]
#[test]
fn a_target_file_that_is_a_symlink_is_not_written_through() {
    let r = root();
    std::fs::write(r.join("real"), "keep").unwrap();
    std::os::unix::fs::symlink(r.join("real"), r.join("f")).unwrap();
    let p = plan(&r, vec![inline("a", &r.join("f"), "new")]);
    let h = TestHost::default();
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(rep.steps[0].failure, Some(Failure::Symlink));
    assert_eq!(std::fs::read_to_string(r.join("real")).unwrap(), "keep");
}

#[test]
fn a_failed_step_stops_the_run_and_the_next_run_resumes_without_repeating() {
    let r = root();
    let body = b"hello".to_vec();
    let hash = sha256_hex(&body);
    std::fs::write(r.join("blocker"), "a file").unwrap();
    let p = plan(
        &r,
        vec![
            Step::new(
                "first",
                "Write by hash",
                StepAction::WriteFile {
                    path: s(&r.join("one")),
                    mode: None,
                    content: Content::Hash {
                        sha256: hash.clone(),
                    },
                },
            ),
            mkdir("second", &r.join("blocker").join("sub")),
            inline("third", &r.join("three"), "3"),
        ],
    );
    let h = TestHost::default();
    h.contents.borrow_mut().push((hash, body));
    let st = r.join("st");
    let ex = Executor::new(&p, &h, &st);
    let rep = ex.run(p.hash(), &[]).unwrap();
    assert_eq!(
        rep.steps.len(),
        2,
        "the run stopped at the failure: {rep:?}"
    );
    assert_eq!(rep.steps[1].status, StepStatus::Failed);
    assert!(!r.join("three").exists());
    assert!(std::fs::read_to_string(&st).unwrap().contains("failed"));
    assert_eq!(h.content_calls.get(), 1);
    // fix the cause and run again with the same state file
    std::fs::remove_file(r.join("blocker")).unwrap();
    let rep = ex.run(p.hash(), &[]).unwrap();
    assert!(rep.ok(&p), "{rep:?}");
    assert!(rep.steps[0].skipped, "a verified step is not repeated");
    assert_eq!(
        h.content_calls.get(),
        1,
        "the host was not asked for the content again"
    );
    assert!(r.join("three").exists());
}

#[test]
fn host_content_that_does_not_match_its_hash_is_refused() {
    let r = root();
    let hash = sha256_hex(b"expected");
    let p = plan(
        &r,
        vec![Step::new(
            "a",
            "Write by hash",
            StepAction::WriteFile {
                path: s(&r.join("f")),
                mode: None,
                content: Content::Hash {
                    sha256: hash.clone(),
                },
            },
        )],
    );
    let h = TestHost::default();
    h.contents.borrow_mut().push((hash, b"tampered".to_vec()));
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(rep.steps[0].failure, Some(Failure::ContentHashMismatch));
    assert!(!r.join("f").exists());
    let none = TestHost::default();
    let rep = Executor::new(&p, &none, r.join("st2"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(rep.steps[0].failure, Some(Failure::ContentUnavailable));
}

#[test]
fn a_state_file_for_another_plan_is_refused() {
    let r = root();
    let a = plan(&r, vec![inline("a", &r.join("f"), "1")]);
    let b = plan(&r, vec![inline("a", &r.join("f"), "2")]);
    let h = TestHost::default();
    let st = r.join("st");
    assert!(
        Executor::new(&a, &h, &st)
            .run(a.hash(), &[])
            .unwrap()
            .ok(&a)
    );
    assert_eq!(
        Executor::new(&b, &h, &st).run(b.hash(), &["a"]),
        Err(RunError::StateMismatch)
    );
}

#[test]
fn a_program_past_its_timeout_is_killed() {
    let r = root();
    let sleepy = script("sleep 20", "ping -n 21 127.0.0.1 >nul");
    let p = plan(&r, vec![run_step("slow", sleepy, 1, vec![])]);
    let h = TestHost::default();
    let t = std::time::Instant::now();
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(rep.steps[0].failure, Some(Failure::Timeout));
    assert!(t.elapsed().as_secs() < 15, "not killed: {:?}", t.elapsed());
}

#[test]
fn a_check_that_holds_skips_the_step_and_a_failed_verify_fails_it() {
    let r = root();
    std::fs::write(r.join("have"), "x").unwrap();
    let mut done = run_step("done", script("exit 9", "exit 9"), 5, vec![]);
    done.check = Some(Check::PathExists {
        path: s(&r.join("have")),
    });
    let mut bad = mkdir("bad", &r.join("d"));
    bad.verify = Some(Check::PathExists {
        path: s(&r.join("never")),
    });
    let p = plan(&r, vec![done, bad]);
    let h = TestHost::default();
    let rep = Executor::new(&p, &h, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert!(
        rep.steps[0].skipped && rep.steps[0].status == StepStatus::Verified,
        "{rep:?}"
    );
    assert_eq!(rep.steps[1].failure, Some(Failure::VerifyFailed));
}

#[test]
fn services_go_to_the_host_and_a_declining_host_fails_the_step() {
    let r = root();
    let p = plan(
        &r,
        vec![Step::new(
            "svc",
            "Start it",
            StepAction::Service {
                name: "app".into(),
                state: ServiceState::Up,
            },
        )],
    );
    let no = TestHost::default();
    let rep = Executor::new(&p, &no, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert_eq!(rep.steps[0].failure, Some(Failure::ServiceDeclined));
    let yes = TestHost {
        serve: true,
        ..Default::default()
    };
    let rep = Executor::new(&p, &yes, r.join("st"))
        .run(p.hash(), &[])
        .unwrap();
    assert!(rep.ok(&p));
    assert_eq!(yes.services.borrow().as_slice(), ["app"]);
}
