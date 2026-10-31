# Changelog

## 0.5.41

- New crate `configurant-web` (ROADMAP row 29, slice a): the schema wizard as plain HTML pages served on loopback by HTTPAnt 0.5.19, for one visitor and one run. One thread owns the `Wizard`; a form post becomes the library's own events; the file is written after the review is confirmed, `<file>.bak` first. Safety rules, each with a known-bad test shown to bite by mutation: loopback bind only, a one-time launch token swapped for an `HttpOnly; SameSite=Strict` cookie, `Host` and `Origin` checks, a policy header set (no script, no outside resource, no framing), a post-size limit, an idle stop, HTML escaping, and no text box or echo for a secret. `docs/BROWSER-WIZARD.md` records which HTTPAnt claims were verified against 0.5.19. Core: `Wizard::plain` (the value in force as plain text, a secret as `<redacted>`). The JSON endpoint, the example and the fresh-eyes trial are the next slices. All crates move to 0.5.41.

## 0.5.40

- Dependency: `persistant` 0.11.7 to 0.12.3 (crates.io latest; CireSnave's rule that every dependency is at its latest version). 0.12.0 changes the wording of the unmet-need text in `Choice.reason`; no test or doc of ours pins that text, and the full test run passes. Every other direct dependency was already at its crates.io latest. All crates move to 0.5.40.

## 0.5.39

- An unknown field or variant in a schema (or a mounted fragment) now ends the error with `(did you mean `x`?)` when a known name is close, e.g. `secrt` gives `secret`; a far-off name gets none, as before. The text is built in `SchemaError`'s `Display`, so the `Parse` variant is unchanged. All crates move to 0.5.39.

## 0.5.38

- Fix: `edit::Session::stage` now runs the modules' cross-key rules (`Module::validate_all`, interface version 2) over the values that would be in force, as a load does. Before, an editing session accepted a second port that collided with another key's (P5) and the next load refused the file. The refusal names the other key and never a value; a collision with a `secret` key leaks nothing. The loader and the session share one function. All crates move to 0.5.38.

## 0.5.37

- Docs only: `docs/BROWSER-WIZARD.md`, the design note for ROADMAP row 29 (browser renderer: options, recommendation, loopback safety rules, what is needed from HTTPAnt); row 29 points to it. All crates move to 0.5.37.

## 0.5.36

- Docs only: HANDOFF rewritten as current state (the lane is idle; every open row names its blocker). All crates move to 0.5.36.

## 0.5.35

- Docs only: ROADMAP gains a "Blocked on" column (every open row names its blocker: a board item, a lane or a dependency; row 31 says it has none beyond missing demand); rows 17 (0.5.33 published) and 30 (card `ready`) now read done. All crates move to 0.5.35.

## 0.5.34

- Docs only: ROADMAP rows 11 (modules) and 28 (plan executor) now read done with their PR numbers; HANDOFF refreshed after the 0.5.33 publish. All crates move to 0.5.34.

## 0.5.33

- Docs only: `docs/PATENT-SWEEP.md` gains an addendum (2026-10-30) for row 11, the ports, logging and paths modules (0.5.25 to 0.5.31): 6 new ideas, 6 queries, 6 hits whose independent claims were read, all "outside, as read" (closest: US20040019670A1, active to 2029, which needs a server with a second in-memory representation and a change request), 28 found-but-not-read patents listed without a verdict, nothing needing review. ROADMAP row 19 updated. All crates move to 0.5.33.

## 0.5.32

- Docs only: HANDOFF refreshed after the paths module (0.5.31); row 11 is complete. All crates move to 0.5.32.

## 0.5.31

- The `paths` module (ROADMAP row 11, slice 4; `docs/MODULES.md` section 3), behind the new Cargo feature `paths`: `configurant::modules::paths::{Paths, Platform}`, registered under `provider = "paths"` on a `path` or `string`. Lexical only (no file, link or drive is looked at). T1: not empty, no NUL or control character, valid UTF-8. T2: `.within(root)` keeps the normalised path under the root (whole components, `..` resolved, case folded on Windows, relative values taken relative to the root). T3: `.relative()` / `.absolute()`. T4 (Windows rules): reserved device names with or without an extension, trailing dot or space, `< > : " | ? *`, drive-relative `C:x`, `\?\` and `\.\` prefixes, incomplete UNC. T5: `.file()` refuses a trailing separator, `.` and `..`. Rules follow a `Platform` (default: the host's; settable, so Windows rows are tested everywhere). `on_change` offers one `Mkdir` step for a new valid absolute `.directory()` value and never a removal or move. A refusal never quotes the path. 18 tests in `tests/known_bad_paths.rs`; 16 mutations proven to bite (see the PR). All crates move to 0.5.31.

## 0.5.30

- Docs only: HANDOFF refreshed after the logging module (0.5.29); next is `paths`. All crates move to 0.5.30.

## 0.5.29

- The `logging` module (ROADMAP row 11, slice 3; `docs/MODULES.md` section 4), behind the new Cargo feature `logging`: `configurant::modules::logging::{Logging, Level}`, registered under `provider = "logging"` on a `string` or `enum`. L1: a level is `off error warn info debug trace` in any case, a near miss (`inf`, `warning`) gets a `did you mean`. L2: a filter is comma-separated `target=level` directives (a target is a Rust path) plus at most one bare default level; an empty directive, a second default, a bare non-level word and a bad target are refused. L3: `.max_level(Level)` refuses a more verbose level in a bare level and in every directive. `.levels_only()` refuses a filter. `list_choices` returns the levels least to most verbose with a "what you see" reason, cut at the ceiling. The module changes no text and adds no dependency; a refusal quotes the token escaped and cut at 32 characters, and a `secret` key is `<redacted>`. 14 tests in `tests/known_bad_logging.rs`; seven mutations proven to bite (see the PR). All crates move to 0.5.29.

## 0.5.28

- Docs only: HANDOFF refreshed after ports slice 2 (0.5.27). All crates move to 0.5.28.

## 0.5.27

- Ports rule P5 and module interface version 2 (ROADMAP row 11, slice 2). New defaulted trait method `Module::validate_all(&[(&str, &Value)]) -> Vec<(String, String)>`, called once per load per registered module with the winning value of every key that names it; the loader turns each answer into `ConfigError::ModuleRejected` with the winning value's origin, redacted for a `secret` key. `INTERFACE_VERSION` is now 2 and new `MIN_INTERFACE_VERSION` is 1; the default `Module::interface_version()` stays 1 (a recompiled v1 module does not claim v2) and `Ports`, which implements `validate_all`, returns 2: a version 1 module still registers, a version 3 one is refused (new `Modules::names()`). `Ports` implements P5: two keys on the same port with overlapping hosts (same host without case, or a wildcard `0.0.0.0` / `[::]` / a bare int port) are each refused naming the other key, never a value; port 0 never collides. 10 new tests in `tests/known_bad_modules.rs`; two mutations proven to bite (loader call removed: 6 fail; wildcard overlap removed: 2 fail). Not done: `edit::Session` does not run `validate_all` yet. All crates move to 0.5.27.

## 0.5.26

- Docs only: HANDOFF refreshed after the ports module (0.5.25); next is row 11 slice 2 (interface v2, P5). All crates move to 0.5.26.

## 0.5.25

- The `ports` module (ROADMAP row 11, slice 1; `docs/MODULES.md` section 2), behind the new Cargo feature `ports`: `configurant::modules::ports::Ports`, registered under `provider = "ports"`. P1: an `int` port is 1 to 65535 (`0` only with `.allow_zero()`). P2: a `string` is `host:port` or `[v6]:port` with an IP literal or a DNS name (labels of 1 to 63, no leading or trailing `-`, 253 in all); a name of only digits and dots must be a valid IPv4 address. P3: a port below 1024 is refused unless the schema key declares the new optional field `privileged = "<reason>"` (valid only with `provider = "ports"`; empty is a schema error; shown by `help` and `explain`, new `Explanation::privileged`). P4: a refusal names the key and the rule and never quotes the value (a `secret` key's whole reason is `<redacted>` as for every module). `.suggest(&[..])` makes `list_choices` offer ports; it never probes. New defaulted trait method `Module::validate_for(&ValueDef, &Value)` (calls `validate` unless overridden; the interface stays version 1, every existing module is unchanged). 22 tests in `tests/known_bad_modules.rs`; six mutations proven to bite. P5 (two ports must not collide) is slice 2. First customer: `synapsed` `listen.addr` (`127.0.0.1:7920`), adopted by the Synapse lane in a latest-deps bump. All crates move to 0.5.25.

## 0.5.24

- Docs only: `docs/MODULES.md`, the design for ROADMAP row 11 (ports, paths, logging modules): what a project declares, the loader rules with a negative control each, the first customer of each, and slice 1 = ports (P1 to P4, no interface change). Nothing built. All crates move to 0.5.24.

## 0.5.23

- Docs only: HANDOFF refreshed after the 0.5.22 publish. All crates move to 0.5.23.

## 0.5.22

- Docs only: `docs/PATENT-SWEEP.md` gains an addendum (2026-10-30) for row 28, the plan executor (0.5.17 to 0.5.21): 6 new ideas (hash-bound confirmation, secrets by reference, the Windows owner-only rule, resumable state, argv-only with real-path checks, guarded undo), 6 queries, 6 hits whose independent claims were read (some cut off by the tool and marked), all "outside, as read", 16 found-but-not-read patents listed without a verdict, nothing needing review. All crates move to 0.5.22.

## 0.5.21

- Plan executor, part c (ROADMAP row 28): `Executor::undo(confirmed_hash)` reverses the steps that ran, last first, bound to the plan hash, and goes on past one it cannot undo. New `Undo::Remove { path }`: a file is removed only while it still holds exactly what the step wrote (`Left(Failure::Changed)` otherwise), a directory only while it is empty (`NotEmpty`), and only for the path the step itself made (`NotOurs`), so a data store is never dropped; a plan whose undo would remove a declared root is refused (`Problem::IsRoot`). `UndoReport` lists each step as `Undone`, `ForwardOnly`, `NotApplied`, `Left(..)` or `Failed(..)`; `complete()` is true only if every step that ran was reversed. An undone step is pending again, so the plan can run again. Services go through `Host::service` (down for an undo `Action`). `StepAction`, `Check`, `Content` and `Undo` are `#[non_exhaustive]`. 11 tests in `tests/known_bad_exec_undo.rs`; mutations proven to bite (see the PR). SECURITY.md: "Supported versions" now says the latest line gets fixes, an older line only when the backport is small, and no support period is promised (CireSnave, 2026-10-29). All crates move to 0.5.21.

## 0.5.20

- Plan executor, part b (ROADMAP row 28). `Executor::new(plan, host, state_path).run(confirmed_hash, approved_overwrites)` applies a plan: refused unless the hash is the plan's (nothing changes); each step is check, apply, verify; the state file (TOML: plan hash, step ids, `pending|applied|verified|failed`, no values) lets the next run skip verified steps, stop at the first failure and resume there, and a state file for another plan is refused. Paths are checked again on the real filesystem (symlinks and junctions followed) and must stay in the roots; a target that is a symlink is not written through; an existing file with different content is replaced only for a step id the front end approved. `Host` trait: `secret`, `content` (checked against its hash), `owner_only_acl` (Windows: a secret file is written only if the host reports an owner-only ACL, else `Failure::AclRequired`, never downgraded) and `service` (declines by default). A secret goes only to a child environment variable or a file created 0600 on Unix; command output is discarded; a `Failure` carries no value and no host message. Programs run with no shell, no stdin, and are killed at their timeout. `Secret` has a redacted `Debug`. 14 tests in `tests/known_bad_exec_run.rs`; seven mutations proven to bite (hash binding, resume skip, real-path check, overwrite approval, ACL rule, timeout, content hash). The symlink-target test runs on Unix only, so CI proves that one. `describe()` now lists a `Run` step's environment in a separate `env` field instead of appending it to `command`; `Host::content` has a default (`None`). Undo and real service control are part c. All crates move to 0.5.20.

## 0.5.19

- Plan executor, part a (ROADMAP row 28, `docs/PLAN-EXECUTOR.md`). New `configurant::exec`: `Step` (`Mkdir`, `WriteFile` with `Content::{Inline,Hash,Secret}`, `Run` with argv only, `Service`) with `check`, `verify` and `undo`; `ExecPlan::new(roots, steps, secrets)` validates (declared roots, no `..`, unique ids, inline text at most 64 KiB, SHA-256 shape, mode, timeout, names) and refuses a plan that contains any known secret value in a title, path, argv, environment literal, inline content or service name. The error names the step and field, never the value. `ExecPlan::hash()` (SHA-256 of the canonical plan) and an inert `describe()` (create / overwrite / unchanged / conflict, secrets shown as `<redacted ref NAME>`, a note that modes are advisory on Windows and that some steps are forward-only). `ActionKind::Step` carries a step in a module `Plan` and `ExecPlan::from_plan` collects them. Nothing runs yet (parts b and c). New dependency: `sha2`. 16 tests in `tests/known_bad_exec.rs`, five mutations proven to bite; a doctest. All crates move to 0.5.19.

## 0.5.18

- Hub card flipped to `ready` (ROADMAP row 30) after a fresh-eyes trial of the PUBLISHED 0.5.16 crate: the README snippet compiled and ran unchanged. README fixes from that trial: status line said 0.0.x; docs links are absolute (they broke on crates.io); `cargo install configurant` and the sibling crates are named; the schema value fields are listed; the `Settings` snippet says what it is. New test `the_readme_snippet_is_the_example` fails if the README hello-world drifts from `examples/quickstart.rs`. All crates move to 0.5.18.

## 0.5.17

- Docs only: `docs/PLAN-EXECUTOR.md`, the design for ROADMAP row 28. Steps as data (`Mkdir`, `WriteFile`, `Run` with argv only, `Service`) with check, verify and undo; `Executor` with an inert `describe()`, `run(confirmed_hash)` bound to the plan hash with a resumable state file that holds no values, and `undo()`. Three PRs. Nothing is built; three questions are open for the PM.

## 0.5.16

- Docs only: `docs/PATENT-SWEEP.md` gains an addendum (2026-10-29) for rows 20 to 26 (prompts, view-model, schema fragments, form projection, step engine and wizard): 6 new ideas, 8 queries, 14 hits whose independent claims were read, all "outside, as read" (none "clear"), 14 found-but-not-read patents listed without a verdict, nothing needing review. All crates move to 0.5.16.

## 0.5.15

- Step engine, part b (ROADMAP row 26, `docs/STEP-ENGINE.md`). New `configurant::wizard`: `Wizard` wraps a `view::Driver` and walks `Schema::effective_steps()`. New events `next`, `back` and `goto { step }` (a plain `Driver` refuses them); a step whose `when` does not hold is skipped both ways and `goto` only returns to a step already reached; `confirm` works only at the review. `View.wizard` (`WizardView`: the step list as `done`/`current`/`todo`/`hidden`, the current step's title, intro and keys, `can_back`, `can_next`, `at_review`, a one-time `note`). `WizardState` (step ids only, never a value) is serialisable and `Wizard::resume` continues from it; a stale step id starts at the first step with a note. `Session::plain(key)` gives a value as plain text (no quotes; a secret is `<redacted>`) for a `when` to compare. A `required` value is already guaranteed by load, so `next` refuses only at the review (`docs/STEP-ENGINE.md` corrected). 9 tests in `tests/known_bad_wizard.rs` (the visibility check proven to bite by mutation: 5 fail) and a doctest. All crates move to 0.5.15.

## 0.5.14

- Step engine, part a (ROADMAP row 26, `docs/STEP-ENGINE.md`). A schema may declare `[[step]]` entries (`id`, `title`, `intro`, `keys`, optional `when`); `when` is one data test on a value, `equals`, `not_equals` or `set`, read as text. `Schema::effective_steps()` gives the order a wizard walks: the declared steps then a final "More settings" step with every unlisted value, or, with no `[[step]]`, one step per form section. `Step::visible(lookup)` decides a step by data alone. New `Step`, `When` and `SchemaError::{BadStepId, DuplicateStep, EmptyStepTitle, StepUnknownKey, StepKeyTwice, BadWhen}`: a step naming an unknown key or a table, a key in two steps, an empty title, a bad or repeated id, and a `when` that reads an unknown key or makes zero or several tests are refused. 9 tests in `tests/known_bad_steps.rs` (the `not_equals` test proven to bite by mutation). Part b (`Wizard`, `next`/`back`/`goto`, `View.wizard`, resume) is next. All crates move to 0.5.14.

## 0.5.13

- Docs only: `docs/STEP-ENGINE.md`, the design for the step engine (ROADMAP row 26): `[[step]]` in the schema, `when` as data, `next`/`back`/`goto` events, `View.wizard`, resume by step id, one step per section when none are declared. Nothing built. All crates move to 0.5.13.
## 0.5.12

- Form projection (ROADMAP row 23). `View` now groups the fields for a form: `sections` (title and keys, in order of first appearance, schema order inside) and `file_only` (the tables and maps only a configuration file can set, with their descriptions, so a form says where to edit them). New schema keys on a `[[value]]`: `section` (the title; default the key's first segment, or `General`) and `advanced` (a form may fold it away); an empty `section` is refused (`SchemaError::EmptySection`) and a fragment override may not set either. `edit::Field` and `view::FieldView` carry `section` and `advanced`; `Session::file_only()`. The grouping carries keys only, never a value. 6 tests in `tests/known_bad_form.rs` (the section override proven to bite by mutation). All crates move to 0.5.12.
## 0.5.11

- Schema fragments (ROADMAP row 22, `docs/SCHEMA-FRAGMENTS.md`). `Schema::parse_with(text, &[Mount::new(name, fragment)])` adds a library's options under the mount name: a fragment is a schema with `[fragment]` (name, version, summary, schema_version) instead of `[project]`, and its `[[value]]`, `[[table]]` and `[[map]]` keys are written relative to the mount. A declared `env` becomes `<env_prefix><MOUNT>_<ENV>` and a `flag` becomes `--<mount>-<flag>`. The application may restate a mounted value to change its default and wording only (`SchemaError::MountOverride` otherwise); a mounted secret stays secret. New `Mount`, `SchemaError::{DuplicateMount, FragmentParse, MountOverride}`; `Schema::parse` is `parse_with(text, &[])`. A fragment with migrations is refused for now (new ROADMAP row 31). 13 tests in `tests/known_bad_fragments.rs` (two mutations proven to bite). All crates move to 0.5.11.
## 0.5.10

- Serialisable view-model (ROADMAP row 21): new `configurant::view`. `Driver` owns an `edit::Session`; `Driver::view()` returns `View` (every field with prompt, example, hint, choices, current and staged value, plus the review: changes, restart keys, plan actions) as plain `Serialize` data, and `Driver::apply(Event)` takes `Event::{Set, Unset, Confirm}` (`Deserialize`, JSON `{"event":"set","key":..,"text":..}`) and returns a `Reply` (new view, `refused` with the named error, `confirmed`). A secret stays `<redacted>` and cannot be set; a refused event changes nothing; an empty review cannot be confirmed; `take_confirmed()` hands the host the `Confirmed` once, to write and execute (the view-model writes nothing). No new runtime dependency (serde was already one); `serde_json` is a dev-dependency. 8 tests in `tests/known_bad_view.rs` (the confirm guard proven to bite by mutation). All crates move to 0.5.10.

## 0.5.9

- Row 20 finished: a `[[value]]` may also carry `hint`, one short sentence of help for the person answering (where to find the value, what to avoid). `edit::Field::hint`; `help` shows `[hint: ...]`; the TUI detail line and the GUI detail and edit panels show it. An empty hint is refused (`SchemaError::EmptyHint`). 2 more tests in `tests/known_bad_prompts.rs`. All crates move to 0.5.9.

## 0.5.8

- Easy-gate (EXPECTATIONS 6.24): `README.md` opens with a 10-line hello-world, which is `examples/quickstart.rs` and is run by `tests/quickstart_example.rs`; `easyants-card.toml` is the hub card read by ThinkersJournal/easyants, `not-ready` ("Early release, not ready") until the fresh-eyes trial has run and the example is checked against the published crate. All crates move to 0.5.8.

## 0.5.7

- Plain-language prompts (ROADMAP row 20). A `[[value]]` may carry `prompt` (the question a wizard asks) and `example` (a sample as it would be typed). `ValueDef::question()` gives the prompt, else the description; `edit::Field` has `prompt` and `example`; `help` shows `[example: ...]`; the TUI detail line and the GUI detail and edit panels show the prompt and the example. An example must pass the same checks as a default (type, min/max, choices: `SchemaError::BadExample`), a secret may not have one (`SecretWithExample`, and the error does not echo it), and an empty prompt is refused (`EmptyPrompt`). 7 tests in `tests/known_bad_prompts.rs`. All crates move to 0.5.7.

## 0.5.6

- Docs only: `docs/SCHEMA-FRAGMENTS.md`, the design for a library shipping its own options as a feature-gated schema fragment that an application mounts (proposed, nothing built), and ROADMAP rows 20-29 in the order the PM's wave 0 asked for (prompts and view-model first, then fragments, form projection, step engine). All crates move to 0.5.6.

## 0.5.5

- Dependencies: `persistant` 0.9.28 -> 0.11.7 in the core crate, `configurant-tui` and `configurant-gui` (CireSnave: all dependencies on their most recent versions); `unisql` follows to 0.27.10 and `capant` 0.0.7 joins the lock. No source change was needed; the workspace builds and all tests pass with `--all-features`. All crates move to 0.5.5.

## 0.5.4

- Docs only: ROADMAP row 5 (`Needs` schema field) shows its ruling, flags only (PM 2026-10-25); it still read "open, needs a PM call".

## 0.5.3

- GUI: Up and Down move between the value rows (egui's focus navigation already did; the selection and the detail pane below did not follow it, because the row selected itself only on the frame it gained focus). The focused row is now the selected row, so the detail pane shows the value the arrows landed on. Down from the last row goes on to the controls below, as Tab does (`docs/GUI.md` says so). 1 new test in `crates/configurant-gui/tests/app.rs` (red before the fix at the detail-pane assertion). All crates move to 0.5.1.

## 0.5.2

- Docs only: `docs/PATENT-SWEEP.md`, the due-diligence patent sweep of the 0.5.x line at `ad5700c` (14 ideas, 14 queries on Google Patents, 11 hits whose independent claims were read, all `clear`; none needs review; not legal advice). `ROADMAP.md` row 19 keeps the re-sweep standing. All crates move to 0.5.2.

## 0.5.0

- **Breaking** (public types, second number moves pre-1.0): `ValueDef::min` and `max` are `Option<Bound>` (was `Option<f64>`), and `ConfigError::OutOfRange`'s `min` and `max` are `Option<Bound>` too. `Bound` is new and exported: `Int(i64)` for a bound written as an integer, `Float(f64)` otherwise; it implements `Display`. An `int` value is now compared exactly: before, a bound or value above 2^53 was rounded to f64, so `min = 9007199254740993` accepted `9007199254740992` and `max = 9007199254740992` accepted `9007199254740993`; `min > max` is also exact. A float bound on an `int` still works (`min = 1.5` refuses 1, accepts 2). Schema files need no change; only Rust code that reads `ValueDef::min`/`max` or matches `OutOfRange`'s bounds does (migration: match on `Bound`, or print it). 5 tests in `tests/known_bad_int_bounds.rs`, 4 of them red on 0.4.0. All crates move to 0.5.0.

## 0.4.0

- **0.3.0 was never published to crates.io**: this release is the first on crates.io after 0.2.34 and carries the 0.3.0 Breaking change below as well (backslash escapes in env/CLI list text). Read the 0.3.0 entry too when upgrading from 0.2.x.
- **Breaking** (the second number moves, pre-1.0; PM ruling 2026-10-25): a schema default is now checked by its value's module at load, like a value from any other layer. A default that its registered module rejects fails the load with `ConfigError::ModuleRejected` naming the key (origin `Default`), even when a higher layer overrides it. Schemas that loaded before can now fail, and only these: a value with a `provider` whose schema `default` that provider's module rejects, loaded with that module registered. With no module registered a default still loads unchecked, as before (a front end that offers only some modules is unaffected). Migration: fix the default, or the module's `validate`. 3 tests in `tests/known_bad_default_module.rs`; skipping the check makes 2 of them fail. All crates move to 0.4.0.

## 0.3.0

- **Breaking** (the second number moves, pre-1.0; PM 2026-10-25): list text from the environment or the command line reads backslashes differently. Inputs that change, and only these: a value containing `\,` (was a backslash ending one item, then a split; now a literal comma inside the item) and a value containing `\\` (was two backslashes; now one). A lone backslash anywhere else, such as `C:\data`, is unchanged. Migration: to keep a literal backslash before a split, write `\\,`; to keep two literal backslashes, write `\\\\`. Files (TOML arrays) are unaffected.
- A list value from the environment or the command line can hold a comma: `\,` is a literal comma and `\\` a literal backslash (so `a\\,b` is `a\` and `b`). Any other backslash stays as written, so `C:\data,D:\more` is unchanged; only a backslash directly before a comma, or doubled, reads differently than before (ROADMAP row 14, PM ruling 2026-10-25). Files are unaffected (TOML arrays). 5 tests in `tests/known_bad_list_text.rs`, the escape arm shown to bite by narrowing it. All crates move to 0.3.0.

## 0.2.34

- `configurant-secrets` gains the `file` source, `FileSource` (`source = "file:<name>"`; CireSnave 2026-10-25, PM board 277): the directory comes from `CONFIGURANT_SECRETS_DIR` (`FileSource::from_env`), the reference must be a bare file name, at most 64 KiB of UTF-8 with one trailing newline removed, a symlink is followed only if it stays inside the directory, and on Unix a file with any group or other permission bit is refused. Windows has no permission check (stated in `docs/SECRET-SOURCES.md` section 4a). No path, content or OS message reaches an error.
- `SecretSource::check_reference` (default accepts) and `ResolveError::BadReference`: a malformed reference is refused by `resolve` before any read, even when the environment sets the key. `SourceFailure::Invalid` (too large, not text). Both enums are `#[non_exhaustive]`, so this is not breaking.
- 9 new tests (7 run on Windows; the permission and symlink tests run on Unix) in `crates/configurant-secrets/tests/file_source.rs` (traversal, oversize, bad permissions, symlink out of the directory, missing file, UTF-8, the canary in no error, `explain` and `Debug`). All crates move to 0.2.34.

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
