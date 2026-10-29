// SPDX-License-Identifier: MIT OR Apache-2.0
//! The plan executor, part b: [`Executor::run`] applies a confirmed [`ExecPlan`] step by step.
//!
//! Guarantees (each has a known-bad test in `tests/known_bad_exec_run.rs`):
//!
//! - **Bound to the hash.** `run` refuses unless the hash it is given is the plan's; nothing changes.
//! - **Inside the roots, for real.** Before a step touches a path, the nearest existing ancestor is resolved
//!   (symlinks and junctions followed) and must still be inside a resolved root; a file that is itself a
//!   symlink is never written through.
//! - **No secret leaves the host.** A [`SecretRef`] is resolved by the [`Host`] at the moment of use and goes
//!   only to a child process environment variable or a file created owner-only. The state file holds step ids,
//!   states and the plan hash. A report holds ids and a [`Failure`], whose text never has a value or a host
//!   message. A command output is discarded, since it may echo a secret.
//! - **Windows.** A file that carries a secret is written only if the host reports an owner-only ACL for it
//!   ([`Host::owner_only_acl`], default `false`); otherwise the step fails with [`Failure::AclRequired`].
//! - **No surprise overwrite.** A file that exists with different content is replaced only if its step id is in
//!   the `approved_overwrites` the front end passes after showing the user the overwrite.
//! - **Resumable.** The state file records each step as `pending`, `applied`, `verified` or `failed`. A run
//!   skips verified steps, stops at the first failure and the next run continues there. A state file for a
//!   different plan is refused.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{
    Check, Content, EnvValue, ExecPlan, SecretRef, ServiceState, Step, StepAction, is_sha256,
    sha256_hex,
};

/// A secret value held for the moment of use. `Debug` and `Display` never show it.
pub struct Secret(Vec<u8>);

impl Secret {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Secret(bytes.into())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// What the application provides for the parts a library cannot own.
pub trait Host {
    /// The value of a secret by name, or `None` if the host has none. Called at the moment of use.
    fn secret(&self, name: &str) -> Option<Secret>;

    /// The bytes whose SHA-256 is `sha256` (for `Content::Hash`), or `None`. The executor checks the hash.
    fn content(&self, sha256: &str) -> Option<Vec<u8>>;

    /// Is `path` protected so that only its owner can read it? Asked on Windows before a secret file is
    /// written. The default is `false`, so a host that does not implement it never gets a secret file there.
    fn owner_only_acl(&self, _path: &Path) -> bool {
        false
    }

    /// Bring a service up or down. The default declines, and the step fails.
    fn service(&self, _name: &str, _state: ServiceState) -> bool {
        false
    }
}

/// Where a step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepStatus {
    Pending,
    Applied,
    Verified,
    Failed,
}

/// Why a step failed. No value and no host message is ever in one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Failure {
    /// The path resolves (through a symlink or junction) outside the declared roots.
    OutsideRoots,
    /// The target file is a symlink; it is not written through.
    Symlink,
    /// A file where a directory is wanted, or the reverse.
    WrongKind,
    /// The file exists with different content and the step was not approved to overwrite it.
    OverwriteRefused,
    /// The host has no value for this secret.
    SecretUnavailable { name: String },
    /// The host has no bytes for this content hash.
    ContentUnavailable,
    /// The host's bytes do not hash to the plan's hash.
    ContentHashMismatch,
    /// Windows: a secret file needs an owner-only ACL and the host did not report one.
    AclRequired,
    /// The host declined to control the service.
    ServiceDeclined,
    /// The program exited with a failure status (`None`: ended by a signal).
    CommandFailed { code: Option<i32> },
    /// The program ran past its timeout and was killed.
    Timeout,
    /// The read-back after the step did not hold.
    VerifyFailed,
    /// An operating-system error, by kind only.
    Io(std::io::ErrorKind),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::OutsideRoots => f.write_str("the path resolves outside the declared roots"),
            Failure::Symlink => f.write_str("the target is a symlink"),
            Failure::WrongKind => f.write_str("a file and a directory are in each other's way"),
            Failure::OverwriteRefused => f.write_str(
                "the file exists with different content and the overwrite was not approved",
            ),
            Failure::SecretUnavailable { name } => write!(f, "the host has no secret named {name}"),
            Failure::ContentUnavailable => f.write_str("the host has no content for that hash"),
            Failure::ContentHashMismatch => f.write_str("the host content does not match its hash"),
            Failure::AclRequired => f.write_str(
                "a secret file needs an owner-only ACL on Windows and the host did not report one",
            ),
            Failure::ServiceDeclined => f.write_str("the host declined to control the service"),
            Failure::CommandFailed { code: Some(c) } => {
                write!(f, "the program exited with status {c}")
            }
            Failure::CommandFailed { code: None } => {
                f.write_str("the program was ended by a signal")
            }
            Failure::Timeout => f.write_str("the program ran past its timeout"),
            Failure::VerifyFailed => f.write_str("the read-back after the step did not hold"),
            Failure::Io(k) => write!(f, "operating-system error: {k}"),
        }
    }
}

/// What happened to one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepReport {
    pub id: String,
    pub status: StepStatus,
    /// Already verified (by an earlier run or by its `check`); nothing was done.
    pub skipped: bool,
    pub failure: Option<Failure>,
}

/// The result of a run: one entry per step reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub steps: Vec<StepReport>,
}

impl Report {
    /// Every step of the plan is verified.
    pub fn ok(&self, plan: &ExecPlan) -> bool {
        self.steps.len() == plan.steps().len()
            && self.steps.iter().all(|s| s.status == StepStatus::Verified)
    }
}

/// A run that did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunError {
    /// The hash given is not the plan's: the plan is not the one the user confirmed.
    HashMismatch,
    /// The state file belongs to a different plan; remove it to start over.
    StateMismatch,
    /// The state file could not be read or written, or is malformed.
    State(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::HashMismatch => {
                f.write_str("the plan hash is not the one that was confirmed; nothing was run")
            }
            RunError::StateMismatch => {
                f.write_str("the state file is for a different plan; remove it to start over")
            }
            RunError::State(m) => write!(f, "state file: {m}"),
        }
    }
}

impl std::error::Error for RunError {}

#[derive(Serialize, Deserialize)]
struct StateEntry {
    id: String,
    state: StepStatus,
}

#[derive(Serialize, Deserialize)]
struct StateFile {
    hash: String,
    step: Vec<StateEntry>,
}

/// Runs a confirmed plan against a host, keeping resumable state in a file.
pub struct Executor<'a, H: Host> {
    plan: &'a ExecPlan,
    host: &'a H,
    state_path: PathBuf,
}

impl<H: Host> fmt::Debug for Executor<'_, H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Executor")
            .field("plan", &self.plan.hash())
            .field("state_path", &self.state_path)
            .finish()
    }
}

impl<'a, H: Host> Executor<'a, H> {
    pub fn new(plan: &'a ExecPlan, host: &'a H, state_path: impl Into<PathBuf>) -> Self {
        Executor {
            plan,
            host,
            state_path: state_path.into(),
        }
    }

    fn load(&self) -> Result<StateFile, RunError> {
        match std::fs::read_to_string(&self.state_path) {
            Ok(text) => {
                let st: StateFile = toml::from_str(&text)
                    .map_err(|_| RunError::State("the file is not a valid state file".into()))?;
                if st.hash != self.plan.hash() {
                    return Err(RunError::StateMismatch);
                }
                Ok(st)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StateFile {
                hash: self.plan.hash().to_owned(),
                step: self
                    .plan
                    .steps()
                    .iter()
                    .map(|s| StateEntry {
                        id: s.id.clone(),
                        state: StepStatus::Pending,
                    })
                    .collect(),
            }),
            Err(e) => Err(RunError::State(format!("cannot read: {}", e.kind()))),
        }
    }

    fn save(&self, st: &StateFile) -> Result<(), RunError> {
        let text = toml::to_string(st).map_err(|_| RunError::State("cannot encode".into()))?;
        let tmp = self.state_path.with_extension("tmp");
        std::fs::write(&tmp, text)
            .and_then(|_| std::fs::rename(&tmp, &self.state_path))
            .map_err(|e| RunError::State(format!("cannot write: {}", e.kind())))
    }

    /// Apply the plan. `confirmed_hash` must be the plan's hash. `approved_overwrites` are the ids of steps the
    /// user saw as `overwrite` and accepted. Stops at the first failed step; the report says which.
    pub fn run(
        &self,
        confirmed_hash: &str,
        approved_overwrites: &[&str],
    ) -> Result<Report, RunError> {
        if confirmed_hash != self.plan.hash() {
            return Err(RunError::HashMismatch);
        }
        let mut st = self.load()?;
        let mut report = Report { steps: Vec::new() };
        for (i, step) in self.plan.steps().iter().enumerate() {
            if st.step[i].state == StepStatus::Verified {
                report.steps.push(StepReport {
                    id: step.id.clone(),
                    status: StepStatus::Verified,
                    skipped: true,
                    failure: None,
                });
                continue;
            }
            let approved = approved_overwrites.contains(&step.id.as_str());
            let mut skipped = false;
            let outcome = self.one(step, approved, &mut skipped, |s| {
                st.step[i].state = s;
                self.save(&st)
            });
            match outcome {
                Ok(()) => {
                    st.step[i].state = StepStatus::Verified;
                    self.save(&st)?;
                    report.steps.push(StepReport {
                        id: step.id.clone(),
                        status: StepStatus::Verified,
                        skipped,
                        failure: None,
                    });
                }
                Err(f) => {
                    st.step[i].state = StepStatus::Failed;
                    self.save(&st)?;
                    report.steps.push(StepReport {
                        id: step.id.clone(),
                        status: StepStatus::Failed,
                        skipped: false,
                        failure: Some(f),
                    });
                    return Ok(report);
                }
            }
        }
        Ok(report)
    }

    fn one(
        &self,
        step: &Step,
        approved: bool,
        skipped: &mut bool,
        mut mark: impl FnMut(StepStatus) -> Result<(), RunError>,
    ) -> Result<(), Failure> {
        if let Some(c) = &step.check
            && self.holds(c)?
        {
            *skipped = true;
            return Ok(());
        }
        let written = self.apply(&step.action, approved)?;
        mark(StepStatus::Applied).map_err(|_| Failure::Io(std::io::ErrorKind::Other))?;
        match &step.verify {
            Some(c) => {
                if self.holds(c)? {
                    Ok(())
                } else {
                    Err(Failure::VerifyFailed)
                }
            }
            None => self.implicit_verify(&step.action, written.as_deref()),
        }
    }

    fn holds(&self, c: &Check) -> Result<bool, Failure> {
        match c {
            Check::PathExists { path } => {
                self.inside(Path::new(path))?;
                Ok(Path::new(path).exists())
            }
            Check::FileHash { path, sha256 } => {
                self.inside(Path::new(path))?;
                Ok(std::fs::read(path)
                    .map(|b| sha256_hex(&b) == sha256.to_ascii_lowercase())
                    .unwrap_or(false))
            }
            Check::CommandOk { argv } => {
                Ok(spawn(argv, None, &[], Duration::from_secs(60)) == Ok(Some(0)))
            }
        }
    }

    fn inside(&self, path: &Path) -> Result<(), Failure> {
        let p = resolve(path);
        if self
            .plan
            .roots()
            .iter()
            .any(|r| p.starts_with(resolve(Path::new(r))))
        {
            Ok(())
        } else {
            Err(Failure::OutsideRoots)
        }
    }

    /// Returns the bytes written, so the implicit read-back can compare without asking the host again.
    fn apply(&self, action: &StepAction, approved: bool) -> Result<Option<Vec<u8>>, Failure> {
        match action {
            StepAction::Mkdir { path, mode } => {
                let p = Path::new(path);
                self.inside(p)?;
                if p.exists() && !p.is_dir() {
                    return Err(Failure::WrongKind);
                }
                std::fs::create_dir_all(p).map_err(io)?;
                set_mode(p, *mode, false)?;
                Ok(None)
            }
            StepAction::WriteFile {
                path,
                mode,
                content,
            } => {
                let p = Path::new(path);
                self.inside(p)?;
                let secret = matches!(content, Content::Secret { .. });
                if secret && cfg!(windows) && !self.host.owner_only_acl(p) {
                    return Err(Failure::AclRequired);
                }
                let bytes = self.bytes(content)?;
                match std::fs::symlink_metadata(p) {
                    Ok(m) if m.file_type().is_symlink() => return Err(Failure::Symlink),
                    Ok(m) if m.is_dir() => return Err(Failure::WrongKind),
                    Ok(_) => {
                        let have = std::fs::read(p).map_err(io)?;
                        if have == bytes {
                            set_mode(p, *mode, secret)?;
                            return Ok(Some(bytes));
                        }
                        if !approved {
                            return Err(Failure::OverwriteRefused);
                        }
                    }
                    Err(_) => {}
                }
                write_file(p, &bytes, secret).map_err(io)?;
                set_mode(p, *mode, secret)?;
                Ok(Some(bytes))
            }
            StepAction::Run {
                argv,
                cwd,
                timeout_secs,
                env,
            } => {
                if let Some(c) = cwd {
                    self.inside(Path::new(c))?;
                }
                let mut vars = Vec::new();
                for e in env {
                    let v = match &e.value {
                        EnvValue::Literal { text } => text.clone().into_bytes(),
                        EnvValue::Secret { secret } => self.secret(secret)?,
                    };
                    vars.push((e.name.clone(), v));
                }
                match spawn(
                    argv,
                    cwd.as_deref(),
                    &vars,
                    Duration::from_secs(*timeout_secs),
                ) {
                    Ok(Some(0)) => Ok(None),
                    Ok(code) => Err(Failure::CommandFailed { code }),
                    Err(f) => Err(f),
                }
            }
            StepAction::Service { name, state } => {
                if self.host.service(name, *state) {
                    Ok(None)
                } else {
                    Err(Failure::ServiceDeclined)
                }
            }
        }
    }

    fn secret(&self, r: &SecretRef) -> Result<Vec<u8>, Failure> {
        self.host
            .secret(&r.0)
            .map(|s| s.0)
            .ok_or_else(|| Failure::SecretUnavailable { name: r.0.clone() })
    }

    fn bytes(&self, c: &Content) -> Result<Vec<u8>, Failure> {
        match c {
            Content::Inline { text } => Ok(text.clone().into_bytes()),
            Content::Hash { sha256 } => {
                debug_assert!(is_sha256(sha256));
                let b = self
                    .host
                    .content(sha256)
                    .ok_or(Failure::ContentUnavailable)?;
                if sha256_hex(&b) == sha256.to_ascii_lowercase() {
                    Ok(b)
                } else {
                    Err(Failure::ContentHashMismatch)
                }
            }
            Content::Secret { secret } => self.secret(secret),
        }
    }

    fn implicit_verify(&self, action: &StepAction, written: Option<&[u8]>) -> Result<(), Failure> {
        let ok = match action {
            StepAction::Mkdir { path, .. } => Path::new(path).is_dir(),
            StepAction::WriteFile { path, .. } => match (std::fs::read(path), written) {
                (Ok(have), Some(want)) => have == want,
                _ => false,
            },
            StepAction::Run { .. } | StepAction::Service { .. } => true,
        };
        if ok {
            Ok(())
        } else {
            Err(Failure::VerifyFailed)
        }
    }
}

fn io(e: std::io::Error) -> Failure {
    Failure::Io(e.kind())
}

/// The path with its nearest existing ancestor resolved through symlinks and junctions.
fn resolve(p: &Path) -> PathBuf {
    let mut tail = Vec::new();
    let mut cur = p.to_path_buf();
    loop {
        if let Ok(c) = cur.canonicalize() {
            let mut out = c;
            for t in tail.iter().rev() {
                out.push(t);
            }
            return out;
        }
        match (cur.file_name().map(|f| f.to_owned()), cur.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                cur = parent.to_path_buf();
            }
            _ => return p.to_path_buf(),
        }
    }
}

fn write_file(p: &Path, bytes: &[u8], secret: bool) -> std::io::Result<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if secret {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = secret;
    let mut f = o.open(p)?;
    f.write_all(bytes)
}

/// Apply `mode` on Unix (a secret file keeps owner bits only); advisory, and ignored, elsewhere.
fn set_mode(p: &Path, mode: Option<u32>, secret: bool) -> Result<(), Failure> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let want = match (mode, secret) {
            (Some(m), true) => Some(m & 0o700),
            (None, true) => Some(0o600),
            (m, false) => m,
        };
        if let Some(m) = want {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).map_err(io)?;
        }
    }
    #[cfg(not(unix))]
    let _ = (p, mode, secret);
    Ok(())
}

/// Run `argv[0]` with the rest as arguments. Output is discarded. `Ok(Some(code))`, `Ok(None)` for a signal.
fn spawn(
    argv: &[String],
    cwd: Option<&str>,
    env: &[(String, Vec<u8>)],
    timeout: Duration,
) -> Result<Option<i32>, Failure> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    for (k, v) in env {
        cmd.env(k, bytes_to_os(v));
    }
    let mut child = cmd.spawn().map_err(io)?;
    let start = Instant::now();
    loop {
        match child.try_wait().map_err(io)? {
            Some(status) => return Ok(status.code()),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Failure::Timeout);
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn bytes_to_os(v: &[u8]) -> std::ffi::OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(v.to_vec())
    }
    #[cfg(not(unix))]
    {
        std::ffi::OsString::from(String::from_utf8_lossy(v).into_owned())
    }
}
