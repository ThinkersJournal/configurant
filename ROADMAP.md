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

| # | Row | Source | State |
|---|---|---|---|
| 1 | Prior-art review with adopt/differ verdicts, sources, licences | spec "Prior art" | done: `docs/PRIOR-ART.md` (#34) |
| 2 | Synapse (`synapsed`) schema: storage id + endpoint, as `examples/synapsed.toml` + test; then hand the adoption PR to the synapse lane | spec "First customers" | done: #36 (schema + tests); adoption held by the synapse lane |
| 3 | Secret sources slice 3: list the slices after 2 and build the first that needs no SecretAnt | `SECRET-SOURCES.md` s4 | docs in #35; `file` source built in 0.2.34 (CireSnave 2026-10-25 "On decision 277: A"); SecretAnt adapter held |
| 4 | SecretAnt adapter crate (`SecretSource` for SecretAnt) | `SECRET-SOURCES.md` s4 item 3 | held: SecretAnt is not on crates.io |
| 5 | `Needs` schema field (today `--require`/`--prefer`/`--min-durability` flags) | `TRANSFER-UI.md` | open, needs a PM call on the field's shape |
| 6 | `configurant transfer` command | `TRANSFER.md` s4 "only if wanted" | held: nobody has asked for it |
| 7 | Transfer progress callback (a spinner only today) | `TRANSFER-UI.md` q4 | held: PersistAnt has no progress callback |
| 8 | Check the GUI with a real screen reader (only the accessibility tree is tested) | `GUI.md` | held: needs a person with a screen reader (CireSnave) |
| 9 | GUI arrow-key navigation inside the value list | `GUI.md` "does not give" | open, low |
| 10 | Scan assist for Python/TypeScript (Rust only so far) | `SCAN.md` q2 | held: not wanted yet (PM 2026-10-23: Rust only first) |
| 11 | Other modules: ports, paths, logging | spec s4 "Other modules" | open, no design yet |
| 12 | Further customers: lanes' tooling, direct-backend projects through PersistAnt | spec "First customers" | held: each project's own lane adopts |
| 13 | Export a schema as JSON Schema for editors | `PRIOR-ART.md` (schemars) | held: not scheduled, needs a CireSnave call |
| 14 | Env/CLI list items containing a comma | HANDOFF "Known gaps" | open: needs a quoting rule decision |
| 15 | Module validation for schema defaults | HANDOFF "Known gaps" | open: decide whether defaults should reach the module |
| 16 | `ValueDef.min/max` as f64 (ints above 2^53) | HANDOFF "Known gaps" | open, low |
| 17 | Publish 0.2.26 onward (crates.io has 0.2.25) | PM, 2026-10-24 | held: PM publishes a batch when a slice is releasable |
| 18 | Re-check the CodeRipper waiver on every `eframe`/`accesskit` bump | this file | standing |

Rows needing a CireSnave call: 8 (a person), 13. Rows needing a PM call: 5, 14, 15.
