// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad cases for the plan executor, part c (ROADMAP row 28): undo reverses only what the plan made,
//! last first, says what it could not undo, never drops a data store, and leaks no secret.

use configurant::exec::{
    Content, ExecPlan, Executor, Failure, Host, Problem, RunError, Secret, SecretRef, ServiceState,
    Step, StepAction, StepStatus, Undo, UndoOutcome,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const CANARY: &str = "hunter2-canary-9f3a";

fn root() -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let p = std::env::temp_dir().join(format!(
        "configurant-undo-{}-{}",
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
    services: RefCell<Vec<(String, ServiceState)>>,
}

impl Host for TestHost {
    fn secret(&self, name: &str) -> Option<Secret> {
        (name == "api.key").then(|| Secret::new(CANARY))
    }
    fn owner_only_acl(&self, _: &Path) -> bool {
        true
    }
    fn service(&self, name: &str, state: ServiceState) -> bool {
        self.services.borrow_mut().push((name.to_owned(), state));
        true
    }
}

fn mkdir(id: &str, path: &Path) -> Step {
    let mut st = Step::new(
        id,
        "Make a directory",
        StepAction::Mkdir {
            path: s(path),
            mode: None,
        },
    );
    st.undo = Undo::Remove { path: s(path) };
    st
}

fn inline(id: &str, path: &Path, text: &str) -> Step {
    let mut st = Step::new(
        id,
        "Write a file",
        StepAction::WriteFile {
            path: s(path),
            mode: None,
            content: Content::Inline { text: text.into() },
        },
    );
    st.undo = Undo::Remove { path: s(path) };
    st
}

fn plan(r: &Path, steps: Vec<Step>) -> ExecPlan {
    ExecPlan::new(vec![s(r)], steps, &[CANARY]).unwrap()
}

fn outcomes(rep: &configurant::exec::UndoReport) -> Vec<(&str, UndoOutcome)> {
    rep.steps
        .iter()
        .map(|x| (x.id.as_str(), x.outcome.clone()))
        .collect()
}

#[test]
fn undo_with_a_wrong_hash_is_refused_and_removes_nothing() {
    let r = root();
    let p = plan(&r, vec![inline("a", &r.join("f"), "x")]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    assert_eq!(ex.undo(&"0".repeat(64)), Err(RunError::HashMismatch));
    assert!(r.join("f").exists());
}

#[test]
fn undo_reverses_what_ran_last_first_and_the_steps_are_pending_again() {
    let r = root();
    let p = plan(
        &r,
        vec![
            mkdir("dir", &r.join("data")),
            inline("conf", &r.join("data").join("app.conf"), "port = 9000\n"),
        ],
    );
    let h = TestHost::default();
    let st = r.join("st");
    let ex = Executor::new(&p, &h, &st);
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(
        outcomes(&rep),
        vec![("conf", UndoOutcome::Undone), ("dir", UndoOutcome::Undone)],
        "last first, so the file goes before its directory"
    );
    assert!(rep.complete());
    assert!(!r.join("data").exists());
    assert!(std::fs::read_to_string(&st).unwrap().contains("pending"));
    // and the plan can run again
    let again = ex.run(p.hash(), &[]).unwrap();
    assert!(again.ok(&p) && !again.steps[0].skipped, "{again:?}");
    assert!(r.join("data").join("app.conf").exists());
}

#[test]
fn a_forward_only_step_is_reported_and_its_effect_stays() {
    let r = root();
    let mut keep = inline("keep", &r.join("kept"), "migrated");
    keep.undo = Undo::None;
    let p = plan(&r, vec![keep, inline("tmp", &r.join("tmp"), "t")]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(
        outcomes(&rep),
        vec![
            ("tmp", UndoOutcome::Undone),
            ("keep", UndoOutcome::ForwardOnly)
        ]
    );
    assert!(!rep.complete());
    assert!(r.join("kept").exists() && !r.join("tmp").exists());
}

#[test]
fn a_file_changed_since_the_step_wrote_it_is_left_in_place() {
    let r = root();
    let p = plan(&r, vec![inline("a", &r.join("f"), "as written")]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    std::fs::write(r.join("f"), "edited by the user").unwrap();
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(rep.steps[0].outcome, UndoOutcome::Left(Failure::Changed));
    assert_eq!(
        std::fs::read_to_string(r.join("f")).unwrap(),
        "edited by the user"
    );
    assert!(
        std::fs::read_to_string(r.join("st"))
            .unwrap()
            .contains("verified")
    );
}

#[test]
fn a_directory_with_data_in_it_is_never_dropped() {
    let r = root();
    let p = plan(&r, vec![mkdir("dir", &r.join("db"))]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    std::fs::write(r.join("db").join("data.sqlite"), "rows").unwrap();
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(rep.steps[0].outcome, UndoOutcome::Left(Failure::NotEmpty));
    assert_eq!(
        std::fs::read_to_string(r.join("db").join("data.sqlite")).unwrap(),
        "rows"
    );
}

#[test]
fn a_remove_aimed_at_something_the_step_did_not_make_is_refused() {
    let r = root();
    std::fs::write(r.join("precious"), "mine").unwrap();
    std::fs::create_dir(r.join("pdir")).unwrap();
    let mut a = Step::new(
        "a",
        "Make a directory",
        StepAction::Mkdir {
            path: s(&r.join("made")),
            mode: None,
        },
    );
    a.undo = Undo::Remove {
        path: s(&r.join("precious")),
    };
    let mut b = Step::new(
        "b",
        "Write a file",
        StepAction::WriteFile {
            path: s(&r.join("wrote")),
            mode: None,
            content: Content::Inline { text: "x".into() },
        },
    );
    b.undo = Undo::Remove {
        path: s(&r.join("pdir")),
    };
    let p = plan(&r, vec![a, b]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(
        outcomes(&rep),
        vec![
            ("b", UndoOutcome::Left(Failure::NotOurs)),
            ("a", UndoOutcome::Left(Failure::NotOurs))
        ]
    );
    assert!(r.join("precious").exists() && r.join("pdir").exists());
}

#[test]
fn a_plan_whose_undo_would_remove_a_root_is_refused() {
    let r = root();
    let mut st = mkdir("a", &r.join("x"));
    st.undo = Undo::Remove { path: s(&r) };
    let e = ExecPlan::new(vec![s(&r)], vec![st], &[]).expect_err("must be refused");
    assert_eq!(e.problem, Problem::IsRoot);
    let mut st = mkdir("a", &r.join("x"));
    st.undo = Undo::Remove {
        path: s(&r.join(".")),
    };
    assert_eq!(
        ExecPlan::new(vec![s(&r)], vec![st], &[])
            .unwrap_err()
            .problem,
        Problem::IsRoot,
        "a trailing dot is the same root"
    );
}

#[test]
fn a_step_that_never_ran_has_nothing_to_undo_and_only_what_ran_is_reversed() {
    let r = root();
    std::fs::write(r.join("blocker"), "a file").unwrap();
    let p = plan(
        &r,
        vec![
            inline("one", &r.join("one"), "1"),
            mkdir("two", &r.join("blocker").join("sub")),
            inline("three", &r.join("three"), "3"),
        ],
    );
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    let rep = ex.run(p.hash(), &[]).unwrap();
    assert_eq!(rep.steps[1].status, StepStatus::Failed);
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(
        rep.steps[0],
        configurant::exec::UndoStep {
            id: "three".into(),
            outcome: UndoOutcome::NotApplied
        }
    );
    assert_eq!(rep.steps[2].outcome, UndoOutcome::Undone);
    assert!(!r.join("one").exists());
    assert!(
        r.join("blocker").exists(),
        "the blocker was never the plan's"
    );
}

#[test]
fn an_undo_action_goes_through_the_host() {
    let r = root();
    let mut svc = Step::new(
        "svc",
        "Start it",
        StepAction::Service {
            name: "app".into(),
            state: ServiceState::Up,
        },
    );
    svc.undo = Undo::Action {
        action: Box::new(StepAction::Service {
            name: "app".into(),
            state: ServiceState::Down,
        }),
    };
    let p = plan(&r, vec![svc]);
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    assert!(ex.undo(p.hash()).unwrap().complete());
    assert_eq!(
        h.services.borrow().as_slice(),
        [
            ("app".to_owned(), ServiceState::Up),
            ("app".to_owned(), ServiceState::Down)
        ]
    );
}

#[test]
fn undo_never_removes_through_a_link_out_of_the_roots() {
    let r = root();
    let outside = root();
    std::fs::write(outside.join("f"), "x").unwrap();
    let p = plan(
        &r,
        vec![
            mkdir("dir", &r.join("d")),
            inline("file", &r.join("d").join("f"), "x"),
        ],
    );
    let h = TestHost::default();
    let ex = Executor::new(&p, &h, r.join("st"));
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    // after the run, the directory is swapped for a link that leads out of the roots
    std::fs::remove_dir_all(r.join("d")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, r.join("d")).unwrap();
    #[cfg(windows)]
    {
        let o = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &s(&r.join("d")), &s(&outside)])
            .output()
            .unwrap();
        assert!(o.status.success(), "{o:?}");
    }
    let rep = ex.undo(p.hash()).unwrap();
    for step in &rep.steps {
        assert!(
            matches!(
                step.outcome,
                UndoOutcome::Failed(Failure::OutsideRoots | Failure::Symlink)
            ),
            "{rep:?}"
        );
    }
    assert_eq!(std::fs::read_to_string(outside.join("f")).unwrap(), "x");
    assert!(outside.exists());
}

#[test]
fn a_secret_file_is_removed_only_while_it_still_holds_the_secret_and_nothing_leaks() {
    let r = root();
    let mut key = Step::new(
        "key",
        "Write the key",
        StepAction::WriteFile {
            path: s(&r.join("k")),
            mode: None,
            content: Content::Secret {
                secret: SecretRef("api.key".into()),
            },
        },
    );
    key.undo = Undo::Remove {
        path: s(&r.join("k")),
    };
    let p = plan(&r, vec![key]);
    let h = TestHost::default();
    let st = r.join("st");
    let ex = Executor::new(&p, &h, &st);
    assert!(ex.run(p.hash(), &[]).unwrap().ok(&p));
    std::fs::write(r.join("k"), "rotated by hand").unwrap();
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(rep.steps[0].outcome, UndoOutcome::Left(Failure::Changed));
    std::fs::write(r.join("k"), CANARY).unwrap();
    let rep = ex.undo(p.hash()).unwrap();
    assert_eq!(rep.steps[0].outcome, UndoOutcome::Undone);
    assert!(!r.join("k").exists());
    let shown = format!("{rep:?} {}", std::fs::read_to_string(&st).unwrap());
    assert!(!shown.contains(CANARY), "{shown}");
}
