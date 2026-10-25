// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `file` source: `source = "file:<name>"` reads `<name>` from a secrets directory the operator names with
//! `CONFIGURANT_SECRETS_DIR` (a mounted `/run/secrets`-style directory). See `docs/SECRET-SOURCES.md` slice 3.
//!
//! Nothing a file holds, and no path, can reach an error: a failure is a [`SourceFailure`] kind.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::{SecretSource, SecretValue, SourceFailure};

/// Reads secrets from files in one directory.
#[derive(Debug, Clone)]
pub struct FileSource {
    dir: PathBuf,
}

impl FileSource {
    /// The environment variable that names the secrets directory. It is never in the schema, so a schema holds
    /// no machine path.
    pub const DIR_ENV: &'static str = "CONFIGURANT_SECRETS_DIR";

    /// The largest file read, in bytes.
    pub const MAX_BYTES: usize = 64 * 1024;

    pub fn new(dir: impl Into<PathBuf>) -> Self {
        FileSource { dir: dir.into() }
    }

    /// The source for the directory `env` names, or `None` when [`DIR_ENV`](Self::DIR_ENV) is unset or empty.
    pub fn from_env(env: &BTreeMap<String, String>) -> Option<Self> {
        env.get(Self::DIR_ENV)
            .filter(|dir| !dir.is_empty())
            .map(FileSource::new)
    }
}

/// A bare file name: no separator, no `..`, no drive letter or absolute path, no NUL.
fn is_bare_name(reference: &str) -> bool {
    !reference.is_empty()
        && reference != "."
        && reference != ".."
        && !reference.contains(['/', '\\', ':', '\0'])
}

impl SecretSource for FileSource {
    fn name(&self) -> &str {
        "file"
    }

    fn check_reference(&self, reference: &str) -> bool {
        is_bare_name(reference)
    }

    fn fetch(&self, reference: &str) -> Result<SecretValue, SourceFailure> {
        if !is_bare_name(reference) {
            return Err(SourceFailure::Denied);
        }
        let dir = self.dir.canonicalize().map_err(not_found_or_denied)?;
        // Resolving links first means a link is followed only when it ends inside the directory (a container
        // mount links `name -> ..data/name`); one that leaves it is refused.
        let path = dir
            .join(reference)
            .canonicalize()
            .map_err(not_found_or_denied)?;
        if !path.starts_with(&dir) || path == dir {
            return Err(SourceFailure::Denied);
        }
        read_private(&path)
    }
}

fn not_found_or_denied(e: std::io::Error) -> SourceFailure {
    if e.kind() == std::io::ErrorKind::NotFound {
        SourceFailure::NotFound
    } else {
        SourceFailure::Denied
    }
}

fn read_private(path: &Path) -> Result<SecretValue, SourceFailure> {
    let file = std::fs::File::open(path).map_err(not_found_or_denied)?;
    // Asked of the open handle, not the path, so the file cannot change between the check and the read.
    let meta = file.metadata().map_err(not_found_or_denied)?;
    if !meta.is_file() {
        return Err(SourceFailure::Denied);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(SourceFailure::Denied);
        }
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(FileSource::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SourceFailure::Unreachable)?;
    if bytes.len() > FileSource::MAX_BYTES {
        return Err(SourceFailure::Invalid);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| SourceFailure::Invalid)?;
    let text = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    Ok(SecretValue::new(text))
}
