// SPDX-License-Identifier: MIT OR Apache-2.0
//! Named errors. No variant carries a secret's value: where a value would be shown for a secret
//! key, the text is `<redacted>`.

use std::fmt;

/// Where a value (or an offending input) came from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Origin {
    Default,
    /// A configuration file, by the name it was given.
    File(String),
    /// An environment variable, by name.
    Env(String),
    /// A command-line flag (or argument position).
    Cli(String),
    /// A secret source (`configurant-secrets`), as the schema declares it: `<source>:<reference>`. Never the value.
    Source(String),
    /// A lookup by key (`Config::get`, `explain`), not a source of values.
    Lookup,
    /// A change made by a front end through [`crate::edit::Session`].
    Edit,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Origin::Default => f.write_str("the schema default"),
            Origin::File(n) => write!(f, "file `{n}`"),
            Origin::Env(n) => write!(f, "environment variable `{n}`"),
            Origin::Cli(n) => write!(f, "command-line `{n}`"),
            Origin::Source(declared) => match declared.split_once(':') {
                Some((name, reference)) => {
                    write!(f, "secret source `{name}` (reference `{reference}`)")
                }
                None => write!(f, "secret source `{declared}`"),
            },
            Origin::Lookup => f.write_str("this lookup"),
            Origin::Edit => f.write_str("this edit"),
        }
    }
}

/// One refused input.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("unknown key `{key}` from {origin}{}", suggest(.suggestion))]
    UnknownKey {
        key: String,
        suggestion: Option<String>,
        origin: Origin,
    },
    #[error("`{key}` from {origin}: expected {expected}, found {found}")]
    WrongType {
        key: String,
        expected: &'static str,
        found: String,
        origin: Origin,
    },
    #[error("`{key}` from {origin}: {found} is outside {}", range(*.min, *.max))]
    OutOfRange {
        key: String,
        found: String,
        min: Option<f64>,
        max: Option<f64>,
        origin: Origin,
    },
    #[error("`{key}` from {origin}: {found} is not one of {}", .choices.join(", "))]
    NotAllowed {
        key: String,
        found: String,
        choices: Vec<String>,
        origin: Origin,
    },
    #[error("`{key}` is a secret and may not come from {origin}; use its environment variable")]
    SecretInForbiddenSource { key: String, origin: Origin },
    #[error("`{key}` is required and no origin set it")]
    MissingRequired { key: String },
    #[error("`{key}` has no value (no default and no origin set it)")]
    NotSet { key: String },
    #[error("flag `{flag}` needs a value")]
    MissingCliValue { flag: String },
    #[error("unexpected command-line argument at position {index}")]
    UnexpectedArgument { index: usize },
    #[error("configuration file `{name}`: {reason}")]
    FileParse { name: String, reason: String },
    #[error("`{key}` takes its value from module `{module}`, which is not registered")]
    ModuleMissing { key: String, module: String },
    #[error(
        "configuration file `{name}` is at config version {found}, this schema is at {current}; run `configurant migrate`"
    )]
    FileOutdated {
        name: String,
        found: u32,
        current: u32,
    },
    #[error(
        "configuration file `{name}` is at config version {found}, newer than this schema's {current}"
    )]
    FileNewer {
        name: String,
        found: u32,
        current: u32,
    },
    #[error("map `{key}` from {origin} has an empty key")]
    EmptyMapKey { key: String, origin: Origin },
    #[error("`{key}` is an array of tables or a map, which only the configuration file can set")]
    FileOnly { key: String },
    #[error("the configuration does not fit the target type: {reason}")]
    Deserialize { reason: String },
    #[error("`{key}` from {origin} was rejected by module `{module}`: {reason}")]
    ModuleRejected {
        key: String,
        module: String,
        reason: String,
        origin: Origin,
    },
}

fn suggest(s: &Option<String>) -> String {
    s.as_ref()
        .map(|s| format!(" (did you mean `{s}`?)"))
        .unwrap_or_default()
}

fn range(min: Option<f64>, max: Option<f64>) -> String {
    match (min, max) {
        (Some(a), Some(b)) => format!("{a}..={b}"),
        (Some(a), None) => format!(">= {a}"),
        (None, Some(b)) => format!("<= {b}"),
        (None, None) => "the allowed range".into(),
    }
}

/// Every error found in one load (the loader does not stop at the first).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub struct Errors(pub Vec<ConfigError>);

impl fmt::Display for Errors {
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

/// The closest of `known` to `unknown` by edit distance, if close enough to be a likely typo.
pub(crate) fn suggestion<'a>(
    unknown: &str,
    known: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let limit = (unknown.chars().count() / 3).max(2);
    known
        .map(|k| (edit_distance(unknown, k), k))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k.to_owned())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}
