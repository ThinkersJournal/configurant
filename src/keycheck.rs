// SPDX-License-Identifier: MIT OR Apache-2.0
//! The compile-time key check: find the places where Rust code asks the library for a key the schema does
//! not define. It reads source text and writes nothing (no generated code); run it from a `build.rs`, from
//! `configurant keys`, or in CI. A name it cannot read as a plain string literal is counted, never guessed.
//!
//! What counts as asking: `.get::<T>("key")` (the turbofish tells it from `HashMap::get`), `.value("key")`,
//! `.rows("key")` and `.explain("key")`. It matches text, so another type with a method of one of those
//! names and a string argument is reported too; spell such a key as a variable to leave it alone.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::error::suggestion;
use crate::schema::Schema;

/// One call that asks for a key the schema does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UnknownKey {
    pub file: String,
    /// 1-based.
    pub line: usize,
    pub key: String,
    pub suggestion: Option<String>,
}

/// What a check found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Report {
    /// Calls with a literal key that were looked up in the schema.
    pub checked: usize,
    /// Calls whose key is computed: not checked.
    pub dynamic: usize,
    pub unknown: Vec<UnknownKey>,
}

impl Report {
    /// One line per unknown key: `file:line: unknown key `k` (did you mean `s`?)`. Key names only.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for u in &self.unknown {
            let hint = u
                .suggestion
                .as_ref()
                .map(|s| format!(" (did you mean `{s}`?)"))
                .unwrap_or_default();
            let _ = writeln!(out, "{}:{}: unknown key `{}`{hint}", u.file, u.line, u.key);
        }
        out
    }
}

/// Why a directory could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot read {path}: {reason}")]
pub struct KeyCheckError {
    pub path: String,
    pub reason: String,
}

/// Check one file's text. `file` is only a label for the report.
#[must_use]
pub fn check_source(schema: &Schema, file: &str, text: &str) -> Report {
    let mut report = Report::default();
    let mut starts = vec![0usize];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    for (at, _) in text.match_indices('.') {
        let Some(arg) = asking_at(text, at + 1) else {
            continue;
        };
        let line_idx = starts.partition_point(|&s| s <= at) - 1;
        if text[starts[line_idx]..at].contains("//") {
            continue;
        }
        let Some(key) = arg else {
            report.dynamic += 1;
            continue;
        };
        report.checked += 1;
        if !known(schema, &key) {
            let suggestion = suggestion(&key, schema.all_keys());
            report.unknown.push(UnknownKey {
                file: file.to_owned(),
                line: line_idx + 1,
                key,
                suggestion,
            });
        }
    }
    report
}

/// Check every `.rs` file under `root`, skipping hidden directories, `target` and `node_modules`.
pub fn check_dir(schema: &Schema, root: &Path) -> Result<Report, KeyCheckError> {
    let err = |e: std::io::Error, p: &Path| KeyCheckError {
        path: p.display().to_string(),
        reason: e.to_string(),
    };
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|e| err(e, &dir))? {
            let entry = entry.map_err(|e| err(e, &dir))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    stack.push(path);
                }
            } else if name.ends_with(".rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut total = Report::default();
    for path in files {
        let text = std::fs::read_to_string(&path).map_err(|e| err(e, &path))?;
        let label = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        let r = check_source(schema, &label, &text);
        total.checked += r.checked;
        total.dynamic += r.dynamic;
        total.unknown.extend(r.unknown);
    }
    Ok(total)
}

/// A key the schema defines, or an entry under one of its maps.
fn known(schema: &Schema, key: &str) -> bool {
    schema.all_keys().any(|k| k == key)
        || schema.maps().iter().any(|m| {
            key.strip_prefix(m.key.as_str())
                .is_some_and(|r| r.starts_with('.'))
        })
}

/// If an asking call starts at `at` (just past a `.`): `Some(Some(key))` for a literal, `Some(None)` for a
/// computed name, `None` if this is not one.
fn asking_at(text: &str, at: usize) -> Option<Option<String>> {
    let rest = &text[at..];
    let after = if let Some(r) = rest.strip_prefix("get::<") {
        let close = turbofish_end(r)?;
        r[close..].strip_prefix('(')?
    } else {
        ["value(", "rows(", "explain("]
            .iter()
            .find_map(|m| rest.strip_prefix(m))?
    };
    let arg = after.trim_start();
    if let Some(body) = arg.strip_prefix('"') {
        let close = body.find('"')?;
        let s = &body[..close];
        if !s.contains('\\') && !s.contains('\n') {
            let tail = body[close + 1..].trim_start();
            if tail.starts_with(')') || tail.starts_with(',') {
                return Some(Some(s.to_owned()));
            }
        }
    }
    Some(None)
}

/// The offset just past the `>` that closes a turbofish whose `<` is already consumed.
fn turbofish_end(r: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in r.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            ';' | '{' | '}' | '\n' => return None,
            _ => {}
        }
    }
    None
}
