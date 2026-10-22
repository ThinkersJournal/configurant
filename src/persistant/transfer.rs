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

use std::collections::HashSet;
use std::fmt;

use ::persistant::capability::provider::ProviderNeeds;
use ::persistant::kv::{KvBackend, KvError};
use ::persistant::registry::{self, Endpoint};

use super::{Durability, PersistantError, PersistantModule};
use crate::module::Needs;

/// What the report says is left alone.
pub const NOT_COPIED: &str = "leases, blob and relational data are not copied";

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
}

impl Default for TransferOptions {
    fn default() -> Self {
        TransferOptions {
            allow_existing: false,
            page_size: 256,
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
        copy_and_verify(src.kv(), dst.kv(), options, &hide).await
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
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| TransferError::Runtime(e.to_string()))?;
        rt.block_on(self.transfer(from, to, needs, options))
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
    copy_and_verify(source, target, options, &[]).await
}

/// One page of the source, as `(key, value)` pairs and the cursor for the next. Backends that can scan
/// keys and values together in one snapshot (`ordered_scan_values`) do; the blob-based ones (`memory`, `fs`)
/// scan keys only, so each value is then read with `get` (a key that expired in between is skipped).
async fn read_page<S: KvBackend>(
    source: &S,
    after: Option<&str>,
    limit: usize,
    hide: &[String],
) -> Result<(Vec<(String, Vec<u8>)>, Option<String>), TransferError> {
    match source.ordered_scan_values("", after, limit).await {
        Ok(page) => return Ok((page.entries, page.next)),
        Err(KvError::Unsupported(_)) => {}
        Err(e) => return Err(kv_err("source", "ordered_scan_values", e, hide)),
    }
    let keys = source
        .ordered_scan("", after, limit)
        .await
        .map_err(|e| kv_err("source", "ordered_scan", e, hide))?;
    let mut entries = Vec::with_capacity(keys.keys.len());
    for key in keys.keys {
        let got = source
            .get(&key)
            .await
            .map_err(|e| kv_err("source", "get", e, hide))?;
        if let Some(value) = got {
            entries.push((key, value));
        }
    }
    Ok((entries, keys.next))
}

async fn target_has_any<T: KvBackend>(target: &T, hide: &[String]) -> Result<bool, TransferError> {
    match target.ordered_scan("", None, 1).await {
        Ok(page) => Ok(!page.keys.is_empty()),
        Err(KvError::Unsupported(_)) => target
            .prefix_list("", None, 1)
            .await
            .map(|page| !page.keys.is_empty())
            .map_err(|e| kv_err("target", "prefix_list", e, hide)),
        Err(e) => Err(kv_err("target", "ordered_scan", e, hide)),
    }
}

async fn target_count<T: KvBackend>(
    target: &T,
    limit: usize,
    hide: &[String],
) -> Result<usize, TransferError> {
    let (mut n, mut after) = (0, None::<String>);
    loop {
        let page = target
            .ordered_scan("", after.as_deref(), limit)
            .await
            .map_err(|e| kv_err("target", "ordered_scan", e, hide))?;
        n += page.keys.len();
        match page.next {
            Some(cursor) => after = Some(cursor),
            None => return Ok(n),
        }
    }
}

async fn copy_and_verify<S: KvBackend, T: KvBackend>(
    source: &S,
    target: &T,
    options: &TransferOptions,
    hide: &[String],
) -> Result<TransferReport, TransferError> {
    let limit = options.page_size.max(1);
    if !options.allow_existing && target_has_any(target, hide).await? {
        return Err(TransferError::TargetNotEmpty);
    }

    let (mut copied, mut with_ttl, mut skipped) = (0, 0, 0);
    let mut kept: HashSet<String> = HashSet::new();
    let mut after: Option<String> = None;
    loop {
        let (entries, next) = read_page(source, after.as_deref(), limit, hide).await?;
        for (key, value) in entries {
            let ttl = source
                .ttl_remaining(&key)
                .await
                .map_err(|e| kv_err("source", "ttl_remaining", e, hide))?;
            if options.allow_existing {
                let stored = target
                    .put_if_absent(&key, value, ttl)
                    .await
                    .map_err(|e| kv_err("target", "put_if_absent", e, hide))?;
                if !stored {
                    skipped += 1;
                    kept.insert(key);
                    continue;
                }
            } else {
                target
                    .put(&key, value, ttl)
                    .await
                    .map_err(|e| kv_err("target", "put", e, hide))?;
            }
            copied += 1;
            with_ttl += usize::from(ttl.is_some());
        }
        match next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }

    // Read both sides back. The source is scanned again (not remembered), so what is compared is what
    // is stored now.
    let (mut verified, mut missing, mut differing) = (0, 0, 0);
    let mut sample: Vec<String> = Vec::new();
    let mut note = |key: String| {
        if sample.len() < 10 {
            sample.push(key);
        }
    };
    let mut after: Option<String> = None;
    loop {
        let (entries, next) = read_page(source, after.as_deref(), limit, hide).await?;
        for (key, value) in entries {
            if kept.contains(&key) {
                continue;
            }
            match target
                .get(&key)
                .await
                .map_err(|e| kv_err("target", "get", e, hide))?
            {
                None => {
                    missing += 1;
                    note(key);
                }
                Some(stored) if stored != value => {
                    differing += 1;
                    note(key);
                }
                Some(_) => verified += 1,
            }
        }
        match next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    let extra = if options.allow_existing {
        0
    } else {
        target_count(target, limit, hide)
            .await?
            .saturating_sub(verified + differing)
    };
    if missing + differing + extra > 0 {
        return Err(TransferError::Mismatch {
            missing,
            differing,
            extra,
            sample,
        });
    }
    Ok(TransferReport {
        copied,
        with_ttl,
        skipped,
        verified,
    })
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
    fn a_path_endpoint_hides_nothing() {
        assert!(secrets_of(&Endpoint::path("/data/x")).is_empty());
    }
}
