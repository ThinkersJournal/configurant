# configurant HANDOFF (rewritten 2026-10-30 by the configurant lane after the paths module merged, 0.5.32)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`; `send_message` takes `to_id`) and the
pm-assist lane (merge notices). Read `C:\Projects\CLAUDE.md` and `CLAUDE.md`. Run on Sonnet, never Opus. You CAN
push your own branches: push, open the PR, send [READY]. One PR at a time, not stacked; version bump + CHANGELOG
entry in each (breaking = second number pre-1.0).

## State (main = 0.5.31 at c864714; crates.io has 0.5.22; the PM publishes)
- Done and merged: wave 0 (rows 20-23), step engine (26), patent re-sweep (19), plan executor a/b/c (row 28), hub card
  `ready` (#57), row 11 design (#63), ports slice 1 (#64, 0.5.25), **ports slice 2 (#66, 0.5.27)**: interface version 2
  (`Module::validate_all`, `MIN_INTERFACE_VERSION` 1; default `interface_version()` stays 1, `Ports` returns 2) and
  rule P5. Known gap: `edit::Session` does not run `validate_all`.
- **Logging module (row 11 slice 3, #68, 0.5.29) merged**: `src/modules/logging.rs`, `tests/known_bad_logging.rs`,
  feature `logging`; worktree and branch removed. Lesson from its fresh-eyes trial: a library `Loader` reads the
  environment only via `.env(map)`, so a doc example that shows an env var must pass it.
- **Row 11 is complete**: ports (#64, #66), logging (#68), paths (#70, 0.5.31) are merged; worktrees and branches removed.
  Paths has its own parser (not `std::path`) under a `Platform`, so Windows rules are tested on every host.
- NEXT: no unblocked row of mine. Check the issue sweep (due ~2026-11-01), `edit::Session` not running `validate_all`
  (known gap, small), and the "did you mean" for unknown schema fields (reported to the PM); otherwise ask the PM.
- Soak wave 1 is done (replied on both channels; you hear peers and Synapse both).
- Consumers: only `synapse/crates/synapsed` depends on configurant (`0.2.34`, a breaking jump to 0.5.x): Synapse lane's call;
  reported to the PM, not edited by us.
- Other rows. Remaining: 29 browser renderer (held), 25 SecretAnt connector (crate is a 0.0.0
  placeholder), 27 RedactAnt (not built), 24 WebHostAnt (its lane), 31 fragment migrations (no need), 8/13 need
  CireSnave. Possible follow-up: the unknown-field schema error lists valid fields but has no
  "did you mean" (reported to the PM as a [FINDING]).
- Every PR must show a fresh-eyes trial of its hello path (PM 2026-10-29): spawn a Sonnet subagent with ONLY the
  doc snippet and a path dependency, one cargo at a time, and put the result and the gaps it found in the PR body.
- Weekly issue sweep (next ~2026-11-01): repo has had 0 issues ever; report `issues: open N, closed N, outsiders
  answered N; roadmap: open N, done N, stale N`.
- persistant 0.12.x is published upstream: bump when wanted (`ProviderUnmet` gained `unverified`; we have no exhaustive
  destructure of it).
- Box memory limit (PM 2026-10-26, until cleared): CARGO_BUILD_JOBS=2, one cargo at a time, no --workspace unless needed.
  NEVER start a second cargo while one runs (a chained background command once did: file-lock error and a flaky scan test).
- Primary checkout stays on `main`; work in worktrees, removed after merge. Three detached `%TEMP%\.tmp*\wt` worktrees in
  `git worktree list` are not ours: leave them.
- Known gap: a secret bool/int in a serde error redacts by substring (can over-redact, safe direction).
- CodeQL: a NEW alert on scan code goes to the PM. Lesson: assert on bools, never format a result into an assert message.
- Tables/maps are file-only; no `Needs` schema field. TUI/GUI binaries offer only `memory`, `fs`.

## Toolchain gotchas (this box)
MSVC `link.exe` cannot load `mspdbcore.dll` from tool shells: use a .ps1 that sets LIB (MSVC 14.51.36231 lib\x64 + Windows Kits
10.0.28000.0 {um,ucrt}\x64), `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld`, `CARGO_TARGET_DIR` under `%TEMP%`, then `& cargo @args`;
call it from Bash with `powershell -NoProfile -File` (`%TEMP%\cg.ps1` is the wrapper; if gone, recreate it and match the MSVC dir with `-like`, not a backslash regex).
Python edit scripts: assert each anchor count == 1; `cargo fmt` re-wraps strings, so re-read a line before using it as
an anchor. A bash heredoc with `'` in a Rust doc comment broke once: write source files with the Write tool. `bash` from
PowerShell is the WSL launcher: run `.github/spdx_gate.sh` from the Bash tool. Never grep all of `C:\Projects`: name the
repo. Bump versions with `sed` on the Cargo.toml files plus a regex on Cargo.lock; check with `cargo metadata --locked`.
Test runs: grep for `FAILED|^error`, not "failed". Windows junctions stand in for symlinks in tests (`mklink /J`).
