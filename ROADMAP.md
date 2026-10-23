# ConfigurAnt roadmap

Rows that come from the advisory CodeRipper check. Baseline: `coderipper sweep --no-config --profile extended
--workspace --all-features` 0.8.19 at main 9a87cf8 (the default `classic` profile reports nothing). One row per
failing rule; none is silenced.

| Rule | Count | Verdict | Row |
|---|---|---|---|
| SUP-002 | 3 (High) | True, but informational | `zbus_names` 4.3.5 (RUSTSEC-2026-0371), `zvariant` 5.15.1 (-0370) and `zvariant_derive` 5.15.1 (-0372) are advisories of kind *unmaintained*, not vulnerabilities. One root: `eframe` 0.37 -> `egui-winit` -> `accesskit_winit` -> `accesskit_unix` (Linux AT-SPI), reached only by `configurant-gui`. Not fixable here: when `eframe`/`accesskit` move off zbus, `cargo update`; until then re-check on each dependency bump. |
| SEC-006 | 3 (High) | Same three advisories | The finding is that the advisory has no severity score (an *unmaintained* notice never has one). It clears with the SUP-002 row; nothing separate to do. |
| licence | 1 (Medium) | True | `LICENSE-MIT` reads "Copyright (c) 2026 CireSnave", a person's handle. Which holder name belongs there is CireSnave's decision (`CIRESNAVE-DECISIONS.md`); then change the line in `LICENSE-MIT`. |
| lint-suppression | 11 (Low) | True | `#[allow]` with no `reason`: `crates/configurant-gui/src/lib.rs:485`, `crates/configurant-tui/src/lib.rs:514` (`needless_range_loop`, index three arrays together), `crates/configurant-gui/src/main.rs:230` (`unused_mut`, only `--features scan` mutates), and eight `dead_code` on serde structs in `tests/known_bad.rs:488-534` (fields are read only by `Debug`/the error text). Each already has a comment, not a `reason = "..."`; the fix is to add the reason, or to restructure the loop and the `mut` so the allow goes. |

The PM's note of "+2 unclassified" did not reproduce: this run lists exactly the 11 lint-suppression findings above.
