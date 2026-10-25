// SPDX-License-Identifier: MIT OR Apache-2.0
//! Secret sources for ConfigurAnt: fetch a `secret` value by reference at load time and hand it to the
//! loader as redacted text. See `docs/SECRET-SOURCES.md`.
//!
//! A source cannot put text in an error (it returns a [`SourceFailure`] kind), and a [`SecretValue`] shows
//! nothing through `Debug`; the environment is consulted first and a source is not asked for a key the
//! environment already sets.

mod file;

pub use file::FileSource;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use configurant::{Loader, Schema};
use zeroize::Zeroizing;

/// The interface version a source is written against.
pub const SOURCE_INTERFACE_VERSION: u32 = 1;

/// A fetched secret. No `Clone`, no `Display`, and `Debug` shows nothing; the text is wiped on drop.
pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub fn new(text: impl Into<String>) -> Self {
        SecretValue(Zeroizing::new(text.into()))
    }

    /// The secret text. The name is the warning: do not print, log or store it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretValue(<redacted>)")
    }
}

/// Why a source could not supply a secret. A kind only: nothing a source received can reach an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceFailure {
    NotFound,
    Denied,
    Unreachable,
    /// The source found something, but it is not usable (too large, not text).
    Invalid,
    /// Added by [`resolve`] when the source did not answer in time.
    TimedOut,
}

impl fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SourceFailure::NotFound => "not found",
            SourceFailure::Denied => "access denied",
            SourceFailure::Unreachable => "unreachable",
            SourceFailure::Invalid => "invalid content",
            SourceFailure::TimedOut => "timed out",
        })
    }
}

/// Somewhere a secret can be fetched from, by reference.
pub trait SecretSource: Send + Sync {
    /// The name a schema's `source = "<name>:<reference>"` refers to.
    fn name(&self) -> &str;

    fn interface_version(&self) -> u32 {
        SOURCE_INTERFACE_VERSION
    }

    /// Whether `reference` is well formed for this source. Asked before any fetch, for every sourced secret in
    /// a schema; a source that can tell a bad reference without looking it up should say so here.
    fn check_reference(&self, _reference: &str) -> bool {
        true
    }

    /// Fetch the secret named by `reference`.
    fn fetch(&self, reference: &str) -> Result<SecretValue, SourceFailure>;
}

/// Why a source could not be registered.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SourcesError {
    #[error("source `{source_name}` speaks interface version {got}; this crate speaks {want}")]
    InterfaceMismatch {
        source_name: String,
        got: u32,
        want: u32,
    },
    #[error("a source named `{0}` is already registered")]
    Duplicate(String),
}

/// The registered sources. Nothing is discovered.
#[derive(Default)]
pub struct Sources {
    by_name: BTreeMap<String, Arc<dyn SecretSource>>,
}

impl Sources {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, source: Box<dyn SecretSource>) -> Result<(), SourcesError> {
        let name = source.name().to_owned();
        if source.interface_version() != SOURCE_INTERFACE_VERSION {
            return Err(SourcesError::InterfaceMismatch {
                source_name: name,
                got: source.interface_version(),
                want: SOURCE_INTERFACE_VERSION,
            });
        }
        if self.by_name.contains_key(&name) {
            return Err(SourcesError::Duplicate(name));
        }
        self.by_name.insert(name, Arc::from(source));
        Ok(())
    }

    fn names(&self) -> String {
        if self.by_name.is_empty() {
            "none".to_owned()
        } else {
            self.by_name
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

/// Why a secret could not be resolved. Key, source and reference names only; never a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ResolveError {
    #[error(
        "`{key}` names a source that is not registered: `{source_name}` (registered: {registered})"
    )]
    UnknownSource {
        key: String,
        source_name: String,
        registered: String,
    },
    #[error("`source` of `{key}` must look like `<source>:<reference>`")]
    BadSource { key: String },
    #[error("`{key}` has a reference `{reference}` that source `{source_name}` does not accept")]
    BadReference {
        key: String,
        source_name: String,
        reference: String,
    },
    #[error(
        "secret source `{source_name}` could not supply `{key}` (reference `{reference}`): {kind}"
    )]
    Failed {
        key: String,
        source_name: String,
        reference: String,
        kind: SourceFailure,
    },
}

/// Every reason a resolve failed, not just the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveErrors(pub Vec<ResolveError>);

impl fmt::Display for ResolveErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ResolveErrors {}

/// The secrets fetched for a load, by key. `Debug` lists keys only.
#[derive(Default)]
pub struct Resolved(BTreeMap<String, SecretValue>);

impl Resolved {
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Hand the secrets to the loader (consuming them: each `SecretValue` is wiped as it is moved in).
    pub fn into_loader<'a>(self, loader: Loader<'a>) -> Loader<'a> {
        let texts = self
            .0
            .iter()
            .map(|(k, v)| (k.clone(), v.expose().to_owned()))
            .collect();
        loader.resolved(texts)
    }
}

impl fmt::Debug for Resolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resolved")
            .field("keys", &self.0.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Fetch every secret that declares a `source` and is not already set by the environment `env`. Each fetch
/// waits at most `timeout`; a source that has not answered by then is [`SourceFailure::TimedOut`] (its worker
/// thread is left to finish on its own). Returns every failure, not the first.
pub fn resolve(
    schema: &Schema,
    sources: &Sources,
    env: &BTreeMap<String, String>,
    timeout: Duration,
) -> Result<Resolved, ResolveErrors> {
    let mut out = BTreeMap::new();
    let mut errors = Vec::new();
    for def in schema.values().iter().filter(|d| d.secret) {
        let Some(declared) = def.source.as_deref() else {
            continue;
        };
        // The environment is the explicit override: do not even ask the source.
        let env_set = def.env.as_ref().is_some_and(|name| env.contains_key(name));
        let Some((name, reference)) = declared
            .split_once(':')
            .filter(|(n, r)| !n.is_empty() && !r.is_empty())
        else {
            if !env_set {
                errors.push(ResolveError::BadSource {
                    key: def.key.clone(),
                });
            }
            continue;
        };
        let Some(source) = sources.by_name.get(name) else {
            if !env_set {
                errors.push(ResolveError::UnknownSource {
                    key: def.key.clone(),
                    source_name: name.to_owned(),
                    registered: sources.names(),
                });
            }
            continue;
        };
        // A malformed reference is a schema fault, whatever the environment holds.
        if !source.check_reference(reference) {
            errors.push(ResolveError::BadReference {
                key: def.key.clone(),
                source_name: name.to_owned(),
                reference: reference.to_owned(),
            });
            continue;
        }
        if env_set {
            continue;
        }
        match fetch_within(source, reference, timeout) {
            Ok(v) => {
                out.insert(def.key.clone(), v);
            }
            Err(kind) => errors.push(ResolveError::Failed {
                key: def.key.clone(),
                source_name: name.to_owned(),
                reference: reference.to_owned(),
                kind,
            }),
        }
    }
    if errors.is_empty() {
        Ok(Resolved(out))
    } else {
        Err(ResolveErrors(errors))
    }
}

fn fetch_within(
    source: &Arc<dyn SecretSource>,
    reference: &str,
    timeout: Duration,
) -> Result<SecretValue, SourceFailure> {
    let (tx, rx) = mpsc::channel();
    let (source, reference) = (Arc::clone(source), reference.to_owned());
    std::thread::spawn(move || {
        let _ = tx.send(source.fetch(&reference));
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(SourceFailure::TimedOut),
        // The worker ended without answering (it panicked).
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(SourceFailure::Unreachable),
    }
}
