// SPDX-License-Identifier: MIT OR Apache-2.0
//! "Add from scan" (feature `scan`): run `configurant-scan` over a project, tick the proposals to add, review
//! them, and hand the new schema text to the caller to write. The scan runs on the caller's worker thread
//! (it starts a language server and can take a minute); nothing here writes a file.

use std::path::PathBuf;
use std::time::Duration;

use configurant::Schema;
use configurant_scan::lsp::LspResolver;
use configurant_scan::{Proposals, scan};

/// What the scan needs from the caller: the schema (its text and path), the project to scan, and the server.
pub struct ScanSetup {
    pub schema_path: PathBuf,
    pub schema_text: String,
    pub root: PathBuf,
    /// The language server command, e.g. `rust-analyzer`.
    pub server: String,
    pub wait: Duration,
}

/// A scan to run on a worker thread; hand its result to [`App::scanned`](crate::App::scanned).
pub struct ScanJob {
    schema_text: String,
    root: PathBuf,
    server: String,
    wait: Duration,
}

impl std::fmt::Debug for ScanJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanJob")
            .field("root", &self.root)
            .field("server", &self.server)
            .finish_non_exhaustive()
    }
}

impl ScanJob {
    pub(crate) fn new(s: &ScanSetup) -> Self {
        ScanJob {
            schema_text: s.schema_text.clone(),
            root: s.root.clone(),
            server: s.server.clone(),
            wait: s.wait,
        }
    }

    /// Start the server, scan, and stop it. The error is text: no variant of it carries file contents.
    pub fn run(self) -> Result<Proposals, String> {
        let schema = Schema::parse(&self.schema_text).map_err(|e| e.to_string())?;
        let mut server = LspResolver::start(&self.server, &[], &self.root, self.wait)
            .map_err(|e| e.to_string())?;
        scan(&self.root, &schema, &mut server).map_err(|e| e.to_string())
    }
}

/// The proposals on screen, with the ones the user has ticked.
pub(crate) struct Pick {
    pub found: Proposals,
    pub checked: Vec<bool>,
}

impl Pick {
    pub fn new(found: Proposals) -> Self {
        let checked = vec![false; found.proposals.len()];
        Pick { found, checked }
    }

    pub fn envs(&self) -> Vec<String> {
        self.found
            .proposals
            .iter()
            .zip(&self.checked)
            .filter(|(_, c)| **c)
            .map(|(p, _)| p.env.clone())
            .collect()
    }

    /// What the scan could not settle. A partial scan is said first and plainly.
    pub fn header(&self) -> Vec<String> {
        let f = &self.found;
        let mut h = vec![format!(
            "Scan: {} files, {} reads; {} proposed, {} already in the schema.",
            f.files,
            f.candidates,
            f.proposals.len(),
            f.known
        )];
        if f.is_partial() {
            h.push(format!(
                "PARTIAL: the language server could not resolve {} calls, so this list may be missing variables.",
                f.unresolved
            ));
        }
        if !f.conflicts.is_empty() {
            h.push(format!(
                "Decide by hand (their guessed key is taken): {}",
                f.conflicts.join(", ")
            ));
        }
        if !f.dynamic.is_empty() {
            h.push(format!(
                "{} read(s) with a computed name are not proposed.",
                f.dynamic.len()
            ));
        }
        h
    }

    /// One line per proposal, for its checkbox. A secret holds no default, so none can be drawn.
    pub fn lines(&self) -> Vec<String> {
        self.found
            .proposals
            .iter()
            .map(|p| {
                let mut l = format!("{} -> {} ({}){}", p.env, p.key, p.type_name(), p.mark());
                if let Some(d) = &p.default_seen {
                    l += &format!(" default {d}");
                }
                if let Some(s) = p.sites.first() {
                    l += &format!(
                        " at {}:{}",
                        s.file.display().to_string().replace('\\', "/"),
                        s.line
                    );
                }
                l
            })
            .collect()
    }
}

/// The ticked proposals, accepted into new schema text, waiting for `y`.
pub(crate) struct SchemaReview {
    pub pick: Pick,
    pub new_text: String,
    pub envs: Vec<String>,
}
