// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `paths` module (feature `paths`): `provider = "paths"` on a `path` (or `string`) value. Rules T1 to T5 of
//! `docs/MODULES.md` section 3. They are lexical: no file, link or drive is looked at, so a load stays
//! deterministic and works on a host where the directory does not exist yet.
//!
//! The rules differ by platform (separators, drives, reserved names), so the module carries a [`Platform`]
//! that defaults to the one it runs on and can be set, which also lets a test check Windows rules on Linux.
//!
//! A refusal names the rule and says what is wrong, but never quotes the value (a path can hold a user name or
//! a token), and the loader already replaces the whole reason of a `secret` key with `<redacted>`.

use std::path::Path;

use crate::exec::{Step, StepAction};
use crate::module::{Action, ActionKind, Choice, Module, Needs, Plan};
use crate::value::Value;

/// The path rules a [`Paths`] applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// `/` separates; a path is absolute if it starts with `/`; nothing else is reserved.
    Unix,
    /// `/` and `\` separate; drives (`C:`), UNC (`\\server\share`), reserved device names, no trailing dot or space.
    Windows,
}

impl Platform {
    /// The platform this program runs on.
    pub fn host() -> Platform {
        if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Unix
        }
    }

    fn is_sep(self, c: char) -> bool {
        c == '/' || (self == Platform::Windows && c == '\\')
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Any,
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Any,
    Relative,
    Absolute,
}

/// The module a schema's `provider = "paths"` names. Options belong to the application, not the schema.
#[derive(Debug, Clone)]
pub struct Paths {
    platform: Platform,
    kind: Kind,
    anchor: Anchor,
    within: Option<String>,
}

impl Default for Paths {
    fn default() -> Self {
        Paths {
            platform: Platform::host(),
            kind: Kind::Any,
            anchor: Anchor::Any,
            within: None,
        }
    }
}

impl Paths {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply another platform's rules (the default is the one the program runs on).
    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }

    /// The value names a directory: a trailing separator is fine, and a change offers a `Mkdir` step.
    pub fn directory(mut self) -> Self {
        self.kind = Kind::Directory;
        self
    }

    /// The value names a file: T5 refuses a trailing separator and a last component of `.` or `..`.
    pub fn file(mut self) -> Self {
        self.kind = Kind::File;
        self
    }

    /// T3: refuse a path with a root, drive or share.
    pub fn relative(mut self) -> Self {
        self.anchor = Anchor::Relative;
        self
    }

    /// T3: refuse a path that is not rooted completely (on Windows: a drive and a root, or a UNC share).
    pub fn absolute(mut self) -> Self {
        self.anchor = Anchor::Absolute;
        self
    }

    /// T2: after lexical normalisation the path must stay under `root`, which must be an absolute path of this
    /// module's platform. A relative value is taken relative to `root`.
    pub fn within(mut self, root: &str) -> Self {
        self.within = Some(root.to_owned());
        self
    }

    fn check(&self, text: &str) -> Result<(), String> {
        // T1
        if text.is_empty() {
            return Err("paths T1: the path is empty".into());
        }
        if text.chars().any(char::is_control) {
            return Err("paths T1: the path holds a NUL or control character".into());
        }
        let p = parse(text, self.platform)?;
        // T3
        match self.anchor {
            Anchor::Any => {}
            Anchor::Relative if p.prefix != Prefix::None => {
                return Err(
                    "paths T3: this key takes a relative path; remove the leading root, drive or share".into(),
                );
            }
            Anchor::Absolute if !p.is_absolute() => {
                return Err(if p.prefix == Prefix::None {
                    "paths T3: this key takes an absolute path; write it from the root".into()
                } else {
                    "paths T3: this key takes an absolute path; this one is rooted but not complete (it has no drive, or a drive without a root)"
                        .into()
                });
            }
            _ => {}
        }
        // T4
        if self.platform == Platform::Windows {
            for c in &p.comps {
                check_windows_component(c)?;
            }
        }
        // T5
        if self.kind == Kind::File
            && (p.trailing || matches!(p.comps.last(), Some(&".") | Some(&"..")))
        {
            return Err(
                "paths T5: this key names a file, but the path ends in a separator, `.` or `..`, which is a directory"
                    .into(),
            );
        }
        // T2
        if let Some(root) = &self.within {
            self.check_within(&p, root)?;
        }
        Ok(())
    }

    fn check_within(&self, p: &Parsed<'_>, root: &str) -> Result<(), String> {
        let root = parse(root, self.platform)
            .ok()
            .filter(Parsed::is_absolute)
            .ok_or_else(|| {
                "paths T2: the application's `within` root is not an absolute path (a mistake in the program, not in this value)"
                    .to_owned()
            })?;
        let root_comps = normalise(&root.comps, self.platform);
        let comps = match &p.prefix {
            Prefix::None => {
                let joined: Vec<&str> = root_comps
                    .iter()
                    .map(String::as_str)
                    .chain(p.comps.iter().copied())
                    .collect();
                normalise(&joined, self.platform)
            }
            prefix if p.is_absolute() && same_prefix(prefix, &root.prefix, self.platform) => {
                normalise(&p.comps, self.platform)
            }
            prefix
                if p.is_absolute()
                    || *prefix == Prefix::Rooted && self.platform == Platform::Unix =>
            {
                return Err("paths T2: the path is outside the allowed directory (a different root, drive or share)".into());
            }
            _ => {
                return Err("paths T2: the path has no drive, so it cannot be checked against the allowed directory; write it in full".into());
            }
        };
        if comps.starts_with(&root_comps) {
            Ok(())
        } else {
            Err("paths T2: the path leaves the allowed directory (check for `..` and for an absolute path elsewhere)".into())
        }
    }

    fn text<'a>(&self, value: &'a Value) -> Result<&'a str, String> {
        match value {
            Value::String(s) => Ok(s),
            Value::Path(p) => {
                Path::to_str(p).ok_or_else(|| "paths T1: the path is not valid UTF-8".to_owned())
            }
            other => Err(format!(
                "paths T1: expected a path, found {}",
                other.type_of().name()
            )),
        }
    }
}

impl Module for Paths {
    fn name(&self) -> &str {
        "paths"
    }

    fn list_choices(&self, _needs: &Needs) -> Vec<Choice> {
        Vec::new()
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        self.check(self.text(value)?)
    }

    /// A new, valid, absolute directory gets a `Mkdir` step, shown and confirmed by a front end. The old
    /// directory is never removed or moved: moving data is a transfer, which is a separate, explicit command.
    fn on_change(&self, old: &Value, new: &Value) -> Plan {
        if self.kind != Kind::Directory || old == new {
            return Plan::default();
        }
        let Ok(text) = self.text(new) else {
            return Plan::default();
        };
        let Ok(parsed) = parse(text, self.platform) else {
            return Plan::default();
        };
        if self.check(text).is_err() || !parsed.is_absolute() {
            return Plan::default();
        }
        let trimmed = trim_separators(text, self.platform);
        Plan {
            actions: vec![Action {
                description: format!(
                    "Create the directory `{trimmed}`; the old directory is left where it is (moving its data is a separate transfer)"
                ),
                kind: ActionKind::Step(Box::new(Step::new(
                    "paths-mkdir",
                    "Create the new directory",
                    StepAction::Mkdir {
                        path: trimmed.to_owned(),
                        mode: None,
                    },
                ))),
            }],
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Prefix {
    /// No root: a relative path.
    None,
    /// A root with no drive: `/x`, and on Windows `\x`.
    Rooted,
    /// Windows `C:` with `rooted` true for `C:\x` and false for the drive-relative `C:x`.
    Drive { letter: char, rooted: bool },
    /// Windows `\\server\share`.
    Unc { server: String, share: String },
}

struct Parsed<'a> {
    prefix: Prefix,
    comps: Vec<&'a str>,
    trailing: bool,
    /// Parsed under Unix rules, where a bare root is complete.
    unix: bool,
}

impl Parsed<'_> {
    fn is_absolute(&self) -> bool {
        match &self.prefix {
            Prefix::Rooted => self.unix,
            Prefix::Drive { rooted, .. } => *rooted,
            Prefix::Unc { .. } => true,
            Prefix::None => false,
        }
    }
}

fn parse(text: &str, platform: Platform) -> Result<Parsed<'_>, String> {
    let sep = |c: char| platform.is_sep(c);
    let chars: Vec<char> = text.chars().take(3).collect();
    let (prefix, rest) = if platform == Platform::Windows {
        if chars.len() >= 2 && sep(chars[0]) && sep(chars[1]) {
            let after = &text[2..];
            let first = after.split(sep).next().unwrap_or("");
            if first == "?" || first == "." {
                return Err(
                    "paths T4: a `\\\\?\\` or `\\\\.\\` path skips the checks Windows normally makes; write the path without it"
                        .into(),
                );
            }
            let mut parts = after.splitn(3, sep);
            let server = parts.next().unwrap_or("");
            let share = parts.next().unwrap_or("");
            if server.is_empty() || share.is_empty() {
                return Err("paths T4: a network path needs both a server and a share (`\\\\server\\share`)".into());
            }
            (
                Prefix::Unc {
                    server: server.to_owned(),
                    share: share.to_owned(),
                },
                parts.next().unwrap_or(""),
            )
        } else if chars.len() >= 2 && chars[0].is_ascii_alphabetic() && chars[1] == ':' {
            let rest = &text[2..];
            let rooted = rest.chars().next().is_some_and(sep);
            if !rooted {
                return Err(
                    "paths T4: a drive with no separator after it (`C:x`) depends on the program's current directory on that drive; write `C:\\x`, and a `:` is allowed only in a drive prefix such as `C:\\`"
                        .into(),
                );
            }
            (
                Prefix::Drive {
                    letter: chars[0].to_ascii_lowercase(),
                    rooted,
                },
                rest,
            )
        } else if chars.first().copied().is_some_and(sep) {
            (Prefix::Rooted, text)
        } else {
            (Prefix::None, text)
        }
    } else if text.starts_with('/') {
        (Prefix::Rooted, text)
    } else {
        (Prefix::None, text)
    };
    let comps: Vec<&str> = rest.split(sep).filter(|c| !c.is_empty()).collect();
    let trailing = text.chars().last().is_some_and(sep);
    Ok(Parsed {
        prefix,
        comps,
        trailing,
        unix: platform == Platform::Unix,
    })
}

fn same_prefix(a: &Prefix, b: &Prefix, platform: Platform) -> bool {
    match (a, b) {
        (Prefix::Rooted, Prefix::Rooted) => platform == Platform::Unix,
        (Prefix::Drive { letter: x, .. }, Prefix::Drive { letter: y, .. }) => x == y,
        (
            Prefix::Unc {
                server: s1,
                share: h1,
            },
            Prefix::Unc {
                server: s2,
                share: h2,
            },
        ) => s1.eq_ignore_ascii_case(s2) && h1.eq_ignore_ascii_case(h2),
        _ => false,
    }
}

/// Resolve `.` and `..` (a `..` at the top stays at the top, as the operating system does for an absolute path),
/// folding case on Windows.
fn normalise(comps: &[&str], platform: Platform) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in comps {
        match *c {
            "." => {}
            ".." => {
                out.pop();
            }
            other if platform == Platform::Windows => out.push(other.to_lowercase()),
            other => out.push(other.to_owned()),
        }
    }
    out
}

fn trim_separators(text: &str, platform: Platform) -> &str {
    let trimmed = text.trim_end_matches(|c| platform.is_sep(c));
    if trimmed.is_empty() || trimmed.ends_with(':') {
        text
    } else {
        trimmed
    }
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// T4 for one component of a Windows path.
fn check_windows_component(c: &str) -> Result<(), String> {
    if c == "." || c == ".." {
        return Ok(());
    }
    if c.chars().any(|ch| "<>:\"|?*".contains(ch)) {
        return Err("paths T4: a Windows name may not hold any of `< > : \" | ? *` (a `:` is allowed only after a drive letter)".into());
    }
    if c.ends_with('.') || c.ends_with(' ') {
        return Err("paths T4: a Windows name may not end in a dot or a space".into());
    }
    let stem = c.split('.').next().unwrap_or("").trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err("paths T4: a component is a reserved Windows device name (CON, PRN, AUX, NUL, COM1 to COM9, LPT1 to LPT9), with or without an extension".into());
    }
    Ok(())
}
