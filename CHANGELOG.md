# Changelog

## 0.2.6

- Design note `docs/TRANSFER.md`: how the PersistAnt module will execute a `DataTransfer` (copy then verify,
  TTL kept, secret endpoints never printed). Docs only; all three crates move to 0.2.6.

## 0.2.5

- `configurant-gui`: a window front end on egui (eframe 0.37, OpenGL backend, AccessKit on). A thin view over
  `edit::Session`, like the TUI: select a value, edit it (a bad value stays in the box with its error), review,
  confirm, and only then write `<file>.bak` and the file. Secrets are never held or drawn and have no edit box.
  Keyboard-only operation and accessibility labels are tested through the accessibility tree
  (`egui_kittest`); `docs/GUI.md` says what egui does and does not give. All three crates move to 0.2.5; the core
  is unchanged.
  The minimum Rust version rises from 1.94 to 1.95, which egui 0.37 requires.

## 0.2.4

- Arrays of tables and maps in the core. A schema declares `[[table]]` (key, description, `required`, `restart`,
  and `[[table.field]]` entries that are `[[value]]`s with a single-name key and no `env`, `flag`, `provider` or
  `secret`) and `[[map]]` (key, `value_type` of string, int, float, bool or path). A file sets them as `[[name]]`
  rows and as a table. Every row is checked (unknown key with suggestion, type, range, choices, required, field
  defaults) and errors name the row (`mount[1].guest`); a bad row means nothing of the array loads. They come from
  the file only: a secret is refused in a row or map by the schema, and no environment variable or flag reaches
  them. New: `Config::rows`, `Config::map`, `Schema::{tables, maps, table, map, all_keys}`, `Value::{Rows, Map}`,
  `ValueType::{Table, Map}` (both refused as a `[[value]]` type), `ConfigError::{EmptyMapKey, FileOnly}`;
  `explain`, `help` and `Config::deserialize` cover them. `edit::Session` refuses to stage them (`FileOnly`) and
  keeps them in the file untouched. Additive (the new enum variants are on `#[non_exhaustive]` types).
- `examples/antspawn.toml` now covers `[[mount]]`, `[[output]]` and `job.env`.

## 0.2.3

- First customer schema: `examples/antspawn.toml` describes an AntSpawn job spec (job, limits, needs) as data, no
  generated code; `tests/customer_antspawn.rs` loads real specs and refuses bad ones. Gap found: `[[mount]]`,
  `[[output]]` (arrays of tables) and `job.env` (a map) cannot be expressed yet and are refused as unknown keys.

## 0.2.2

- Front-end support in the core: `configurant::edit::Session` stages edits to a loaded `Config`. Each edit
  meets the rules a load does (type, range, choices, provider module); a secret is refused
  (`SecretInForbiddenSource`, new `Origin::Edit`); staging the value already in force is not a change.
  `Session::review()` shows the changes, the keys that need a restart and the modules' `Plan`; only
  `Review::confirm()` yields a `Confirmed`, the sole source of new file text (`file_text`, via `toml_edit`:
  comments and layout kept) and of the plan to execute. Nothing here writes a file or runs a plan.
- New crate `configurant-tui` (terminal front end, `ratatui` 0.30.2): `configurant-tui <schema> --file <f>`.
  Edit, review, `y` to confirm; saves `<file>.bak` first (refused if it exists). A `DataTransfer` in the plan
  is listed as NOT DONE: running one needs both endpoints, which this front end does not open.
- The repo is now a cargo workspace; CI builds, lints and tests `--workspace`.

## 0.2.1

- PersistAnt module, behind the new cargo feature `persistant` (off by default, so the core stays light;
  `persistant` 0.9.10 `registry`). `configurant::persistant::PersistantModule`: `list_choices` returns the
  backends this build offers for the project's needs (`Needs::required` = capability ids that must all hold,
  `Needs::order` = what matters most), best first, each with PersistAnt's reasons; `try_list_choices` says why
  when the list is empty; `validate` refuses a backend id nobody offers (with a suggestion) or, if the
  project's needs are configured, one that no longer meets them; `on_change` returns a `Plan` holding a
  `DataTransfer` between the old and new backend ids. The module owns no backend list and never opens a
  backend or copies data. CI now also builds, lints, tests and MSRV-checks with `--all-features`.

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
