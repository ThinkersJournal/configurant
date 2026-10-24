# Changelog

## 0.2.33

- Docs only: `docs/SECRET-SOURCES.md` lists the slices after 2 (3 `file` source, proposed; 4 SecretAnt adapter, held; 5 publish), the rules a `file` source must meet, what is not planned and why, and the open decision whether to build it. All crates move to 0.2.33.

## 0.2.32

- Second customer schema: `examples/synapsed.toml` (the Synapse daemon: home, listen address, mailbox store id + endpoint, launch and alert roles; environment names as `synapsed` reads them at origin/main) with `tests/customer_synapsed.rs` (7 tests; the module-absent refusal was shown to bite by removing `provider`). Tests and an example only; no library change. All crates move to 0.2.32.

## 0.2.31

- Docs only: `docs/PRIOR-ART.md` rewritten as a reviewed table (adopt / differ, source, licence, how each was checked;
  corrects the note that figment has no provenance report: it has `find_metadata` and error origins). `ROADMAP.md` gains
  the doc-to-roadmap audit (18 rows). Stale "later / not built" statements corrected in `README.md`, `DESIGN.md`,
  `GUI.md`, `TRANSFER.md`, `TRANSFER-UI.md`. All crates move to 0.2.31.

## 0.2.30

- CI only: the advisory `portfolio` job is repinned from `d14a22b6` to coderipper `ee9c3950` (0.9.24). `CLAUDE.md` no longer
  says lanes cannot push. No behaviour change. All crates move to 0.2.30.

## 0.2.29

- CI only: the advisory `portfolio` job is repinned from `30e012b7` to coderipper `d14a22b6` (installs coderipper 0.9.14; the
  old pin installs 0.9.6). No behaviour change. All crates move to 0.2.29.

## 0.2.28

- CI only: the advisory `portfolio` job is repinned from a48fc6cc to coderipper `30e012b7`. No behaviour change. All crates
  move to 0.2.28.

## 0.2.27

- Lint suppressions: all 11 `#[allow]` now carry a `reason = "..."` (3 in source, 8 `dead_code` in `tests/known_bad.rs`),
  clearing the CodeRipper `lint-suppression` row from `ROADMAP.md`. New `.coderipper.toml` waives SUP-002 and SEC-006 for
  RUSTSEC-2026-0370, -0371 and -0372 (unmaintained `zvariant`, `zbus_names`, `zvariant_derive`, reached only by
  `configurant-gui` through `eframe` -> `accesskit_unix`; PM ruling), each with its reason; the sweep reports no findings and
  6 waived. No behaviour change. All crates move to 0.2.27.

## 0.2.26

- Licence: the `LICENSE-MIT` holder line in the root and in every crate now reads "Copyright (c) 2026 The
  configurant contributors" (was a person's handle; CireSnave's ruling on board 263). Thinker's Journal Inc. waits
  for confirmed company ownership. The CodeRipper `licence` row leaves `ROADMAP.md`. All crates move to 0.2.26.

## 0.2.25

- Secret sources, slice 2: a secret's `source` reference (never the value) is shown where the secret is described.
  `describe`/`help` adds `[source: vault:ENTRY]`; `Explanation::source` and its text add the same; `edit::Field::source`
  carries it, and the TUI and GUI show it and say a secret comes only from its environment variable or its secret
  source; `configurant check` lists each secret that names a source and says this binary has none built in. All
  crates move to 0.2.25.

## 0.2.24

- Secret sources, slice 1 (design: `docs/SECRET-SOURCES.md`). New crate `configurant-secrets`: the `SecretSource`
  trait, `Sources`, `SecretValue` (wiped on drop, shows nothing), `resolve` (every failure reported, a bounded
  wait, a key the environment sets is not fetched) and `Resolved::into_loader`. Core: a `secret` value may declare
  `source = "<source>:<reference>"` (`SchemaError::SourceOnNonSecret`, `BadSource`), `Loader::resolved`,
  `Layer::Source`, `ORDER_WITH_SOURCES` (default < file < source < environment < command line; `DEFAULT_ORDER` is
  unchanged and has no source layer) and `Origin::Source`. No real source ships; a file, the command line and a
  default still cannot carry a secret. CI: the advisory portfolio job moves to coderipper a48fc6c. All crates
  move to 0.2.24.

## 0.2.23

- Docs only: `docs/SECRET-SOURCES.md`, the design for secret-source modules (what a source is, how it composes with
  the vault, SecretAnt and PersistAnt, ten rules each with a negative control, the four decisions taken); the
  README states the key check's known limits, including a key read through a helper function. All crates move to
  0.2.23.

## 0.2.22

- The compile-time key check: `configurant::keycheck` (`check_source`, `check_dir`, `Report`, `UnknownKey`) and the
  `configurant keys <schema> <dir>` subcommand find Rust code that asks for a key the schema does not define
  (`.get::<T>("k")`, `.value("k")`, `.rows("k")`, `.explain("k")`), with file, line and a did-you-mean. It reads
  source text and writes nothing; computed names are counted, not guessed; comments and `target/` are skipped;
  exit 1 on an unknown key. Usable from a `build.rs` (call `check_dir`, panic on `!report.unknown.is_empty()`).
  All crates move to 0.2.22.

## 0.2.21

- CI: the advisory `portfolio` job moves to coderipper commit 789b012 (Windows runner bash fix). `ROADMAP.md`: the
  SUP-002 row names itself a waiver candidate. No code change. All crates move to 0.2.21.

## 0.2.20

- `ROADMAP.md`: one row per rule the advisory CodeRipper check reports (SUP-002, SEC-006, licence, lint-suppression),
  each checked. No code change. All crates move to 0.2.20.

## 0.2.19

- CI: an advisory `portfolio` job (the CodeRipper portfolio check, pinned by commit). It is not a required check.
  No code change. All crates move to 0.2.19.

## 0.2.18

- Migrations gain a lossless `drop`: `{ op = "drop", key = "legacy.mode", value = "compat" }` removes the key only
  while it holds exactly that string (empty parent tables go too, comments elsewhere survive), refuses any other
  value or a non-string with a named error that never shows the value (`MigrateError::DropMismatch`, `WrongType`),
  and carrying back writes `legacy.mode = "compat"` again (refused if the key holds something else). A file that
  never had the key gains it when carried back. New `Op::Drop` (the enum is `non_exhaustive`). All crates move
  to 0.2.18.

## 0.2.17

- `configurant-gui` "Add from scan" (scan assist slice 3, GUI), behind the Cargo feature `scan` (off by default).
  Build with `--features scan` and run `configurant-gui <schema> --file <config> --scan <project dir>`
  (`--scan-server`, `--scan-wait`): the "Add from scan" button runs the scan on a worker thread, lists the
  proposals as checkboxes with a PARTIAL banner when the language server could not resolve every call, "Review the
  ticked entries" shows what will be added, and "Write the schema" saves `<schema>.bak` and rewrites the schema.
  A failed scan is shown and changes nothing; a secret is proposed with no default and none is ever drawn; the
  button is refused while edits are staged. The session keeps the old schema, so it asks for a restart to edit
  the new values. Library: `App::with_scan`, `App::scanned`, `App::schema_written`, `Effect::Scan`,
  `Effect::WriteSchema`, `ScanSetup`, `ScanJob`. All crates move to 0.2.17.

## 0.2.16

- Dependencies to latest: persistant 0.9.10 to 0.9.28 (all three manifests), plus `cargo update` of the lockfile
  (unisql 0.25, uuid 1.29, icu 2.4, cc 1.10 and others). No code change. `generic-array` and `pest*` stay behind
  latest because other crates pin them. All crates move to 0.2.16.

## 0.2.15

- `configurant-tui` "Add from scan" (scan assist slice 3, TUI first), behind the new Cargo feature `scan`
  (off by default; the GUI follows). Build with `--features scan` and run
  `configurant-tui <schema> --file <config> --scan <project dir>` (`--scan-server`, `--scan-wait`): key `a` runs
  the scan on a worker thread, lists the proposals with a PARTIAL banner when the language server could not
  resolve every call, Space ticks, Enter shows the entries to be added, and `y` writes the schema after saving
  `<schema>.bak`. A failed scan is shown and changes nothing; a secret is proposed with no default and none is
  ever drawn; the key is refused while edits are staged. The session keeps the old schema, so it asks for a
  restart to edit the new values. Library: `App::with_scan`, `App::scanned`, `App::schema_written`,
  `Effect::Scan`, `Effect::WriteSchema`, `ScanSetup`, `ScanJob`. All crates move to 0.2.15.

## 0.2.14

- `configurant-scan --server-arg ARG` (repeatable) passes arguments to the language server command. The
  binary's exit code 2 (PARTIAL) and its server-start error are now tested against a fake stdio language server
  (`examples/fake_lsp.rs`, built by `cargo test`/`cargo build --all-targets`), not only the real rust-analyzer.
  All crates move to 0.2.14.

## 0.2.13

- `configurant-scan` binary (scan assist slice 2): `configurant-scan <schema> --root <dir>` lists the
  environment variables a Rust project reads that its schema lacks, with their guessed key, type, literal
  default (never for a secret) and sites, plus reads with a computed name, key clashes and calls the project
  resolves to its own `env::var`. It exits 2 and prints PARTIAL when rust-analyzer could not resolve every call,
  1 on an error (no schema, no server). `--accept ENV` (repeatable) prints the schema with those proposals
  added; with `--write` it saves `<schema>.bak` first. New in the library: `Proposals::report` and
  `Proposals::accept`. It is a separate binary rather than `configurant scan` because the scan crate depends on
  `configurant`, so the core binary cannot depend on it. All crates move to 0.2.13.

## 0.2.12

- New crate `configurant-scan` (slice 1 of scan assist, `docs/SCAN.md`): finds the environment variables a Rust
  project reads (`std::env::var`, `var_os`, `env!`, `option_env!`) and proposes schema entries. Candidates come
  from the source text and rust-analyzer confirms each call resolves to `std::env`; a call it cannot resolve
  marks the result partial, a project-own `env::var` is rejected, a non-literal name is listed by location. A
  name that looks like a secret is proposed `secret` with no default, and the default seen beside it is dropped.
  It suggests only. Needs `lsp-bridge` 0.4 (0.2 built a non-URI `rootUri` on Windows, so rust-analyzer never
  loaded the project).
- Core: `configurant::schema_edit::add_value` appends one `[[value]]` to schema text with `toml_edit` (comments
  and layout survive), refuses a secret with a default, and returns text only when the result parses as a
  schema. Additive. All crates move to 0.2.12. CI installs `rust-analyzer` and `rust-src` for the one real
  rust-analyzer test.

## 0.2.11

- Design note `docs/SCAN.md`: scan assist (LSP) as an optional `configurant-scan` crate on `lsp-bridge`.
  Proposals are data, secrets are never shown, accepting one edits the schema text with `toml_edit`. No
  behaviour change; all three crates move to 0.2.11.

## 0.2.10

- `configurant-gui` runs the data transfer in a plan, the same flow as the TUI. "Write the file" on the review
  asks for the source and target endpoints (a path, `env:NAME` for a URL, nothing for `memory`), copies on a
  worker thread with a spinner, and writes the file only after the copy has read back equal. A failed copy
  writes nothing and keeps the staged edit. "Write without copying..." asks for a second confirmation and the
  result says `NOT COPIED`. New flags: `--require`, `--prefer`, `--min-durability` and `--endpoint ID=ANSWER`
  (a URL is refused and never echoed). `App::with_transfer`, `App::transferred` and `Effect::Transfer` are new
  (additive). All three crates move to 0.2.10.

## 0.2.9

- `configurant-tui` runs the data transfer in a plan. After the review's `y`, if the plan holds one
  `DataTransfer`, it asks for the source and target endpoints (a path, `env:NAME` for a URL, nothing for
  `memory`), copies on a worker thread with a spinner, and writes the file only after the copy has read back
  equal. A failed copy writes nothing and keeps the staged edit. `Ctrl-W` then `y` writes without copying
  (the result says `NOT COPIED`). New flags: `--require`, `--prefer`, `--min-durability` (the project's needs)
  and `--endpoint ID=ANSWER` (prefill; a URL is refused and never echoed).
- Core: `persistant::parse_endpoint` (a typed URL is refused without being echoed; a URL backend takes
  `env:NAME`), `PersistantModule::endpoint_spec`, `TransferJob` (feature `persistant-blocking`), and
  `PersistantModule` is now `Clone` + `Debug`. Additive. The GUI is the next PR. All three crates move to 0.2.9.

## 0.2.8

- Design note `docs/TRANSFER-UI.md`: running a transfer from the TUI and GUI (endpoints asked in the UI, URLs
  only as environment variable names, copy before write, an explicit write-without-copy). Docs only; all three
  crates move to 0.2.8.

## 0.2.7

- `PersistantModule::transfer` executes the `DataTransfer` plan: opens both backends through PersistAnt's
  registry, copies every record with the time to live it has left, then reads both sides back and compares
  them. The source is never changed, a non-empty target is refused (`TransferOptions::allow_existing` keeps
  what is there), a source that cannot report expiry is refused, and endpoint URLs and passwords are scrubbed
  from every error. `copy_between` does the same for two key-value backends you already hold;
  `transfer_blocking` (new feature `persistant-blocking`, Tokio `rt` only) runs it without a runtime of your
  own. Leases, blob and relational data are not copied and the report says so. Additive. All three crates move
  to 0.2.7.

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
