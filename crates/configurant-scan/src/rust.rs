// SPDX-License-Identifier: MIT OR Apache-2.0
//! Finding candidate environment reads in Rust source text. Text only: whether a candidate really is the
//! standard library's call is for the language server to confirm.

use std::path::{Path, PathBuf};

use crate::ScanError;

/// The reader API a candidate looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Api {
    EnvVar,
    EnvVarOs,
    EnvMacro,
    OptionEnvMacro,
}

impl Api {
    /// A function call the language server can check; a macro is accepted by name.
    pub fn is_function(self) -> bool {
        matches!(self, Api::EnvVar | Api::EnvVarOs)
    }
}

/// What the call was given as its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    Literal(String),
    /// Anything but a plain string literal: reported by location, never guessed.
    Dynamic,
}

/// One place that looks like an environment read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub file: PathBuf,
    /// 0-based line and UTF-16 column of the function's identifier (the LSP position).
    pub line: u32,
    pub col: u32,
    pub api: Api,
    pub arg: Arg,
    /// A literal given to `unwrap_or` straight after the call.
    pub default: Option<String>,
}

const NEEDLES: [(&str, Api, usize); 4] = [
    ("env::var_os(", Api::EnvVarOs, 5),
    ("env::var(", Api::EnvVar, 5),
    ("option_env!(", Api::OptionEnvMacro, 0),
    ("env!(", Api::EnvMacro, 0),
];

/// Every `.rs` file under `root`, sorted, skipping dot-directories, `target` and `node_modules`.
pub fn source_files(root: &Path) -> Result<Vec<PathBuf>, ScanError> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir).map_err(|e| ScanError::Io {
            path: dir.clone(),
            reason: e.to_string(),
        })?;
        for entry in rd {
            let entry = entry.map_err(|e| ScanError::Io {
                path: dir.clone(),
                reason: e.to_string(),
            })?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    stack.push(path);
                }
            } else if name.ends_with(".rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Candidates in one file's text.
pub fn candidates(file: &Path, text: &str) -> Vec<Candidate> {
    let mut starts = vec![0usize];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    let mut found: Vec<(usize, Candidate)> = Vec::new();
    for (needle, api, ident_off) in NEEDLES {
        for (at, _) in text.match_indices(needle) {
            if glued(text, at) {
                continue;
            }
            let line_idx = starts.partition_point(|&s| s <= at) - 1;
            if text[starts[line_idx]..at].contains("//") {
                continue;
            }
            let ident = at + ident_off;
            let col = text[starts[line_idx]..ident].encode_utf16().count() as u32;
            let after = at + needle.len();
            let (arg, rest) = parse_arg(text, after);
            let default = if api.is_function() && matches!(arg, Arg::Literal(_)) {
                default_after(text, rest)
            } else {
                None
            };
            found.push((
                at,
                Candidate {
                    file: file.to_path_buf(),
                    line: line_idx as u32,
                    col,
                    api,
                    arg,
                    default,
                },
            ));
        }
    }
    found.sort_by_key(|(at, _)| *at);
    found.dedup_by_key(|(at, _)| *at);
    found.into_iter().map(|(_, c)| c).collect()
}

/// A needle glued to the end of an identifier (`option_env!(` seen as `env!(`, `myenv::var(`) is not it.
fn glued(text: &str, at: usize) -> bool {
    matches!(text[..at].chars().next_back(), Some(c) if c.is_alphanumeric() || c == '_')
}

fn skip_ws(text: &str, mut at: usize) -> usize {
    while let Some(c) = text[at..].chars().next() {
        if c.is_whitespace() {
            at += c.len_utf8();
        } else {
            break;
        }
    }
    at
}

/// The first argument at `at` (just past the `(`), and the offset just past the closing `)`.
fn parse_arg(text: &str, at: usize) -> (Arg, usize) {
    let at = skip_ws(text, at);
    if let Some((s, end)) = string_literal(text, at) {
        let end = skip_ws(text, end);
        match text[end..].chars().next() {
            Some(')') => return (Arg::Literal(s), end + 1),
            Some(',') => return (Arg::Literal(s), end + 1),
            _ => {}
        }
    }
    (Arg::Dynamic, at)
}

/// A plain `"..."` literal with no escapes.
fn string_literal(text: &str, at: usize) -> Option<(String, usize)> {
    let body = text[at..].strip_prefix('"')?;
    let close = body.find('"')?;
    let s = &body[..close];
    if s.contains('\\') || s.contains('\n') {
        return None;
    }
    Some((s.to_owned(), at + 1 + close + 1))
}

/// `.unwrap_or(<literal>` straight after the call, with an optional `.ok()` between.
fn default_after(text: &str, at: usize) -> Option<String> {
    let mut at = skip_ws(text, at);
    if text[at..].starts_with(".ok()") {
        at = skip_ws(text, at + 5);
    }
    let rest = text[at..].strip_prefix(".unwrap_or(")?;
    let at = skip_ws(text, text.len() - rest.len());
    if let Some((s, _)) = string_literal(text, at) {
        return Some(s);
    }
    let token: String = text[at..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .collect();
    let plain = token.replace('_', "");
    if token == "true" || token == "false" || plain.parse::<i64>().is_ok() {
        return Some(plain);
    }
    None
}
