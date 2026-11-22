// SPDX-License-Identifier: MIT OR Apache-2.0
//! SecretAnt as a ConfigurAnt secret source: `source = "secretant:<name>"` reads `<name>` from a SecretAnt
//! store when the configuration loads. See `docs/SECRET-SOURCES.md` slice 4.
//!
//! The adapter holds a [`secretant::SecretReader`] (the store path, how to unlock it and a timeout, all chosen
//! by the application, never by the schema). A failure is a [`SourceFailure`] kind and nothing else: SecretAnt's
//! own errors carry a class only, and this adapter keeps it that way.

use std::fmt;

use configurant_secrets::{SecretSource, SecretValue, SourceFailure};
use secretant::{ReadConfig, ReadError, SecretReader};

/// Reads secrets from one SecretAnt store.
pub struct SecretAntSource {
    reader: SecretReader,
}

impl fmt::Debug for SecretAntSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretAntSource(<redacted>)")
    }
}

impl SecretAntSource {
    /// The name a schema's `source = "secretant:<name>"` refers to.
    pub const NAME: &'static str = "secretant";

    /// Wrap a reader the application already opened.
    pub fn new(reader: SecretReader) -> Self {
        SecretAntSource { reader }
    }

    /// Open the store `config` describes (checking that it unlocks, within its timeout) and wrap it.
    pub fn open(config: ReadConfig) -> Result<Self, SourceFailure> {
        SecretReader::open(config)
            .map(SecretAntSource::new)
            .map_err(failure)
    }
}

/// SecretAnt's four classes map one to one; a class added later is `Unreachable` until this crate learns it.
fn failure(e: ReadError) -> SourceFailure {
    match e {
        ReadError::NotFound => SourceFailure::NotFound,
        ReadError::Denied => SourceFailure::Denied,
        ReadError::TimedOut => SourceFailure::TimedOut,
        _ => SourceFailure::Unreachable,
    }
}

/// SecretAnt's own rule for a name: not empty, no control character.
fn is_name(reference: &str) -> bool {
    !reference.is_empty() && !reference.chars().any(char::is_control)
}

impl SecretSource for SecretAntSource {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check_reference(&self, reference: &str) -> bool {
        is_name(reference)
    }

    fn fetch(&self, reference: &str) -> Result<SecretValue, SourceFailure> {
        if !is_name(reference) {
            return Err(SourceFailure::NotFound);
        }
        let value = self.reader.get(reference).map_err(failure)?;
        // The loader takes text. A binary entry is not usable here, and says so without showing a byte.
        let text = std::str::from_utf8(value.expose()).map_err(|_| SourceFailure::Invalid)?;
        Ok(SecretValue::new(text))
    }
}

// Compile-checks the README's snippet (`cargo test --doc`).
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
