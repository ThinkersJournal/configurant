// SPDX-License-Identifier: MIT OR Apache-2.0
//! Executing a `DataTransfer` between two PersistAnt backends (`docs/TRANSFER.md`).
//!
//! [`PersistantModule::transfer`] opens both ends through the registry, copies every record (with the
//! time to live it has left), then reads both sides back and compares them. It never changes or deletes the
//! source and refuses a target that already holds records. The front end writes the configuration file
//! only after this returns `Ok`, so a failed copy leaves the old backend configured and its data intact.
//!
//! Endpoints can be secrets: nothing here prints one, and every backend message is scrubbed of the
//! endpoint URL (and its password) before it goes into an error.
//!
//! Only the key-value kind is copied. Leases, blob and relational data are not, and the report says so.

use std::fmt;

use ::persistant::capability::provider::{EndpointSpec, ProviderNeeds};
use ::persistant::kv::copy::{self, CopyError, CopyOptions, Phase, Side};
use ::persistant::kv::{KvBackend, KvError};
use ::persistant::registry::{self, Endpoint};

use super::{Durability, PersistantError, PersistantModule};
use crate::module::Needs;

/// What the report says is left alone.
pub const NOT_COPIED: &str = "leases, blob and relational data are not copied";

/// Turn what a person typed into the endpoint a backend opens from. `spec` is what the backend asks for
/// (`PersistantModule::endpoint_spec`). A connection URL is a secret, so it is **never typed**: a URL-kind
/// backend takes `env:NAME`, the name of an environment variable holding the URL, and a typed URL is refused
/// without being echoed. A path backend takes a path; a backend that needs nothing takes empty or `none`.
pub fn parse_endpoint(text: &str, spec: &EndpointSpec) -> Result<Endpoint, String> {
    let text = text.trim();
    let looks_like_url = text.contains("://");
    match spec {
        EndpointSpec::None => match text {
            "" | "none" => Ok(Endpoint::None),
            _ => Err("this backend needs no endpoint: leave it empty".to_owned()),
        },
        EndpointSpec::Path => {
            if looks_like_url {
                Err("this backend opens from a path, not a URL".to_owned())
            } else if text.is_empty() {
                Err("give the path (a directory or file)".to_owned())
            } else {
                Ok(Endpoint::path(text))
            }
        }
        EndpointSpec::Url { schemes } => {
            if let Some(name) = text.strip_prefix("env:") {
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    Ok(Endpoint::url_env(name))
                } else {
                    Err("after `env:` give a variable name (letters, digits, `_`)".to_owned())
                }
            } else if looks_like_url {
                Err(format!(
                    "a connection URL is a secret and is never typed here; put it in an environment variable and answer `env:NAME` (schemes: {})",
                    schemes.join(", ")
                ))
            } else {
                Err(
                    "answer `env:NAME`, the environment variable that holds the connection URL"
                        .to_owned(),
                )
            }
        }
        _ => Err("this build does not know how to ask for that kind of endpoint".to_owned()),
    }
}

/// One end of a transfer: a backend id and where to open it. `Debug` shows the id and the *kind* of
/// endpoint only, never a path or URL.
pub struct TransferEnd {
    pub id: String,
    pub endpoint: Endpoint,
}

impl TransferEnd {
    pub fn new(id: &str, endpoint: Endpoint) -> Self {
        TransferEnd {
            id: id.to_owned(),
            endpoint,
        }
    }
}

impl fmt::Debug for TransferEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match &self.endpoint {
            Endpoint::None => "none",
            Endpoint::Path(_) => "path",
            Endpoint::Url(_) | Endpoint::Secret(_) => "url (hidden)",
            _ => "endpoint",
        };
        f.debug_struct("TransferEnd")
            .field("id", &self.id)
            .field("endpoint", &kind)
            .finish()
    }
}

/// How to run a transfer.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TransferOptions {
    /// Copy into a target that already holds records, keeping the value of any key already there.
    pub allow_existing: bool,
    /// Records read from the source per page.
    pub page_size: usize,
    /// Count the source's records first, so [`TransferProgress::total`] is known. One extra key-only scan.
    pub count_first: bool,
}

impl Default for TransferOptions {
    fn default() -> Self {
        TransferOptions {
            allow_existing: false,
            page_size: 256,
            count_first: false,
        }
    }
}

impl TransferOptions {
    #[must_use]
    pub fn allow_existing(mut self, yes: bool) -> Self {
        self.allow_existing = yes;
        self
    }

    #[must_use]
    pub fn page_size(mut self, n: usize) -> Self {
        self.page_size = n.max(1);
        self
    }

    /// Count the source first so a progress report has a total (see [`TransferProgress`]).
    #[must_use]
    pub fn count_first(mut self, yes: bool) -> Self {
        self.count_first = yes;
        self
    }
}

/// The part of a transfer a [`TransferProgress`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransferPhase {
    /// Records are being written to the target.
    Copy,
    /// The copy is being read back and compared with the source.
    Verify,
}

/// How far a transfer has got, in records of the source handled so far in this phase. Counts only: no key,
/// value or endpoint. Reported once at the start of each phase (`done` is 0) and after every record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TransferProgress {
    pub phase: TransferPhase,
    pub done: u64,
    /// The source's record count when the transfer began, if [`TransferOptions::count_first`] asked for it.
    pub total: Option<u64>,
}

/// What a finished transfer did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TransferReport {
    /// Records written to the target.
    pub copied: usize,
    /// Of those, how many carried a time to live.
    pub with_ttl: usize,
    /// Records left alone because the target already had the key (`allow_existing` only).
    pub skipped: usize,
    /// Records read back from the target and found equal to the source.
    pub verified: usize,
}

impl fmt::Display for TransferReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "copied {} records ({} with a time to live), skipped {} already present, verified {}; {NOT_COPIED}",
            self.copied, self.with_ttl, self.skipped, self.verified
        )
    }
}

/// Why a transfer did not finish. Carries ids, counts and keys; never a value or an endpoint.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TransferError {
    #[error("the project's needs cannot be turned into a backend gate: {0}")]
    Needs(PersistantError),
    #[error(
        "the source and the target are both backend `{0}`; a transfer needs two different backends"
    )]
    SameBackend(String),
    #[error("could not open the {side}: {reason}")]
    Open { side: &'static str, reason: String },
    #[error(
        "the target already holds records; nothing was copied (allow_existing keeps what is there)"
    )]
    TargetNotEmpty,
    #[error(
        "the {side} backend does not support `{op}`, which the copy needs; nothing was changed"
    )]
    Unsupported { side: &'static str, op: String },
    #[error("the {side} backend failed during `{op}`: {reason}")]
    Backend {
        side: &'static str,
        op: &'static str,
        reason: String,
    },
    #[error(
        "the copy does not match the source: {missing} missing, {differing} different, {extra} extra \
         (first keys: {sample:?}); the source is untouched"
    )]
    Mismatch {
        missing: usize,
        differing: usize,
        extra: usize,
        sample: Vec<String>,
    },
    #[error("could not start the runtime: {0}")]
    Runtime(String),
}

fn kv_err(side: &'static str, op: &'static str, e: KvError, hide: &[String]) -> TransferError {
    match e {
        KvError::Unsupported(_) => TransferError::Unsupported {
            side,
            op: op.to_owned(),
        },
        other => TransferError::Backend {
            side,
            op,
            reason: scrub(other.to_string(), hide),
        },
    }
}

/// Replace every secret in `text` (a longer secret first, so a password inside a URL is not left over).
fn scrub(mut text: String, hide: &[String]) -> String {
    let mut sorted: Vec<&String> = hide.iter().filter(|s| !s.is_empty()).collect();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for s in sorted {
        text = text.replace(s.as_str(), "<hidden>");
    }
    text
}

/// What must never appear: the URL, and the password inside it.
fn secrets_of(endpoint: &Endpoint) -> Vec<String> {
    let url = match endpoint {
        Endpoint::Url(u) => Some(u.clone()),
        Endpoint::Secret(sec) => sec.resolve().ok().map(|r| r.expose().to_string()),
        _ => None,
    };
    let Some(url) = url else { return Vec::new() };
    let mut out = vec![url.clone()];
    if let Some((_, rest)) = url.split_once("://")
        && let Some((userinfo, _)) = rest.rsplit_once('@')
    {
        out.push(userinfo.to_owned());
        if let Some((_, password)) = userinfo.split_once(':') {
            out.push(password.to_owned());
        }
    }
    out
}

impl PersistantModule {
    /// What the backend `id` opens from, or `None` if this build does not offer it.
    pub fn endpoint_spec(&self, id: &str) -> Option<EndpointSpec> {
        let any = ProviderNeeds::new(Vec::new(), Durability::Volatile);
        registry::providers(&any)
            .ok()?
            .into_iter()
            .find(|l| l.id == id)
            .map(|l| l.endpoint)
    }

    /// Copy every record of the backend `from` into the backend `to`, then verify the copy. `needs` gate
    /// the *target* (the backend the project is moving to); the source is opened with no requirement, so a
    /// backend that no longer meets the needs can still be read. The source is never modified.
    pub async fn transfer(
        &self,
        from: &TransferEnd,
        to: &TransferEnd,
        needs: &Needs,
        options: &TransferOptions,
    ) -> Result<TransferReport, TransferError> {
        self.transfer_with_progress(from, to, needs, options, &mut |_| {})
            .await
    }

    /// As [`Self::transfer`], calling `progress` as the work goes (see [`TransferProgress`]). The callback
    /// runs inline between awaits, must be quick, and cannot cancel the transfer.
    pub async fn transfer_with_progress(
        &self,
        from: &TransferEnd,
        to: &TransferEnd,
        needs: &Needs,
        options: &TransferOptions,
        progress: &mut dyn FnMut(TransferProgress),
    ) -> Result<TransferReport, TransferError> {
        if from.id == to.id {
            return Err(TransferError::SameBackend(from.id.clone()));
        }
        let target_gate = self.provider_needs(needs).map_err(TransferError::Needs)?;
        let source_gate = ProviderNeeds::new(Vec::new(), Durability::Volatile);
        let mut hide = secrets_of(&from.endpoint);
        hide.extend(secrets_of(&to.endpoint));
        let open_err = |side, e: registry::OpenError| TransferError::Open {
            side,
            reason: scrub(e.to_string(), &hide),
        };
        let src = registry::open_named(&from.id, &from.endpoint, &source_gate)
            .await
            .map_err(|e| open_err("source", e))?;
        let dst = registry::open_named(&to.id, &to.endpoint, &target_gate)
            .await
            .map_err(|e| open_err("target", e))?;
        copy_and_verify(src.kv(), dst.kv(), options, &hide, progress).await
    }

    /// As [`Self::transfer`], on a current-thread runtime of its own (feature `persistant-blocking`).
    #[cfg(feature = "persistant-blocking")]
    pub fn transfer_blocking(
        &self,
        from: &TransferEnd,
        to: &TransferEnd,
        needs: &Needs,
        options: &TransferOptions,
    ) -> Result<TransferReport, TransferError> {
        self.transfer_blocking_with_progress(from, to, needs, options, &mut |_| {})
    }

    /// As [`Self::transfer_blocking`], calling `progress` on the thread that is blocked (feature
    /// `persistant-blocking`).
    #[cfg(feature = "persistant-blocking")]
    pub fn transfer_blocking_with_progress(
        &self,
        from: &TransferEnd,
        to: &TransferEnd,
        needs: &Needs,
        options: &TransferOptions,
        progress: &mut dyn FnMut(TransferProgress),
    ) -> Result<TransferReport, TransferError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| TransferError::Runtime(e.to_string()))?;
        rt.block_on(self.transfer_with_progress(from, to, needs, options, progress))
    }
}

/// The copy itself, between two key-value backends you already hold (the registry route above opens
/// them for you). Same rules: the source is only read, a non-empty target is refused unless
/// `allow_existing`, and the result is read back before `Ok`.
pub async fn copy_between<S: KvBackend, T: KvBackend>(
    source: &S,
    target: &T,
    options: &TransferOptions,
) -> Result<TransferReport, TransferError> {
    copy_between_with_progress(source, target, options, &mut |_| {}).await
}

/// As [`copy_between`], calling `progress` as the work goes (see [`TransferProgress`]).
pub async fn copy_between_with_progress<S: KvBackend, T: KvBackend>(
    source: &S,
    target: &T,
    options: &TransferOptions,
    progress: &mut dyn FnMut(TransferProgress),
) -> Result<TransferReport, TransferError> {
    copy_and_verify(source, target, options, &[], progress).await
}

/// The copy and the read-back are `persistant_kv::copy`'s; this maps its answer onto ours and scrubs the
/// backend's own error text of the endpoint secrets, which the library leaves to the caller.
async fn copy_and_verify<S: KvBackend, T: KvBackend>(
    source: &S,
    target: &T,
    options: &TransferOptions,
    hide: &[String],
    progress: &mut dyn FnMut(TransferProgress),
) -> Result<TransferReport, TransferError> {
    let opts = CopyOptions {
        allow_existing: options.allow_existing,
        page_size: options.page_size.max(1),
        count_first: options.count_first,
    };
    let mut relay = |p: copy::Progress| {
        progress(TransferProgress {
            phase: match p.phase {
                Phase::Copy => TransferPhase::Copy,
                _ => TransferPhase::Verify,
            },
            done: p.done,
            total: p.total,
        });
    };
    match copy::copy_all_with_progress(source, target, &opts, &mut relay).await {
        Ok(r) => Ok(TransferReport {
            copied: to_usize(r.copied),
            with_ttl: to_usize(r.with_ttl),
            skipped: to_usize(r.skipped),
            verified: to_usize(r.verified),
        }),
        Err(CopyError::TargetNotEmpty) => Err(TransferError::TargetNotEmpty),
        Err(CopyError::Kv { side, op, error }) => Err(kv_err(
            match side {
                Side::Source => "source",
                _ => "target",
            },
            op,
            error,
            hide,
        )),
        Err(CopyError::Mismatch {
            missing,
            differing,
            extra,
            sample,
        }) => Err(TransferError::Mismatch {
            missing: to_usize(missing),
            differing: to_usize(differing),
            extra: to_usize(extra),
            sample,
        }),
        Err(_) => Err(TransferError::Backend {
            side: "target",
            op: "copy",
            reason: "the copy failed".to_owned(),
        }),
    }
}

fn to_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_and_the_password_inside_it_are_scrubbed_from_a_backend_message() {
        let url = "postgres://appuser:hunter2@db.internal/prod";
        let hide = secrets_of(&Endpoint::Url(url.to_owned()));
        let said = format!("connect to {url} failed: password authentication failed for hunter2");
        let clean = scrub(said, &hide);
        assert!(
            !clean.contains("hunter2") && !clean.contains("db.internal"),
            "{clean}"
        );
        assert!(clean.contains("<hidden>"), "{clean}");
    }

    #[test]
    fn a_typed_url_is_refused_without_being_echoed_and_env_name_is_accepted() {
        let spec = EndpointSpec::Url {
            schemes: vec!["postgres".to_owned()],
        };
        let typed = "postgres://appuser:hunter2@db.internal/prod";
        let err = parse_endpoint(typed, &spec).unwrap_err();
        assert!(
            !err.contains("hunter2") && !err.contains("db.internal"),
            "{err}"
        );
        assert_eq!(
            parse_endpoint("env:APP_DB_URL", &spec),
            Ok(Endpoint::url_env("APP_DB_URL"))
        );
        assert!(parse_endpoint("env:", &spec).is_err());
        assert!(parse_endpoint("env:a b", &spec).is_err());
        assert!(parse_endpoint("just words", &spec).is_err());
    }

    #[test]
    fn path_and_none_specs_take_a_path_and_nothing() {
        assert_eq!(
            parse_endpoint(" /data/x ", &EndpointSpec::Path),
            Ok(Endpoint::path("/data/x"))
        );
        assert!(parse_endpoint("", &EndpointSpec::Path).is_err());
        assert!(parse_endpoint("sqlite://x", &EndpointSpec::Path).is_err());
        assert_eq!(parse_endpoint("", &EndpointSpec::None), Ok(Endpoint::None));
        assert_eq!(
            parse_endpoint("none", &EndpointSpec::None),
            Ok(Endpoint::None)
        );
        assert!(parse_endpoint("/x", &EndpointSpec::None).is_err());
    }

    #[test]
    fn a_path_endpoint_hides_nothing() {
        assert!(secrets_of(&Endpoint::path("/data/x")).is_empty());
    }
}

/// A transfer ready to run off the UI thread (feature `persistant-blocking`): the module, the two ends, the
/// needs and the options, owned. A front end returns one from its event handling, runs [`Self::run`] on a
/// worker thread, and hands the result back.
#[cfg(feature = "persistant-blocking")]
#[derive(Debug)]
pub struct TransferJob {
    module: PersistantModule,
    from: TransferEnd,
    to: TransferEnd,
    needs: Needs,
    options: TransferOptions,
}

#[cfg(feature = "persistant-blocking")]
impl TransferJob {
    pub fn new(
        module: PersistantModule,
        from: TransferEnd,
        to: TransferEnd,
        needs: Needs,
        options: TransferOptions,
    ) -> Self {
        TransferJob {
            module,
            from,
            to,
            needs,
            options,
        }
    }

    pub fn from_id(&self) -> &str {
        &self.from.id
    }

    pub fn to_id(&self) -> &str {
        &self.to.id
    }

    /// Run it, blocking the calling thread until it finishes.
    pub fn run(self) -> Result<TransferReport, TransferError> {
        self.run_with_progress(&mut |_| {})
    }

    /// As [`Self::run`], calling `progress` on the worker thread as the work goes.
    pub fn run_with_progress(
        self,
        progress: &mut dyn FnMut(TransferProgress),
    ) -> Result<TransferReport, TransferError> {
        self.module.transfer_blocking_with_progress(
            &self.from,
            &self.to,
            &self.needs,
            &self.options,
            progress,
        )
    }
}
