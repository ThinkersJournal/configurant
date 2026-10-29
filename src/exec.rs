// SPDX-License-Identifier: MIT OR Apache-2.0
//! The plan executor, part a (ROADMAP row 28, `docs/PLAN-EXECUTOR.md`): steps as data, their validation,
//! the plan hash, and an inert [`ExecPlan::describe`] for the confirm screen.
//!
//! A [`Step`] is a directory to make, a file to write, a command to run (an argv array, never a shell
//! string) or a service to bring up or down. An [`ExecPlan`] is a validated list of steps inside declared
//! roots. Building one and describing it changes nothing: `describe` reads the filesystem and writes nothing.
//! [`Executor::run`] (part b) applies a plan; undo and real service control are part c. Pass the plan hash that the
//! user confirmed (`plan.hash()`, the same value `describe()` shows) to `run`; keep the state file outside the
//! roots or inside them, as you like, but on a path of your own.
//!
//! Roots and paths are plain `String`s. The root check is lexical (a path must start with a root, with no `..`):
//! it does not follow symlinks or fold case, so the run step (a later part) must check the real path again.
//! A `Literal` environment value is shown as written, so it is for non-secret text only.
//!
//! A plan never holds a secret. A secret enters as a [`SecretRef`] (a name the host resolves at run time),
//! and [`ExecPlan::new`] refuses a plan in which any known secret value appears in a title, path, argv,
//! environment literal, inline content or service name. An error names the step and the field, never the value.
//!
//! ```
//! use configurant::exec::{ExecPlan, Step, StepAction};
//!
//! let plan = ExecPlan::new(
//!     vec!["/opt/app".into()],
//!     vec![Step::new("dir", "Create the data directory", StepAction::Mkdir {
//!         path: "/opt/app/data".into(),
//!         mode: Some(0o750),
//!     })],
//!     &[], // secret values the plan must not contain
//! )
//! .unwrap();
//! let shown = plan.describe(); // nothing has changed on disk
//! assert_eq!(shown.hash, plan.hash());
//! assert_eq!(shown.steps[0].id, "dir");
//! ```

mod run;
pub use run::{Executor, Failure, Host, Report, RunError, Secret, StepReport, StepStatus};

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The most inline text a `WriteFile` may carry (64 KiB). Larger content comes by hash from the host.
pub const INLINE_LIMIT: usize = 64 * 1024;

/// A secret by name. The host resolves it when the plan runs; the plan never holds the value.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct SecretRef(pub String);

impl fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretRef({})", self.0)
    }
}

/// What a `WriteFile` writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Content {
    /// Small non-secret text, up to [`INLINE_LIMIT`] bytes, hashed into the plan.
    Inline { text: String },
    /// Bytes the host supplies at run time, named by their SHA-256 (64 hex digits).
    Hash { sha256: String },
    /// A file that carries or derives from a secret. The host delivers it; the plan holds the name only.
    Secret { secret: SecretRef },
}

/// A value in a command's environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EnvValue {
    Literal { text: String },
    Secret { secret: SecretRef },
}

/// One environment variable of a `Run` step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvVar {
    pub name: String,
    pub value: EnvValue,
}

/// Which way a service goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceState {
    Up,
    Down,
}

/// What a step does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StepAction {
    /// Make a directory (and its parents). `mode` is advisory on Windows.
    Mkdir { path: String, mode: Option<u32> },
    /// Write a file. `mode` is advisory on Windows.
    WriteFile {
        path: String,
        mode: Option<u32>,
        content: Content,
    },
    /// Run a program. `argv[0]` is the program; there is no shell and no string to split.
    Run {
        argv: Vec<String>,
        cwd: Option<String>,
        timeout_secs: u64,
        env: Vec<EnvVar>,
    },
    /// Bring a service up or down. The host implements this; a container is a service.
    Service { name: String, state: ServiceState },
}

/// A condition the executor evaluates: before a step to see whether it is already done, after it to verify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Check {
    PathExists { path: String },
    FileHash { path: String, sha256: String },
    CommandOk { argv: Vec<String> },
}

/// How a step is undone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Undo {
    /// Forward-only: it cannot be undone, and the confirm screen says so.
    None,
    /// Undo by running this action.
    Action { action: Box<StepAction> },
}

/// One step of an executable plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    /// Unique in the plan.
    pub id: String,
    /// A plain sentence for the confirm screen.
    pub title: String,
    pub action: StepAction,
    /// Evaluated before the step: if it holds, the step is already done.
    pub check: Option<Check>,
    /// Read back after the step.
    pub verify: Option<Check>,
    pub undo: Undo,
}

impl Step {
    /// A forward-only step with no check and no verify.
    pub fn new(id: &str, title: &str, action: StepAction) -> Self {
        Step {
            id: id.to_owned(),
            title: title.to_owned(),
            action,
            check: None,
            verify: None,
            undo: Undo::None,
        }
    }
}

/// Why a plan was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Problem {
    Empty,
    DuplicateId,
    NoRoots,
    ParentDir,
    OutsideRoots,
    TooLarge {
        bytes: usize,
    },
    BadHash,
    BadMode,
    ZeroTimeout,
    BadName,
    /// A known secret value appears in the field.
    SecretValue,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Empty => f.write_str("must not be empty"),
            Problem::DuplicateId => f.write_str("the id is used by another step"),
            Problem::NoRoots => f.write_str("the plan declares no root directory"),
            Problem::ParentDir => f.write_str("a path must not contain .."),
            Problem::OutsideRoots => f.write_str("the path is outside the plan's declared roots"),
            Problem::TooLarge { bytes } => write!(
                f,
                "{bytes} bytes inline is over the {INLINE_LIMIT} byte limit; supply it by hash"
            ),
            Problem::BadHash => f.write_str("expected a SHA-256 as 64 hex digits"),
            Problem::BadMode => f.write_str("a mode is at most 0o7777"),
            Problem::ZeroTimeout => f.write_str("the timeout must be at least one second"),
            Problem::BadName => f.write_str("the name has a character it must not have"),
            Problem::SecretValue => {
                f.write_str("holds a secret value; pass it as a SecretRef instead")
            }
        }
    }
}

/// A refused plan: the step (empty for the plan itself), the field and the problem. Never a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("step {step:?}: {field}: {problem}")]
pub struct ExecError {
    pub step: String,
    pub field: &'static str,
    pub problem: Problem,
}

fn err(step: &str, field: &'static str, problem: Problem) -> ExecError {
    ExecError {
        step: step.to_owned(),
        field,
        problem,
    }
}

/// What a step would do to the machine, as far as can be told without running it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    Create,
    Overwrite,
    Unchanged,
    /// The path exists and is the wrong kind (a file where a directory is wanted, or the reverse).
    Conflict,
    Run,
    Service,
}

/// One step for the confirm screen. Secrets show as `<redacted ref NAME>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepDescription {
    pub id: String,
    pub title: String,
    /// `mkdir`, `write-file`, `run` or `service`.
    pub kind: &'static str,
    pub effect: Effect,
    pub path: Option<String>,
    pub mode: Option<u32>,
    /// The program and its arguments, as the plan holds them (never a secret value).
    pub command: Option<Vec<String>>,
    /// Environment variables of a `Run` step as `NAME=value`; a secret is `NAME=<redacted ref SECRET>`.
    pub env: Vec<String>,
    pub service: Option<String>,
    pub reversible: bool,
}

/// The data the confirm screen shows. Serialisable; a TUI, a window and a web page render it alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Description {
    pub hash: String,
    pub roots: Vec<String>,
    pub steps: Vec<StepDescription>,
    /// Things the user should know before confirming (for example, that a mode is advisory here).
    pub notes: Vec<String>,
}

/// A validated list of steps inside declared roots, with its hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecPlan {
    roots: Vec<String>,
    steps: Vec<Step>,
    hash: String,
}

#[derive(Serialize)]
struct Canon<'a> {
    roots: &'a [String],
    steps: &'a [Step],
}

fn hex(bytes: &[u8]) -> String {
    use fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// SHA-256 of `bytes` as lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The path with `.` removed, or an error for `..`.
fn clean(step: &str, field: &'static str, p: &str) -> Result<PathBuf, ExecError> {
    if p.is_empty() {
        return Err(err(step, field, Problem::Empty));
    }
    let mut out = PathBuf::new();
    for c in Path::new(p).components() {
        match c {
            Component::ParentDir => return Err(err(step, field, Problem::ParentDir)),
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

struct Ctx<'a> {
    roots: Vec<PathBuf>,
    secrets: Vec<&'a str>,
}

impl Ctx<'_> {
    fn path(&self, step: &str, field: &'static str, p: &str) -> Result<(), ExecError> {
        let c = clean(step, field, p)?;
        if self.roots.iter().any(|r| c.starts_with(r)) {
            Ok(())
        } else {
            Err(err(step, field, Problem::OutsideRoots))
        }
    }

    fn text(&self, step: &str, field: &'static str, t: &str) -> Result<(), ExecError> {
        if self.secrets.iter().any(|s| t.contains(s)) {
            Err(err(step, field, Problem::SecretValue))
        } else {
            Ok(())
        }
    }

    fn argv(&self, step: &str, field: &'static str, argv: &[String]) -> Result<(), ExecError> {
        if argv.is_empty() || argv[0].is_empty() {
            return Err(err(step, field, Problem::Empty));
        }
        argv.iter().try_for_each(|a| self.text(step, field, a))
    }

    fn check(&self, step: &str, field: &'static str, c: &Check) -> Result<(), ExecError> {
        match c {
            Check::PathExists { path } => self.path(step, field, path),
            Check::FileHash { path, sha256 } => {
                self.path(step, field, path)?;
                if is_sha256(sha256) {
                    Ok(())
                } else {
                    Err(err(step, field, Problem::BadHash))
                }
            }
            Check::CommandOk { argv } => self.argv(step, field, argv),
        }
    }

    fn action(&self, step: &str, a: &StepAction) -> Result<(), ExecError> {
        match a {
            StepAction::Mkdir { path, mode } => {
                self.path(step, "path", path)?;
                self.text(step, "path", path)?;
                mode_ok(step, *mode)
            }
            StepAction::WriteFile {
                path,
                mode,
                content,
            } => {
                self.path(step, "path", path)?;
                self.text(step, "path", path)?;
                mode_ok(step, *mode)?;
                match content {
                    Content::Inline { text } => {
                        if text.len() > INLINE_LIMIT {
                            return Err(err(
                                step,
                                "content",
                                Problem::TooLarge { bytes: text.len() },
                            ));
                        }
                        self.text(step, "content", text)
                    }
                    Content::Hash { sha256 } => {
                        if is_sha256(sha256) {
                            Ok(())
                        } else {
                            Err(err(step, "content", Problem::BadHash))
                        }
                    }
                    Content::Secret { secret } => name_ok(step, "content", &secret.0),
                }
            }
            StepAction::Run {
                argv,
                cwd,
                timeout_secs,
                env,
            } => {
                self.argv(step, "argv", argv)?;
                if let Some(cwd) = cwd {
                    self.path(step, "cwd", cwd)?;
                }
                if *timeout_secs == 0 {
                    return Err(err(step, "timeout_secs", Problem::ZeroTimeout));
                }
                for e in env {
                    if e.name.is_empty() || e.name.contains('=') || e.name.contains('\0') {
                        return Err(err(step, "env", Problem::BadName));
                    }
                    match &e.value {
                        EnvValue::Literal { text } => self.text(step, "env", text)?,
                        EnvValue::Secret { secret } => name_ok(step, "env", &secret.0)?,
                    }
                }
                Ok(())
            }
            StepAction::Service { name, .. } => {
                if name.is_empty() {
                    return Err(err(step, "service", Problem::Empty));
                }
                if name.chars().any(|c| c.is_whitespace() || c.is_control()) {
                    return Err(err(step, "service", Problem::BadName));
                }
                self.text(step, "service", name)
            }
        }
    }
}

fn mode_ok(step: &str, mode: Option<u32>) -> Result<(), ExecError> {
    match mode {
        Some(m) if m > 0o7777 => Err(err(step, "mode", Problem::BadMode)),
        _ => Ok(()),
    }
}

fn name_ok(step: &str, field: &'static str, name: &str) -> Result<(), ExecError> {
    if name.is_empty() {
        Err(err(step, field, Problem::Empty))
    } else if name.chars().any(|c| c.is_control()) {
        Err(err(step, field, Problem::BadName))
    } else {
        Ok(())
    }
}

impl ExecPlan {
    /// Validate `steps` against the declared `roots` and the known secret values (`secrets`: values the plan
    /// must not contain). Nothing is touched.
    pub fn new(
        roots: Vec<String>,
        steps: Vec<Step>,
        secrets: &[&str],
    ) -> Result<ExecPlan, ExecError> {
        if roots.is_empty() {
            return Err(err("", "roots", Problem::NoRoots));
        }
        let mut cleaned = Vec::new();
        for r in &roots {
            cleaned.push(clean("", "roots", r)?);
        }
        let ctx = Ctx {
            roots: cleaned,
            secrets: secrets.iter().copied().filter(|s| !s.is_empty()).collect(),
        };
        let mut seen = BTreeSet::new();
        for s in &steps {
            if s.id.is_empty() {
                return Err(err("", "id", Problem::Empty));
            }
            if !seen.insert(s.id.clone()) {
                return Err(err(&s.id, "id", Problem::DuplicateId));
            }
            if s.title.is_empty() {
                return Err(err(&s.id, "title", Problem::Empty));
            }
            ctx.text(&s.id, "title", &s.title)?;
            ctx.action(&s.id, &s.action)?;
            if let Some(c) = &s.check {
                ctx.check(&s.id, "check", c)?;
            }
            if let Some(c) = &s.verify {
                ctx.check(&s.id, "verify", c)?;
            }
            if let Undo::Action { action } = &s.undo {
                ctx.action(&s.id, action)?;
            }
        }
        let canon = Canon {
            roots: &roots,
            steps: &steps,
        };
        let text = toml::to_string(&canon).map_err(|_| err("", "plan", Problem::BadName))?;
        let hash = sha256_hex(text.as_bytes());
        Ok(ExecPlan { roots, steps, hash })
    }

    /// The steps that a module's [`crate::Plan`] carries as `ActionKind::Step`, in order.
    pub fn from_plan(
        roots: Vec<String>,
        plan: &crate::Plan,
        secrets: &[&str],
    ) -> Result<ExecPlan, ExecError> {
        let steps = plan
            .actions
            .iter()
            .filter_map(|a| match &a.kind {
                crate::ActionKind::Step(s) => Some((**s).clone()),
                _ => None,
            })
            .collect();
        ExecPlan::new(roots, steps, secrets)
    }

    /// The plan hash (SHA-256, hex). A later run is bound to it: a plan that differs from the one shown is refused.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    /// The confirm-screen data. Reads the filesystem to say create, overwrite or unchanged; writes nothing.
    pub fn describe(&self) -> Description {
        let mut notes = Vec::new();
        if cfg!(windows)
            && self.steps.iter().any(|s| {
                matches!(
                    s.action,
                    StepAction::Mkdir { mode: Some(_), .. }
                        | StepAction::WriteFile { mode: Some(_), .. }
                )
            })
        {
            notes.push("File modes are advisory on Windows and are not applied.".to_owned());
        }
        if self.steps.iter().any(|s| s.undo == Undo::None) {
            notes.push("Some steps cannot be undone (forward-only).".to_owned());
        }
        Description {
            hash: self.hash.clone(),
            roots: self.roots.clone(),
            steps: self.steps.iter().map(describe_step).collect(),
            notes,
        }
    }
}

fn describe_step(s: &Step) -> StepDescription {
    let mut d = StepDescription {
        id: s.id.clone(),
        title: s.title.clone(),
        kind: "",
        effect: Effect::Run,
        path: None,
        mode: None,
        command: None,
        env: Vec::new(),
        service: None,
        reversible: s.undo != Undo::None,
    };
    match &s.action {
        StepAction::Mkdir { path, mode } => {
            d.kind = "mkdir";
            d.path = Some(path.clone());
            d.mode = *mode;
            d.effect = match std::fs::metadata(path) {
                Ok(m) if m.is_dir() => Effect::Unchanged,
                Ok(_) => Effect::Conflict,
                Err(_) => Effect::Create,
            };
        }
        StepAction::WriteFile {
            path,
            mode,
            content,
        } => {
            d.kind = "write-file";
            d.path = Some(path.clone());
            d.mode = *mode;
            let wanted = match content {
                Content::Inline { text } => Some(sha256_hex(text.as_bytes())),
                Content::Hash { sha256 } => Some(sha256.to_ascii_lowercase()),
                Content::Secret { .. } => None,
            };
            d.effect = match std::fs::metadata(path) {
                Err(_) => Effect::Create,
                Ok(m) if m.is_dir() => Effect::Conflict,
                Ok(_) => match (std::fs::read(path), wanted) {
                    (Ok(have), Some(w)) if sha256_hex(&have) == w => Effect::Unchanged,
                    _ => Effect::Overwrite,
                },
            };
        }
        StepAction::Run { argv, env, .. } => {
            d.kind = "run";
            let mut shown = Vec::new();
            for e in env {
                match &e.value {
                    EnvValue::Literal { text } => shown.push(format!("{}={}", e.name, text)),
                    EnvValue::Secret { secret } => {
                        shown.push(format!("{}=<redacted ref {}>", e.name, secret.0))
                    }
                }
            }
            d.command = Some(argv.clone());
            d.env = shown;
            d.effect = Effect::Run;
        }
        StepAction::Service { name, state } => {
            d.kind = "service";
            d.service = Some(format!(
                "{name} {}",
                match state {
                    ServiceState::Up => "up",
                    ServiceState::Down => "down",
                }
            ));
            d.effect = Effect::Service;
        }
    }
    d
}
