# Changelog

## 0.2.0

- Config-file migrations. A file carries a top-level `config_version`; the schema declares
  `[project] config_version = N` and `[[migration]]` steps (`from`, `to = from + 1`, `ops`). Operations are
  `rename` and a one-to-one `map_value`: every one is reversible, and nothing that drops information is
  offered. `Schema::migrate` / `Schema::migrate_back` return the new text (comments and layout kept) and
  the changes as key names only; they never write a file. Collisions and ambiguous values are named errors.
- `configurant migrate <schema> <file> [--from N | --to N] [--write]`: prints by default; `--write`
  saves `<file>.bak` first and refuses if it exists.
- The loader refuses a file whose `config_version` is older (`ConfigError::FileOutdated`, says to run
  `migrate`) or newer (`FileNewer`); a file with no version loads as before.
- **Breaking:** `config_version` is now reserved. A schema that declares an ordinary value with the key
  `config_version` (or `config_version.*`) no longer parses (`SchemaError::ReservedKey`), and a config file's
  top-level `config_version` is read as the file's version. Fix: rename that value in the schema (and in
  your files), e.g. `app_config_version`. The new variants and `Project::config_version` are additive (the
  types are `#[non_exhaustive]`).

## 0.1.0

- **Breaking:** new variant `ConfigError::Deserialize`; and `ConfigError`, `Origin`, `Layer`, `ValueType`,
  `Value`, `SchemaError`, `ModuleError`, `ExplainLine`, `Explanation`, `Project` and `ValueDef` are now
  `#[non_exhaustive]`, so later variants and fields are additive.
- `Config::deserialize::<T>()`: opt-in serde struct binding by dotted key; no codegen. A serde error that
  would quote a secret value is withheld (`ConfigError::Deserialize`, a new variant).

## 0.0.1

- Repository scaffold: CI (jobs build-test, msrv, deny, spdx), `deny.toml`, `CODEOWNERS`, `SECURITY.md`.
- Schema types and strict parser with self-checks; layering (default < file < env < CLI); validation with
  named errors (all reported); `explain`; secrets enforced; the module interface with `ChoiceList`.
- `configurant check|describe|explain`.
