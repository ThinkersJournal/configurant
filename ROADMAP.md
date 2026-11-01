# ConfigurAnt roadmap

Rows that come from the advisory CodeRipper check. Baseline: `coderipper sweep --no-config --profile extended
--workspace --all-features` 0.8.19 at main 9a87cf8 (the default `classic` profile reports nothing). One row per
failing rule; the only silencing is a waiver with its reason in `.coderipper.toml`.

| Rule | Count | Verdict | Row |
|---|---|---|---|


Waived 2026-10-24 (PM ruling): SUP-002 and SEC-006 for RUSTSEC-2026-0370, -0371 and -0372 (`zvariant`, `zbus_names`, `zvariant_derive`: unmaintained, no vulnerability, reached only by `configurant-gui` through `eframe` -> `accesskit_unix`), recorded in `.coderipper.toml`. Re-check on every `eframe`/`accesskit` bump. `cargo deny` does not list them, so `deny.toml` needs no ignore.

## Work named in the docs, audited against them 2026-10-24

Docs read: 10 (`README.md`, `docs/DESIGN.md`, `GUI.md`, `PRIOR-ART.md`, `SCAN.md`, `SECRET-SOURCES.md`, `TRANSFER.md`,
`TRANSFER-UI.md`, `C:\Projects\CONFIGURANT-SPEC.md`, the HANDOFF "Known gaps"). State: **open** (nobody holds it),
**held** (by whom), **done** (PR). Stale statements in five docs were corrected in the PR that added this table.

| # | Row | Source | State | Blocked on |
|---|---|---|---|---|
| 1 | Prior-art review with adopt/differ verdicts, sources, licences | spec "Prior art" | done: `docs/PRIOR-ART.md` (#34) | - |
| 2 | Synapse (`synapsed`) schema: storage id + endpoint, as `examples/synapsed.toml` + test; then hand the adoption PR to the synapse lane | spec "First customers" | done: #36 (schema + tests); adoption held by the synapse lane | - |
| 3 | Secret sources slice 3: list the slices after 2 and build the first that needs no SecretAnt | `SECRET-SOURCES.md` s4 | docs in #35; `file` source built in 0.2.34 (CireSnave 2026-10-25 "On decision 277: A"); SecretAnt adapter held | SecretAnt adapter (row 4) |
| 4 | SecretAnt adapter crate (`SecretSource` for SecretAnt) | `SECRET-SOURCES.md` s4 item 3 | held: SecretAnt is not on crates.io | SecretAnt lane: the crate is a 0.0.0 placeholder on crates.io |
| 5 | `Needs` schema field (today `--require`/`--prefer`/`--min-durability` flags) | `TRANSFER-UI.md` | done: ruled flags only, no `Needs` schema field (PM 2026-10-25) | - |
| 6 | `configurant transfer` command | `TRANSFER.md` s4 "only if wanted" | held: nobody has asked for it | a request from a consumer or CireSnave (nobody has asked) |
| 7 | Transfer progress callback (a spinner only today) | `TRANSFER-UI.md` q4 | done (0.6.7 library, 0.6.8 TUI and GUI show "copying: N of M records" next to the spinner) | - |
| 8 | Check the GUI with a real screen reader (only the accessibility tree is tested) | `GUI.md` | held: needs a person with a screen reader (CireSnave) | CireSnave: a person with a screen reader (no board item yet) |
| 9 | GUI arrow-key navigation inside the value list | `GUI.md` "does not give" | done: arrows move the focused row, the detail follows (0.5.1) | - |
| 10 | Scan assist for Python/TypeScript (Rust only so far) | `SCAN.md` q2 | held: not wanted yet (PM 2026-10-23: Rust only first) | PM ruling 2026-10-23 (Rust only first); reopens on a consumer request |
| 11 | Other modules: ports, paths, logging | spec s4 "Other modules" | done: design `docs/MODULES.md` (#63, 0.5.24); ports P1-P4 (#64, 0.5.25), interface v2 + P5 (#66, 0.5.27), logging L1-L3 (#68, 0.5.29), paths T1-T5 (#70, 0.5.31), patent re-sweep (#72); `synapsed` adopts ports in its own PR (the Synapse lane said yes 2026-10-30) | - |
| 12 | Further customers: lanes' tooling, direct-backend projects through PersistAnt | spec "First customers" | held: each project's own lane adopts | each consumer's own lane: synapse (`synapsed` adopting `ports`, said yes 2026-10-30), AntSpawn |
| 13 | Export a schema as JSON Schema for editors | `PRIOR-ART.md` (schemars) | held: not scheduled, needs a CireSnave call | CireSnave call (no board item yet; the PM puts it on the decision board if wanted) |
| 14 | Env/CLI list items containing a comma | HANDOFF "Known gaps" | done: backslash escape `\,` (PM 2026-10-25), 0.3.0 | - |
| 15 | Module validation for schema defaults | HANDOFF "Known gaps" | done: defaults checked by a registered module at load (PM 2026-10-25), 0.4.0 | - |
| 16 | `ValueDef.min/max` as f64 (ints above 2^53) | HANDOFF "Known gaps" | done: `Bound` (int or float), exact int comparison, 0.5.0 | - |
| 17 | Publish 0.2.26 onward (crates.io has 0.2.25) | PM, 2026-10-24 | done: 0.5.33 published 2026-10-30 (read back on crates.io, tag `v0.5.33`); the PM publishes each later batch | - |
| 18 | Re-check the CodeRipper waiver on every `eframe`/`accesskit` bump | this file | standing | nothing: standing, acts on the next `eframe`/`accesskit` bump |
| 19 | Patent sweep of the 0.5.x line; re-sweep when a new family of capability lands and by 2027-10-25 | `PATENT-SWEEP-PROCESS.md` (CireSnave 2026-10-25) | done: `docs/PATENT-SWEEP.md` (11 hits claim-read, all clear); re-swept 2026-10-29 for rows 20-26 (addendum, 14 hits read, all outside as read); re-swept 2026-10-30 for row 28 (6 hits) and row 11 (ports, logging, paths: 6 hits read, all outside as read, none needing review); re-swept 2026-10-31 for row 29 (browser wizard: 9 hits read, all outside as read, none needing review; closest US8863248B2); re-sweep standing, next for a new capability family | - |
| 20 | Plain-language prompts: a question, an example and a hint per value, rendered by every front end | PM wave 0 (2026-10-28) | done: prompt and example (0.5.7, #46), hint (0.5.9) | - |
| 21 | Serialisable view-model: `Session` fields, review, plan and an event set (set, unset, next, back, confirm) as serde data, secrets still `<redacted>` | PM wave 0; platform-fit finding (PM 2026-10-28) | done: `configurant::view` (`Driver`, `View`, `Event`, `Reply`), 0.5.10; `next`/`back` events arrive with the step engine, row 26 | - |
| 22 | Library options as a schema fragment (`Schema::parse_with`, mounts) | `docs/SCHEMA-FRAGMENTS.md` | done: `Schema::parse_with` and `Mount` (0.5.11) | - |
| 23 | Projection of settings into a form: sections, order and visibility derived from the schema | PM wave 0 | done: `View::sections`, `file_only`, schema `section` and `advanced` (0.5.12) | - |
| 24 | WebHostAnt options shipped as a fragment | `docs/SCHEMA-FRAGMENTS.md` | held: WebHostAnt lane, needs 22 | WebHostAnt lane (row 22, which it needs, is done) |
| 25 | SecretAnt connector (`SecretSource` adapter crate); same as row 4 | `SECRET-SOURCES.md` s4 | held: S, SecretAnt is a 0.0.0 placeholder on crates.io | SecretAnt lane (same as row 4) |
| 26 | Step engine: ordered steps, conditions, back/next, resume; one engine for TUI, GUI and browser | PM wave 0 | done: L, needs 20, 21, 23 (all done); design in `docs/STEP-ENGINE.md`, two PRs: (a) schema `[[step]]` + ordering done (0.5.14); (b) `Wizard` + events + `View.wizard` + resume done (0.5.15); renderers (TUI, GUI, browser) adopt it in their own rows | - |
| 27 | Hand redaction to RedactAnt and delete our copy (`src/config.rs` `REDACTED` and the substring redaction) | ant-fit finding | held: S-M, RedactAnt does not exist yet | RedactAnt lane: the crate does not exist yet |
| 28 | Plan executor with confirmation (dirs, files, commands) for installers | ant-fit finding | done: L, design `docs/PLAN-EXECUTOR.md` (#56, 0.5.17); (a) types, validation, hash, inert `describe` (#58, 0.5.19), (b) run + state + resume (#59, 0.5.20), (c) undo + services (#60, 0.5.21); patent addendum (#61); needs 26 (done) | - |
| 29 | Browser wizard renderer | platform-fit finding (PM 2026-10-28) | done: L, needed 21, 26 and HTTPAnt (published, 0.5.19); design note `docs/BROWSER-WIZARD.md` (loopback, server-rendered pages, new crate `configurant-web`; WebHostAnt does not fit); HTTPAnt 0.5.19 published; slice (a) built (0.5.41): crate, actor, pages, safety rules; (b) JSON endpoint built (0.5.42); (c) example, README and fresh-eyes trial built (0.5.43); test fix (0.5.46); `configurant-web` 0.5.46 published to crates.io by the PM 2026-10-31 (the other five crates at 0.5.43); registry flip waits for one outside use (CireSnave); follow-up: row 32 | - |
| 30 | Easy-gate: README hello-world, examples compiled and run in CI, fresh-eyes trial before a first or breaking release; hub card `easyants-card.toml` | EXPECTATIONS 6.24 (CireSnave 2026-10-28) | done: README, example and CI run (0.5.8); trial of published 0.5.16 passed and the card is `ready` (0.5.18); README drift test added | - |
| 31 | Fragment migrations: a fragment carries its own `[[migration]]` steps and `config_version` under the mount | `docs/SCHEMA-FRAGMENTS.md` item 5 | open: M, no design beyond the doc; not needed until a library renames an option | no demand, not a technical blocker: no library has renamed an option; the PM says whether to build ahead |
| 32 | Browser wizard: no cookie for another port to receive (a session path `/s/<session>/` instead of the `cw` cookie) | `docs/BROWSER-WIZARD.md` "Known limit" (pm-assist read of #80) | done (0.6.0): session path, no cookie; residual (the address in the browser history) written in `docs/BROWSER-WIZARD.md`; publish with the other crates | - |

Rows needing a CireSnave call: 8 (a person), 13. Rows needing a PM call: none open (5 ruled: flags only; 15 ruled: validate defaults through the module).
