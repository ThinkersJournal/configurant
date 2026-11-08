// SPDX-License-Identifier: MIT OR Apache-2.0
//! Projection: render the resolved, NON-secret values of a [`Config`] into the settings block a host
//! understands (ROADMAP row 33; WebHostAnt is the first customer).
//!
//! A host (a Cloudflare `wrangler.toml`, a Docker Compose file, a plain `.env`) wants the settings of an
//! app as environment variables. The schema already names the variable for a value (`env = "..."`), so
//! the projection is that name and the winning value, nothing else:
//!
//! - A `secret` value is **never read**. It appears only as its variable name and, if the schema declares
//!   one, its `<source>:<reference>` (a reference, not a value), so the host can be told where to get it.
//! - A value with no `env`, a table or a map, an unset value, and a name that is not a valid variable name
//!   are listed in [`Projection::skipped`] with the reason, never dropped silently.
//! - A list becomes comma-separated text in the loader's own escaping (`\,`, `\\`), so the app reads back
//!   what was projected.
//!
//! This writes nothing: it returns text. Applying it to a host, and reading anything back from one, are
//! not here (CireSnave 2026-11-08, design D7).

use std::fmt::Write as _;

use crate::config::Config;
use crate::value::Value;

/// A host format to render into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostFormat {
    /// `NAME="value"` lines for a dotenv-style reader that understands double quotes (`$` is written as is).
    Env,
    /// A `[vars]` table for `wrangler.toml`; a secret is a comment naming `wrangler secret put`.
    Wrangler,
    /// An `environment:` mapping for a Compose service; a secret is `NAME: ${NAME:?...}`, read from the
    /// host's own environment at deploy time.
    Compose,
}

impl HostFormat {
    /// The name used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            HostFormat::Env => "env",
            HostFormat::Wrangler => "wrangler",
            HostFormat::Compose => "compose",
        }
    }

    /// The format called `name`, if there is one.
    pub fn parse(name: &str) -> Option<HostFormat> {
        match name {
            "env" => Some(HostFormat::Env),
            "wrangler" => Some(HostFormat::Wrangler),
            "compose" => Some(HostFormat::Compose),
            _ => None,
        }
    }
}

/// One non-secret setting: the variable name and its value as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Var {
    /// The environment variable name (the schema's `env`).
    pub name: String,
    /// The value, as the loader would parse it back.
    pub value: String,
}

/// A secret the host must supply: never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    /// The environment variable the app reads it from.
    pub name: String,
    /// The schema's `<source>:<reference>`, if it declares one.
    pub source: Option<String>,
}

/// A value that has no place in the projection, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The dotted key.
    pub key: String,
    /// Why (never contains a value).
    pub reason: &'static str,
}

/// The settings of a [`Config`], ready to render.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Projection {
    /// Non-secret settings, in schema order.
    pub vars: Vec<Var>,
    /// Secrets the host supplies, in schema order.
    pub secrets: Vec<SecretRef>,
    /// What was left out and why.
    pub skipped: Vec<Skipped>,
}

impl Config {
    /// Project the resolved non-secret values into variables (see the [module](self)). A secret's value
    /// is not touched.
    pub fn project(&self) -> Projection {
        let mut p = Projection::default();
        for def in self.schema().values() {
            let Some(name) = def.env.as_deref() else {
                p.skipped.push(Skipped {
                    key: def.key.clone(),
                    reason: "no `env` name in the schema",
                });
                continue;
            };
            if !valid_name(name) {
                p.skipped.push(Skipped {
                    key: def.key.clone(),
                    reason: "its `env` name is not a valid variable name",
                });
                continue;
            }
            if def.secret {
                p.secrets.push(SecretRef {
                    name: name.to_owned(),
                    source: def.source.clone(),
                });
                continue;
            }
            match self.value(&def.key) {
                None => p.skipped.push(Skipped {
                    key: def.key.clone(),
                    reason: "not set",
                }),
                Some(v) => match text_of(v) {
                    Some(value) => p.vars.push(Var {
                        name: name.to_owned(),
                        value,
                    }),
                    None => p.skipped.push(Skipped {
                        key: def.key.clone(),
                        reason: "a table or map: file-only",
                    }),
                },
            }
        }
        p
    }
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn text_of(v: &Value) -> Option<String> {
    Some(match v {
        Value::String(s) | Value::Enum(s) => s.clone(),
        Value::Path(p) => p.display().to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::List(items) => items
            .iter()
            .map(|i| i.replace('\\', "\\\\").replace(',', "\\,"))
            .collect::<Vec<_>>()
            .join(","),
        Value::Rows(_) | Value::Map(_) => return None,
    })
}

/// A double-quoted string whose escapes mean the same in dotenv, TOML and YAML.
fn quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Text safe inside a one-line `#` comment.
fn comment(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

impl Projection {
    /// Render into `format`. The text ends with a newline. Skipped values are a comment at the end, by
    /// key and reason only.
    pub fn render(&self, format: HostFormat) -> String {
        let mut out = String::new();
        match format {
            HostFormat::Env => {
                for v in &self.vars {
                    let _ = writeln!(out, "{}={}", v.name, quoted(&v.value));
                }
                for s in &self.secrets {
                    let _ = writeln!(out, "# secret {}{}", s.name, source_note(s));
                }
            }
            HostFormat::Wrangler => {
                out.push_str("[vars]\n");
                for v in &self.vars {
                    let _ = writeln!(out, "{} = {}", v.name, quoted(&v.value));
                }
                for s in &self.secrets {
                    let _ = writeln!(
                        out,
                        "# secret {}: set with `wrangler secret put {}`{}",
                        s.name,
                        s.name,
                        source_note(s)
                    );
                }
            }
            HostFormat::Compose => {
                out.push_str("environment:\n");
                for v in &self.vars {
                    // Compose interpolates `$`; a doubled one is a literal.
                    let _ = writeln!(out, "  {}: {}", v.name, quoted(&v.value.replace('$', "$$")));
                }
                for s in &self.secrets {
                    let _ = writeln!(
                        out,
                        "  {}: ${{{}:?set {} in the host environment}}{}",
                        s.name,
                        s.name,
                        s.name,
                        source_note_comment(s)
                    );
                }
            }
        }
        for k in &self.skipped {
            let _ = writeln!(out, "# skipped {}: {}", comment(&k.key), k.reason);
        }
        out
    }
}

fn source_note(s: &SecretRef) -> String {
    match &s.source {
        Some(src) => format!(" (source {})", comment(src)),
        None => String::new(),
    }
}

fn source_note_comment(s: &SecretRef) -> String {
    match &s.source {
        Some(src) => format!(" # source {}", comment(src)),
        None => String::new(),
    }
}
