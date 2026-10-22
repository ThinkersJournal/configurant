// SPDX-License-Identifier: MIT OR Apache-2.0
//! Migrating configuration files between config versions, as data: the library returns the new
//! text and a list of changes; only a front end writes a file. Comments and layout survive.
//!
//! Every operation is reversible by construction (`rename`, a one-to-one `map_value`), so a file can
//! be carried back down. No error and no change line carries a value from the file.

use std::collections::BTreeMap;

use toml_edit::{DocumentMut, Item, Table, TableLike, Value as EditValue};

use crate::schema::{Op, Schema};

/// The result of a migration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Migrated {
    pub text: String,
    pub from: u32,
    pub to: u32,
    /// What changed, as key names only (never a value).
    pub changes: Vec<String>,
}

/// Why a file could not be migrated. No variant carries a value from the file.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum MigrateError {
    #[error("configuration file: {reason}")]
    FileParse { reason: String },
    #[error("the file has no `config_version`; say which version it is (`--from N`)")]
    VersionMissing,
    #[error("`config_version` must be a positive integer")]
    BadVersion,
    #[error("the file is at config version {found}, newer than this schema's {current}")]
    Newer { found: u32, current: u32 },
    #[error(
        "cannot migrate to config version {target}: the file is at {found} and this schema is at {current}"
    )]
    BadTarget {
        target: u32,
        found: u32,
        current: u32,
    },
    #[error("`{key}` already exists, so the rename onto it would lose one of the two")]
    DestinationExists { key: String },
    #[error("`{key}` holds a {found}; this migration maps strings")]
    WrongType { key: String, found: &'static str },
    #[error(
        "`{key}` already holds a value this migration produces, so it could not be carried back"
    )]
    Ambiguous { key: String },
}

impl Schema {
    /// Carry a file up to the schema's `config_version`. `from` names the version of a file that has no
    /// `config_version` of its own (a version in the file wins).
    pub fn migrate(&self, text: &str, from: Option<u32>) -> Result<Migrated, MigrateError> {
        let mut doc = parse(text)?;
        let current = self.project.config_version;
        let found = match (file_version(&doc)?, from) {
            (Some(v), _) | (None, Some(v)) => v,
            (None, None) => return Err(MigrateError::VersionMissing),
        };
        if found == 0 {
            return Err(MigrateError::BadVersion);
        }
        if found > current {
            return Err(MigrateError::Newer { found, current });
        }
        let mut changes = Vec::new();
        for step in &self.migrations()[(found as usize - 1)..(current as usize - 1)] {
            for op in &step.ops {
                apply(&mut doc, op, false, step.from, step.to, &mut changes)?;
            }
        }
        Ok(finish(doc, text, found, current, changes))
    }

    /// Carry a versioned file back down to config version `to`.
    pub fn migrate_back(&self, text: &str, to: u32) -> Result<Migrated, MigrateError> {
        let mut doc = parse(text)?;
        let current = self.project.config_version;
        let found = file_version(&doc)?.ok_or(MigrateError::VersionMissing)?;
        if found > current {
            return Err(MigrateError::Newer { found, current });
        }
        if to == 0 || to > found {
            return Err(MigrateError::BadTarget {
                target: to,
                found,
                current,
            });
        }
        let mut changes = Vec::new();
        for step in self.migrations()[(to as usize - 1)..(found as usize - 1)]
            .iter()
            .rev()
        {
            for op in step.ops.iter().rev() {
                apply(&mut doc, op, true, step.to, step.from, &mut changes)?;
            }
        }
        Ok(finish(doc, text, found, to, changes))
    }
}

fn parse(text: &str) -> Result<DocumentMut, MigrateError> {
    text.parse::<DocumentMut>().map_err(|e| {
        // The parser's message, never the offending line: that line may hold a secret.
        let line = e
            .span()
            .map(|s| {
                format!(
                    " (line {})",
                    text[..s.start.min(text.len())].matches('\n').count() + 1
                )
            })
            .unwrap_or_default();
        MigrateError::FileParse {
            reason: format!("{}{line}", e.message()),
        }
    })
}

fn file_version(doc: &DocumentMut) -> Result<Option<u32>, MigrateError> {
    match doc.get("config_version") {
        None => Ok(None),
        Some(item) => item
            .as_integer()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n >= 1)
            .map(Some)
            .ok_or(MigrateError::BadVersion),
    }
}

fn finish(
    mut doc: DocumentMut,
    original: &str,
    from: u32,
    to: u32,
    changes: Vec<String>,
) -> Migrated {
    if from == to {
        return Migrated {
            text: original.to_owned(),
            from,
            to,
            changes,
        };
    }
    let mut v = EditValue::from(i64::from(to));
    if let Some(old) = doc.get("config_version").and_then(Item::as_value) {
        *v.decor_mut() = old.decor().clone();
    }
    doc["config_version"] = Item::Value(v);
    Migrated {
        text: doc.to_string(),
        from,
        to,
        changes,
    }
}

/// Apply one operation, or its inverse when `backward`. `a` -> `b` labels the direction of travel.
fn apply(
    doc: &mut DocumentMut,
    op: &Op,
    backward: bool,
    a: u32,
    b: u32,
    changes: &mut Vec<String>,
) -> Result<(), MigrateError> {
    match op {
        Op::Rename { from, to } => {
            let (from, to) = if backward { (to, from) } else { (from, to) };
            if rename(doc.as_table_mut(), from, to)? {
                changes.push(format!("{a} -> {b}: renamed `{from}` to `{to}`"));
            }
        }
        Op::MapValue { key, map } => {
            let map: BTreeMap<&str, &str> = if backward {
                map.iter().map(|(k, v)| (v.as_str(), k.as_str())).collect()
            } else {
                map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
            };
            if map_value(doc.as_table_mut(), key, &map)? {
                changes.push(format!("{a} -> {b}: changed the value of `{key}`"));
            }
        }
    }
    Ok(())
}

/// The table holding the last segment of `path`, if every segment before it is a table.
fn parent_of<'a>(root: &'a mut Table, path: &'a str) -> Option<(&'a mut dyn TableLike, &'a str)> {
    let (dirs, leaf) = path.rsplit_once('.').map_or(("", path), |(d, l)| (d, l));
    let mut cur: &mut dyn TableLike = root;
    if !dirs.is_empty() {
        for seg in dirs.split('.') {
            cur = cur.get_mut(seg)?.as_table_like_mut()?;
        }
    }
    Some((cur, leaf))
}

/// Move `from` to `to`. `Ok(false)` if `from` is not in the file.
fn rename(root: &mut Table, from: &str, to: &str) -> Result<bool, MigrateError> {
    let exists = |root: &mut Table, path: &str| {
        parent_of(root, path).is_some_and(|(t, leaf)| t.contains_key(leaf))
    };
    if !exists(root, from) {
        return Ok(false);
    }
    if exists(root, to) {
        return Err(MigrateError::DestinationExists { key: to.to_owned() });
    }
    // Take the entry out (keeping the comments around its key), then prune parents it emptied.
    let (parent, leaf) = parent_of(root, from).expect("checked above");
    let key_decor = parent
        .key_mut(leaf)
        .map(|k| k.leaf_decor().clone())
        .unwrap_or_default();
    let item = parent.remove(leaf).expect("checked above");

    let (dirs, new_leaf) = to.rsplit_once('.').map_or(("", to), |(d, l)| (d, l));
    let mut cur: &mut dyn TableLike = root;
    if !dirs.is_empty() {
        for seg in dirs.split('.') {
            if !cur.contains_key(seg) {
                let mut t = Table::new();
                t.set_implicit(true);
                cur.insert(seg, Item::Table(t));
            }
            cur = cur
                .get_mut(seg)
                .and_then(Item::as_table_like_mut)
                .ok_or_else(|| MigrateError::DestinationExists { key: to.to_owned() })?;
        }
    }
    cur.insert(new_leaf, item);
    if let Some(mut k) = cur.key_mut(new_leaf) {
        *k.leaf_decor_mut() = key_decor;
    }
    // Only now: a table that still holds the destination keeps its header and comments.
    prune(root, from);
    Ok(true)
}

/// Remove tables along `path` that have become empty.
fn prune(root: &mut Table, path: &str) {
    let Some((dirs, _)) = path.rsplit_once('.') else {
        return;
    };
    let segs: Vec<&str> = dirs.split('.').collect();
    for depth in (1..=segs.len()).rev() {
        let joined = segs[..depth].join(".");
        let Some((parent, leaf)) = parent_of(root, &joined) else {
            return;
        };
        let empty = parent
            .get(leaf)
            .and_then(Item::as_table_like)
            .is_some_and(|t| t.is_empty());
        if !empty {
            return;
        }
        parent.remove(leaf);
    }
}

/// Replace the string at `key` through `map`. `Ok(false)` if the key is absent or its value is not
/// one the map moves.
fn map_value(
    root: &mut Table,
    key: &str,
    map: &BTreeMap<&str, &str>,
) -> Result<bool, MigrateError> {
    let Some((parent, leaf)) = parent_of(root, key) else {
        return Ok(false);
    };
    let Some(item) = parent.get_mut(leaf) else {
        return Ok(false);
    };
    let Some(old) = item.as_value() else {
        return Err(MigrateError::WrongType {
            key: key.to_owned(),
            found: item.type_name(),
        });
    };
    let Some(s) = old.as_str() else {
        return Err(MigrateError::WrongType {
            key: key.to_owned(),
            found: old.type_name(),
        });
    };
    match map.get(s) {
        Some(new) => {
            let mut v = EditValue::from(*new);
            *v.decor_mut() = old.decor().clone();
            *item = Item::Value(v);
            Ok(true)
        }
        // Not in the map, but equal to something the map produces: carrying it back would change it.
        None if map.values().any(|out| *out == s) => Err(MigrateError::Ambiguous {
            key: key.to_owned(),
        }),
        None => Ok(false),
    }
}
