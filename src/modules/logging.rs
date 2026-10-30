// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `logging` module (feature `logging`): `provider = "logging"` on a `string` or `enum` that holds a log
//! level (`info`) or a filter (`warn,my_crate::net=debug`). Rules L1 to L3 of `docs/MODULES.md` section 4; they
//! are pure functions of the text.
//!
//! The module only validates and describes. It installs no subscriber and returns the text unchanged, so a
//! project that wants lower case (L1 accepts any case) lowercases the level itself, and maps the validated text
//! to `tracing`, `log` or `env_logger` itself: ConfigurAnt gains no logging dependency.

use crate::module::{Choice, Module, Needs};
use crate::value::Value;

/// A log level, least to most verbose. The order is the order of verbosity, so `Level::Trace > Level::Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    /// Every level, least verbose first.
    pub const ALL: [Level; 6] = [
        Level::Off,
        Level::Error,
        Level::Warn,
        Level::Info,
        Level::Debug,
        Level::Trace,
    ];

    /// The lower-case name.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Off => "off",
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        }
    }

    /// A level from its name, in any case.
    pub fn parse(text: &str) -> Option<Level> {
        Level::ALL
            .into_iter()
            .find(|l| l.as_str().eq_ignore_ascii_case(text))
    }

    fn what_you_see(self) -> &'static str {
        match self {
            Level::Off => "what you see: nothing",
            Level::Error => "what you see: errors only",
            Level::Warn => "what you see: errors and warnings",
            Level::Info => "what you see: the normal progress of the program",
            Level::Debug => "what you see: detail for diagnosing a problem",
            Level::Trace => "what you see: everything, which may include request bodies",
        }
    }
}

/// The module a schema's `provider = "logging"` names. Options belong to the application, not the schema.
#[derive(Debug, Clone, Default)]
pub struct Logging {
    max_level: Option<Level>,
    levels_only: bool,
}

impl Logging {
    pub fn new() -> Self {
        Self::default()
    }

    /// L3: refuse any level more verbose than `level`, in a bare level and in every directive of a filter
    /// (a production build that must never emit `trace`, which can carry request bodies).
    pub fn max_level(mut self, level: Level) -> Self {
        self.max_level = Some(level);
        self
    }

    /// Accept a single level only (L1), not a filter: for a `log_level` key that the application hands to
    /// something that takes one level.
    pub fn levels_only(mut self) -> Self {
        self.levels_only = true;
        self
    }
}

impl Module for Logging {
    fn name(&self) -> &str {
        "logging"
    }

    fn list_choices(&self, _needs: &Needs) -> Vec<Choice> {
        Level::ALL
            .into_iter()
            .filter(|l| self.max_level.is_none_or(|max| *l <= max))
            .map(|l| Choice::new(l.as_str(), l.what_you_see()))
            .collect()
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let text = match value {
            Value::String(s) | Value::Enum(s) => s.trim(),
            other => {
                return Err(format!(
                    "logging L1: expected a level or a filter as text, found {}",
                    other.type_of().name()
                ));
            }
        };
        let directives = if self.levels_only {
            vec![(None, parse_level(text)?)]
        } else {
            parse_filter(text)?
        };
        if let Some(max) = self.max_level
            && let Some((_, level)) = directives.iter().find(|(_, l)| *l > max)
        {
            return Err(format!(
                "logging L3: level `{}` is more verbose than this key allows (at most `{}`)",
                level.as_str(),
                max.as_str()
            ));
        }
        Ok(())
    }
}

/// `value` quoted for a message: escaped, so a control character cannot reach a terminal, and cut short.
fn shown(value: &str) -> String {
    let cut: String = value.chars().take(32).collect();
    if cut.len() < value.len() {
        format!("{cut:?}...")
    } else {
        format!("{cut:?}")
    }
}

fn level_names() -> String {
    Level::ALL.map(Level::as_str).join(", ")
}

/// The level a mistyped one probably meant: a common alias, or a name it is the start of.
fn suggestion(token: &str) -> Option<Level> {
    let lower = token.to_ascii_lowercase();
    match lower.as_str() {
        "warning" => return Some(Level::Warn),
        "err" | "fatal" | "critical" => return Some(Level::Error),
        "information" => return Some(Level::Info),
        "dbg" => return Some(Level::Debug),
        "none" | "silent" | "quiet" => return Some(Level::Off),
        _ => {}
    }
    if lower.len() < 2 {
        return None;
    }
    Level::ALL
        .into_iter()
        .find(|l| l.as_str().starts_with(&lower))
}

/// L1: one level, in any case.
fn parse_level(token: &str) -> Result<Level, String> {
    if token.is_empty() {
        return Err(format!(
            "logging L1: the level is empty; use one of {}",
            level_names()
        ));
    }
    Level::parse(token).ok_or_else(|| {
        let hint = match suggestion(token) {
            Some(l) => format!(
                "did you mean `{}`? the levels are {}",
                l.as_str(),
                level_names()
            ),
            None => format!("use one of {}", level_names()),
        };
        format!("logging L1: {} is not a log level; {hint}", shown(token))
    })
}

/// L2: comma-separated `target=level` directives and at most one bare level, which is the default.
fn parse_filter(text: &str) -> Result<Vec<(Option<&str>, Level)>, String> {
    if text.is_empty() {
        return Err(format!(
            "logging L1: the value is empty; use a level ({}) or a filter such as `warn,my_crate=debug`",
            level_names()
        ));
    }
    let mut out = Vec::new();
    let mut defaults = 0;
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(
                "logging L2: a filter has an empty directive (a stray `,`); write `target=level`"
                    .into(),
            );
        }
        if let Some((target, level)) = part.split_once('=') {
            let target = target.trim();
            check_target(target)?;
            out.push((Some(target), parse_level(level.trim())?));
        } else if let Some(level) = Level::parse(part) {
            defaults += 1;
            if defaults > 1 {
                return Err(
                    "logging L2: a filter has two default levels; keep one bare level and give the rest a `target=`"
                        .into(),
                );
            }
            out.push((None, level));
        } else if suggestion(part).is_some() {
            // A mistyped level (`inf`, `warning`), not a target.
            parse_level(part)?;
        } else {
            return Err(format!(
                "logging L2: {} is neither a level nor `target=level`",
                shown(part)
            ));
        }
    }
    Ok(out)
}

/// A target is a Rust path: `::`-separated segments of letters, digits and `_`, none empty or starting with a digit.
fn check_target(target: &str) -> Result<(), String> {
    if target.is_empty() {
        return Err(
            "logging L2: the target before `=` is empty; write `my_crate::module=level`".into(),
        );
    }
    let ok = target.split("::").all(|segment| {
        let mut chars = segment.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "logging L2: {} is not a target; a target is a Rust path such as `my_crate::module`",
            shown(target)
        ))
    }
}
