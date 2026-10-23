// SPDX-License-Identifier: MIT OR Apache-2.0
//! Scan assist for ConfigurAnt: find the environment variables a Rust project reads and propose schema
//! entries. Candidates come from the source text; a [`Resolver`] (a language server, see [`lsp`]) confirms
//! each resolves to `std::env`. The result is data: nothing is written, and a human accepts a proposal by
//! calling `configurant::schema_edit::add_value` with [`Proposal::to_new_value`].
//!
//! A scan that could not confirm every call says so ([`Proposals::is_partial`]); it never returns a short
//! list as if it were complete. A secret-looking name is proposed as `secret` with no default, and the
//! default seen beside it is dropped here, so no output path can carry it.

pub mod lsp;
pub mod rust;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use configurant::schema_edit::NewValue;
use configurant::{Schema, ValueType};

pub use rust::{Api, Arg, Candidate};

/// Why a scan could not run. No variant carries file contents.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ScanError {
    #[error("{}: {reason}", path.display())]
    Io { path: PathBuf, reason: String },
    #[error("language server `{command}`: {reason}")]
    Server { command: String, reason: String },
}

/// Confirms what a call resolves to. `Ok(None)` means the server had no answer (yet).
pub trait Resolver {
    /// The file the definition at `file:line:col` (0-based, UTF-16 column) lives in, `/`-separated.
    fn definition(
        &mut self,
        file: &Path,
        text: &str,
        line: u32,
        col: u32,
    ) -> Result<Option<String>, ScanError>;
}

/// A place in the scanned tree (path relative to the root, 1-based line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub file: PathBuf,
    pub line: u32,
}

/// An environment variable the code reads that the schema does not declare.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Proposal {
    pub env: String,
    pub key: String,
    pub sites: Vec<Site>,
    /// A literal default seen beside the read; always `None` for a secret.
    pub default_seen: Option<String>,
    pub secret: bool,
}

impl Proposal {
    /// The type and default a value of this name would get, from the literal seen beside the read.
    fn typed(&self) -> (ValueType, Option<toml::Value>) {
        match self.default_seen.as_deref() {
            Some("true") => (ValueType::Bool, Some(toml::Value::Boolean(true))),
            Some("false") => (ValueType::Bool, Some(toml::Value::Boolean(false))),
            Some(s) => match s.parse::<i64>() {
                Ok(i) => (ValueType::Int, Some(toml::Value::Integer(i))),
                Err(_) => (ValueType::String, Some(toml::Value::String(s.to_owned()))),
            },
            None => (ValueType::String, None),
        }
    }

    /// The schema type name of the proposal, for the report.
    pub fn type_name(&self) -> &'static str {
        self.typed().0.name()
    }

    /// What the report appends to a proposal that must not hold a default.
    pub fn mark(&self) -> &'static str {
        if self.secret { " (no default)" } else { "" }
    }

    /// The value to hand to `schema_edit::add_value` once a human has accepted it.
    pub fn to_new_value(&self) -> NewValue {
        let first = &self.sites[0];
        let desc = format!(
            "read from the environment as {} ({}:{})",
            self.env,
            first.file.display().to_string().replace('\', "/"),
            first.line
        );
        let (ty, default) = self.typed();
        let mut n = NewValue::new(&self.key, ty, &desc);
        n.env = Some(self.env.clone());
        n.default = default;
        n.secret = self.secret;
        n
    }
}

/// The outcome of a scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Proposals {
    pub proposals: Vec<Proposal>,
    /// Reads whose name is not a plain string literal.
    pub dynamic: Vec<Site>,
    /// Calls the server says are not `std::env` (a function of the project's own).
    pub rejected: usize,
    /// Calls the server could not resolve: the scan is partial while this is not zero.
    pub unresolved: usize,
    /// Env names the schema declares already.
    pub known: usize,
    /// Env names whose guessed key the schema uses for a different env: decide these by hand.
    pub conflicts: Vec<String>,
    pub files: usize,
    pub candidates: usize,
}

impl Proposals {
    pub fn is_partial(&self) -> bool {
        self.unresolved > 0
    }
}

const SECRET_WORDS: [&str; 9] = [
    "password",
    "passwd",
    "secret",
    "token",
    "credential",
    "apikey",
    "api_key",
    "private_key",
    "dsn",
];

/// Does an env name look like it holds a secret?
pub fn looks_secret(env: &str) -> bool {
    let l = env.to_ascii_lowercase();
    SECRET_WORDS.iter().any(|w| l.contains(w)) || l.ends_with("_key") || l.ends_with("_auth")
}

fn guess_key(env: &str, prefix: Option<&str>) -> String {
    let name = prefix
        .and_then(|p| env.strip_prefix(p))
        .filter(|n| !n.is_empty())
        .unwrap_or(env)
        .to_ascii_lowercase();
    match name.split_once('_') {
        Some((a, b)) if !a.is_empty() && !b.is_empty() => format!("{a}.{b}"),
        _ => format!("app.{name}"),
    }
}

fn is_std_env(path: &str) -> bool {
    path.ends_with("/library/std/src/env.rs")
}

/// Scan `root` and propose what `schema` lacks.
pub fn scan(
    root: &Path,
    schema: &Schema,
    resolver: &mut dyn Resolver,
) -> Result<Proposals, ScanError> {
    let mut out = Proposals::default();
    let mut by_env: BTreeMap<String, Proposal> = BTreeMap::new();
    let prefix = schema.project.env_prefix.as_deref();
    for file in rust::source_files(root)? {
        let text = std::fs::read_to_string(&file).map_err(|e| ScanError::Io {
            path: file.clone(),
            reason: e.to_string(),
        })?;
        out.files += 1;
        let rel = file.strip_prefix(root).unwrap_or(&file).to_path_buf();
        for c in rust::candidates(&file, &text) {
            out.candidates += 1;
            let site = Site {
                file: rel.clone(),
                line: c.line + 1,
            };
            if c.api.is_function() {
                match resolver.definition(&file, &text, c.line, c.col)? {
                    Some(p) if is_std_env(&p) => {}
                    Some(_) => {
                        out.rejected += 1;
                        continue;
                    }
                    None => {
                        out.unresolved += 1;
                        continue;
                    }
                }
            }
            let Arg::Literal(env) = c.arg else {
                out.dynamic.push(site);
                continue;
            };
            let secret = looks_secret(&env);
            let p = by_env.entry(env.clone()).or_insert_with(|| Proposal {
                key: guess_key(&env, prefix),
                env,
                sites: Vec::new(),
                default_seen: None,
                secret,
            });
            p.sites.push(site);
            if !p.secret && p.default_seen.is_none() {
                p.default_seen = c.default;
            }
        }
    }
    for (env, p) in by_env {
        if schema
            .values()
            .iter()
            .any(|v| v.env.as_deref() == Some(&env))
        {
            out.known += 1;
        } else if schema.get(&p.key).is_some() {
            out.conflicts.push(env);
        } else {
            out.proposals.push(p);
        }
    }
    Ok(out)
}

/// Why accepting proposals failed. No variant carries a default.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AcceptError {
    #[error("no proposal for `{env}`; proposed: {}", known.join(", "))]
    Unknown { env: String, known: Vec<String> },
    #[error(transparent)]
    Edit(#[from] configurant::schema_edit::SchemaEditError),
}

impl Proposals {
    /// The schema text with the proposals for `envs` added; the caller writes it.
    pub fn accept(&self, schema_text: &str, envs: &[String]) -> Result<String, AcceptError> {
        let mut text = schema_text.to_owned();
        for env in envs {
            let p = self
                .proposals
                .iter()
                .find(|p| &p.env == env)
                .ok_or_else(|| AcceptError::Unknown {
                    env: env.clone(),
                    known: self.proposals.iter().map(|p| p.env.clone()).collect(),
                })?;
            text = configurant::schema_edit::add_value(&text, &p.to_new_value())?;
        }
        Ok(text)
    }

    /// A plain-text report. A default is shown only for a proposal that is not secret (a secret holds none).
    pub fn report(&self) -> String {
        use std::fmt::Write as _;
        let mut o = String::new();
        let at = |s: &Site| {
            format!(
                "{}:{}",
                s.file.display().to_string().replace('\\', "/"),
                s.line
            )
        };
        let _ = writeln!(
            o,
            "scanned {} files, {} candidate reads: {} proposed, {} already in the schema",
            self.files,
            self.candidates,
            self.proposals.len(),
            self.known
        );
        for p in &self.proposals {
            let _ = write!(o, "  {} -> {} ({}){}", p.env, p.key, p.type_name(), p.mark());
            if let Some(d) = &p.default_seen {
                let _ = write!(o, " default {d}");
            }
            let sites: Vec<_> = p.sites.iter().map(at).collect();
            let _ = writeln!(o, " at {}", sites.join(", "));
        }
        for env in &self.conflicts {
            let _ = writeln!(
                o,
                "  {env}: its guessed key is already used for another variable; decide by hand"
            );
        }
        if !self.dynamic.is_empty() {
            let sites: Vec<_> = self.dynamic.iter().map(at).collect();
            let _ = writeln!(
                o,
                "reads with a computed name (not proposed): {}",
                sites.join(", ")
            );
        }
        if self.rejected > 0 {
            let _ = writeln!(
                o,
                "{} calls resolve to the project's own `env::var`, not std (skipped)",
                self.rejected
            );
        }
        if self.is_partial() {
            let _ = writeln!(
                o,
                "PARTIAL: the language server could not resolve {} calls, so this list may be missing variables",
                self.unresolved
            );
        }
        o
    }
}
