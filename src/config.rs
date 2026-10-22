// SPDX-License-Identifier: MIT OR Apache-2.0
//! Loading: layer defaults, a file, the environment and the command line over a [`Schema`],
//! validate every input with a named error, and keep each value's whole history for `explain`.
//!
//! Inputs are passed in as data (file text, an environment map, an argument list), so a load is
//! a pure function of its inputs; only the `configurant` binary reads the process environment.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use crate::error::{ConfigError, Errors, Origin, suggestion};
use crate::module::Modules;
use crate::schema::{Schema, ValueDef, Violation};
use crate::value::{Value, ValueType};

const REDACTED: &str = "<redacted>";

/// A source of values. Later layers in the order override earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Layer {
    Default,
    File,
    Env,
    Cli,
}

/// The precedence order, lowest first: default < file < environment < command line.
pub const DEFAULT_ORDER: [Layer; 4] = [Layer::Default, Layer::File, Layer::Env, Layer::Cli];

struct Candidate {
    key: String,
    origin: Origin,
    value: Value,
}

/// Collects the inputs of one load.
pub struct Loader<'a> {
    schema: &'a Schema,
    modules: Option<&'a Modules>,
    file: Option<(String, String)>,
    env: BTreeMap<String, String>,
    cli: Vec<String>,
}

impl<'a> Loader<'a> {
    pub fn new(schema: &'a Schema) -> Self {
        Loader {
            schema,
            modules: None,
            file: None,
            env: BTreeMap::new(),
            cli: Vec::new(),
        }
    }

    /// The modules that values with a `provider` are checked by.
    pub fn modules(mut self, modules: &'a Modules) -> Self {
        self.modules = Some(modules);
        self
    }

    /// A TOML configuration file's text, and the name to show for it.
    pub fn file(mut self, name: &str, text: &str) -> Self {
        self.file = Some((name.to_owned(), text.to_owned()));
        self
    }

    /// Environment variables (the whole environment is fine: only declared names and unknown
    /// names under the schema's `env_prefix` matter).
    pub fn env(mut self, env: BTreeMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Command-line arguments, without the program name.
    pub fn cli(mut self, args: Vec<String>) -> Self {
        self.cli = args;
        self
    }

    /// Layer in the standard order and validate.
    pub fn build(&self) -> Result<Config, Errors> {
        self.build_with_order(&DEFAULT_ORDER)
    }

    /// Layer in a given order, lowest precedence first. Exists so the precedence test can prove
    /// that it fails against an inverted order.
    pub fn build_with_order(&self, order: &[Layer]) -> Result<Config, Errors> {
        let mut errors = Vec::new();
        let default = self.default_candidates();
        let file = self.file_candidates(&mut errors);
        let env = self.env_candidates(&mut errors);
        let cli = self.cli_candidates(&mut errors);

        let mut chains: BTreeMap<String, Vec<(Origin, Value)>> = BTreeMap::new();
        for layer in order {
            let batch = match layer {
                Layer::Default => &default,
                Layer::File => &file,
                Layer::Env => &env,
                Layer::Cli => &cli,
            };
            for c in batch {
                chains
                    .entry(c.key.clone())
                    .or_default()
                    .push((c.origin.clone(), c.value.clone()));
            }
        }
        for d in self.schema.values() {
            if d.required && !chains.contains_key(&d.key) {
                errors.push(ConfigError::MissingRequired { key: d.key.clone() });
            }
        }
        if !errors.is_empty() {
            return Err(Errors(errors));
        }
        Ok(Config {
            schema: self.schema.clone(),
            chains,
        })
    }

    fn default_candidates(&self) -> Vec<Candidate> {
        self.schema
            .values()
            .iter()
            .filter_map(|d| {
                self.schema.default_of(&d.key).map(|v| Candidate {
                    key: d.key.clone(),
                    origin: Origin::Default,
                    value: v.clone(),
                })
            })
            .collect()
    }

    fn file_candidates(&self, errors: &mut Vec<ConfigError>) -> Vec<Candidate> {
        let Some((name, text)) = &self.file else {
            return Vec::new();
        };
        let mut table: toml::Table = match toml::from_str(text) {
            Ok(t) => t,
            Err(e) => {
                let line = e
                    .span()
                    .map(|s| {
                        format!(
                            " (line {})",
                            text[..s.start.min(text.len())].matches('\n').count() + 1
                        )
                    })
                    .unwrap_or_default();
                errors.push(ConfigError::FileParse {
                    name: name.clone(),
                    reason: format!("{}{line}", e.message()),
                });
                return Vec::new();
            }
        };
        // `config_version` belongs to the loader, not to the schema's values.
        if let Some(v) = table.remove("config_version") {
            let current = self.schema.project.config_version;
            let origin = Origin::File(name.clone());
            match v {
                toml::Value::Integer(n) if n == i64::from(current) => {}
                toml::Value::Integer(n) if (1..i64::from(current)).contains(&n) => {
                    errors.push(ConfigError::FileOutdated {
                        name: name.clone(),
                        found: n as u32,
                        current,
                    });
                    return Vec::new();
                }
                toml::Value::Integer(n) if n > i64::from(current) && n <= i64::from(u32::MAX) => {
                    errors.push(ConfigError::FileNewer {
                        name: name.clone(),
                        found: n as u32,
                        current,
                    });
                    return Vec::new();
                }
                other => errors.push(ConfigError::WrongType {
                    key: "config_version".into(),
                    expected: "a positive int no newer than the schema's",
                    found: other.type_str().into(),
                    origin,
                }),
            }
        }
        let mut out = Vec::new();
        self.flatten("", &table, name, &mut out, errors);
        out
    }

    fn flatten(
        &self,
        prefix: &str,
        table: &toml::Table,
        file: &str,
        out: &mut Vec<Candidate>,
        errors: &mut Vec<ConfigError>,
    ) {
        let origin = Origin::File(file.to_owned());
        for (k, v) in table {
            let path = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            let def = self.schema.get(&path);
            match (v, def) {
                (toml::Value::Table(_), Some(d)) => errors.push(ConfigError::WrongType {
                    key: path,
                    expected: d.ty.name(),
                    found: "table".into(),
                    origin: origin.clone(),
                }),
                (toml::Value::Table(t), None) if !t.is_empty() => {
                    self.flatten(&path, t, file, out, errors);
                }
                (_, None) => errors.push(self.unknown(path, origin.clone())),
                (_, Some(d)) if d.secret => errors.push(ConfigError::SecretInForbiddenSource {
                    key: path,
                    origin: origin.clone(),
                }),
                (leaf, Some(d)) => match Value::from_toml(d.ty, leaf) {
                    Err(found) => errors.push(ConfigError::WrongType {
                        key: path,
                        expected: d.ty.name(),
                        found: found.into(),
                        origin: origin.clone(),
                    }),
                    Ok(value) => self.accept(d, value, origin.clone(), out, errors),
                },
            }
        }
    }

    fn env_candidates(&self, errors: &mut Vec<ConfigError>) -> Vec<Candidate> {
        let mut out = Vec::new();
        let prefix = self.schema.project.env_prefix.as_deref();
        for (name, raw) in &self.env {
            let origin = Origin::Env(name.clone());
            match self
                .schema
                .values()
                .iter()
                .find(|d| d.env.as_deref() == Some(name.as_str()))
            {
                Some(d) => self.accept_text(d, raw, origin, &mut out, errors),
                None if prefix.is_some_and(|p| name.starts_with(p)) => {
                    let known = self.schema.values().iter().filter_map(|d| d.env.as_deref());
                    errors.push(ConfigError::UnknownKey {
                        key: name.clone(),
                        suggestion: suggestion(name, known),
                        origin,
                    });
                }
                None => {}
            }
        }
        out
    }

    fn cli_candidates(&self, errors: &mut Vec<ConfigError>) -> Vec<Candidate> {
        let mut out = Vec::new();
        let args = &self.cli;
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if !arg.starts_with("--") {
                // Never echo it: a positional argument could be a secret.
                errors.push(ConfigError::UnexpectedArgument { index: i });
                i += 1;
                continue;
            }
            let (name, inline) = match arg.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (arg.as_str(), None),
            };
            i += 1;
            let origin = Origin::Cli(name.to_owned());
            let Some(def) = self
                .schema
                .values()
                .iter()
                .find(|d| d.flag.as_deref() == Some(name))
            else {
                let known = self
                    .schema
                    .values()
                    .iter()
                    .filter_map(|d| d.flag.as_deref());
                errors.push(ConfigError::UnknownKey {
                    key: name.to_owned(),
                    suggestion: suggestion(name, known),
                    origin,
                });
                continue;
            };
            let raw = match inline {
                Some(v) => v,
                None if def.ty == ValueType::Bool => "true".to_owned(),
                None if i < args.len() && !args[i].starts_with("--") => {
                    i += 1;
                    args[i - 1].clone()
                }
                None => {
                    errors.push(ConfigError::MissingCliValue {
                        flag: name.to_owned(),
                    });
                    continue;
                }
            };
            self.accept_text(def, &raw, origin, &mut out, errors);
        }
        out
    }

    fn accept_text(
        &self,
        def: &ValueDef,
        raw: &str,
        origin: Origin,
        out: &mut Vec<Candidate>,
        errors: &mut Vec<ConfigError>,
    ) {
        match Value::parse_text(def.ty, raw) {
            Some(value) => self.accept(def, value, origin, out, errors),
            None => errors.push(ConfigError::WrongType {
                key: def.key.clone(),
                expected: def.ty.name(),
                found: if def.secret {
                    REDACTED.into()
                } else {
                    format!("{raw:?}")
                },
                origin,
            }),
        }
    }

    /// Check a typed value against its definition and its module, and keep it if it passes.
    fn accept(
        &self,
        def: &ValueDef,
        value: Value,
        origin: Origin,
        out: &mut Vec<Candidate>,
        errors: &mut Vec<ConfigError>,
    ) {
        match check_value(def, &value, self.modules, &origin) {
            Ok(()) => out.push(Candidate {
                key: def.key.clone(),
                origin,
                value,
            }),
            Err(e) => errors.push(e),
        }
    }

    fn unknown(&self, key: String, origin: Origin) -> ConfigError {
        let known = self.schema.values().iter().map(|d| d.key.as_str());
        ConfigError::UnknownKey {
            suggestion: suggestion(&key, known),
            key,
            origin,
        }
    }
}

/// Check a typed value against its definition and, if the key has a provider, that module. Shared by
/// the loader and by [`crate::edit::Session`] so an edit meets exactly the rules a load does.
pub(crate) fn check_value(
    def: &ValueDef,
    value: &Value,
    modules: Option<&Modules>,
    origin: &Origin,
) -> Result<(), ConfigError> {
    let found = || {
        if def.secret {
            REDACTED.to_owned()
        } else {
            value.shown()
        }
    };
    match def.violation(value) {
        Some(Violation::OutOfRange) => {
            return Err(ConfigError::OutOfRange {
                key: def.key.clone(),
                found: found(),
                min: def.min,
                max: def.max,
                origin: origin.clone(),
            });
        }
        Some(Violation::NotAllowed) => {
            return Err(ConfigError::NotAllowed {
                key: def.key.clone(),
                found: found(),
                choices: def.choices.clone(),
                origin: origin.clone(),
            });
        }
        None => {}
    }
    if let Some(module) = &def.provider {
        match modules.and_then(|m| m.get(module)) {
            None => {
                return Err(ConfigError::ModuleMissing {
                    key: def.key.clone(),
                    module: module.clone(),
                });
            }
            Some(m) => {
                if let Err(reason) = m.validate(value) {
                    return Err(ConfigError::ModuleRejected {
                        key: def.key.clone(),
                        module: module.clone(),
                        reason: if def.secret { REDACTED.into() } else { reason },
                        origin: origin.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// A type that can be read out of a [`Value`].
pub trait FromValue: Sized {
    const EXPECTED: &'static str;
    fn from_value(v: &Value) -> Option<Self>;
}

impl FromValue for String {
    const EXPECTED: &'static str = "string";
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::String(s) | Value::Enum(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl FromValue for i64 {
    const EXPECTED: &'static str = "int";
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
}

impl FromValue for f64 {
    const EXPECTED: &'static str = "float";
    fn from_value(v: &Value) -> Option<Self> {
        v.as_f64()
    }
}

impl FromValue for bool {
    const EXPECTED: &'static str = "bool";
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl FromValue for PathBuf {
    const EXPECTED: &'static str = "path";
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Path(p) => Some(p.clone()),
            _ => None,
        }
    }
}

impl FromValue for Vec<String> {
    const EXPECTED: &'static str = "list";
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::List(l) => Some(l.clone()),
            _ => None,
        }
    }
}

/// A validated configuration with the history of every value.
pub struct Config {
    schema: Schema,
    chains: BTreeMap<String, Vec<(Origin, Value)>>,
}

impl Config {
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The winning value of `key`, if any layer set it.
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.chains.get(key).and_then(|c| c.last()).map(|(_, v)| v)
    }

    /// Typed access. An unknown key, an unset key and a wrong type are all named errors.
    pub fn get<T: FromValue>(&self, key: &str) -> Result<T, ConfigError> {
        if self.schema.get(key).is_none() {
            return Err(self.unknown_lookup(key));
        }
        let Some((origin, v)) = self.chains.get(key).and_then(|c| c.last()) else {
            return Err(ConfigError::NotSet {
                key: key.to_owned(),
            });
        };
        T::from_value(v).ok_or_else(|| ConfigError::WrongType {
            key: key.to_owned(),
            expected: T::EXPECTED,
            found: v.type_of().name().into(),
            origin: origin.clone(),
        })
    }

    /// Bind the winning values to a struct of the caller's own, by dotted key (`server.port` is the
    /// field `port` of the field `server`). Opt-in and generates nothing: the schema stays the source of
    /// truth and the struct is only a typed view. Unset keys are absent, so an `Option` field is `None`.
    /// An error never carries a secret's value: if serde's message would contain one, it is withheld.
    pub fn deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, ConfigError> {
        let mut root = toml::Table::new();
        let mut secrets = Vec::new();
        for def in self.schema.values() {
            let v = self.value(&def.key);
            if let Some(v) = v.filter(|_| def.secret) {
                secrets.push(v.clone());
            }
            let mut parts: Vec<&str> = def.key.split('.').collect();
            let leaf = parts.pop().unwrap_or_default();
            let mut table = &mut root;
            for part in parts {
                table = table
                    .entry(part)
                    .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                    .as_table_mut()
                    .expect("the schema rejects a key that is both a value and a table prefix");
            }
            if let Some(v) = v {
                table.insert(leaf.to_owned(), v.to_toml());
            }
        }
        T::deserialize(toml::Value::Table(root)).map_err(|e| {
            let reason = e.to_string();
            let leaks = secrets.iter().any(|s| s.appears_in(&reason));
            ConfigError::Deserialize {
                reason: if leaks {
                    format!("{REDACTED}: the message quoted a secret value")
                } else {
                    reason
                },
            }
        })
    }

    /// Where did this value come from, and what did it override? Secrets are redacted.
    pub fn explain(&self, key: &str) -> Result<Explanation, ConfigError> {
        let def = self
            .schema
            .get(key)
            .ok_or_else(|| self.unknown_lookup(key))?;
        let chain = self.chains.get(key).map(Vec::as_slice).unwrap_or_default();
        Ok(Explanation {
            key: key.to_owned(),
            ty: def.ty,
            description: def.description.clone(),
            secret: def.secret,
            restart: def.restart,
            chain: chain
                .iter()
                .enumerate()
                .map(|(i, (origin, v))| ExplainLine {
                    origin: origin.clone(),
                    shown: if def.secret {
                        REDACTED.into()
                    } else {
                        v.shown()
                    },
                    wins: i + 1 == chain.len(),
                })
                .collect(),
        })
    }

    fn unknown_lookup(&self, key: &str) -> ConfigError {
        let known = self.schema.values().iter().map(|d| d.key.as_str());
        ConfigError::UnknownKey {
            key: key.to_owned(),
            suggestion: suggestion(key, known),
            origin: Origin::Lookup,
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for (key, chain) in &self.chains {
            let secret = self.schema.get(key).is_some_and(|d| d.secret);
            let shown = match chain.last() {
                Some(_) if secret => REDACTED.to_owned(),
                Some((_, v)) => v.shown(),
                None => "<unset>".to_owned(),
            };
            map.entry(key, &format_args!("{shown}"));
        }
        map.finish()
    }
}

/// One layer that set a key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExplainLine {
    pub origin: Origin,
    /// The value as shown (`<redacted>` for a secret).
    pub shown: String,
    pub wins: bool,
}

/// The history of one value, lowest precedence first.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Explanation {
    pub key: String,
    pub ty: ValueType,
    pub description: String,
    pub secret: bool,
    pub restart: bool,
    pub chain: Vec<ExplainLine>,
}

impl fmt::Display for Explanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}): {}", self.key, self.ty.name(), self.description)?;
        if self.secret {
            write!(f, " [secret]")?;
        }
        if self.restart {
            write!(f, " [restart needed]")?;
        }
        if self.chain.is_empty() {
            return write!(f, "\n  not set");
        }
        for line in &self.chain {
            let mark = if line.wins { "  <- used" } else { "" };
            write!(f, "\n  {}: {}{mark}", line.origin, line.shown)?;
        }
        Ok(())
    }
}
