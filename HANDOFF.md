# configurant HANDOFF (rewritten 2026-10-30 by the configurant lane after 0.5.25)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`; `send_message` takes `to_id`) and the
pm-assist lane (merge notices). Read `C:\Projects\CLAUDE.md` and `CLAUDE.md`. Run on Sonnet, never Opus. You CAN
push your own branches: push, open the PR, send [READY]. One PR at a time, not stacked; version bump + CHANGELOG
entry in each (breaking = second number pre-1.0).

## State (main = 0.5.25 at 07dc795; crates.io has 0.5.22; the PM publishes)
- Done and merged: wave 0 (rows 20-23), step engine (26), patent re-sweep (19), plan executor a/b/c (row 28), hub card
  `ready` (#57), row 11 design (#63), **ports slice 1 (#64, 0.5.25)**: `modules::ports::Ports` (feature `ports`), schema
  field `privileged`, `Module::validate_for`. Synapse lane said YES: `synapsed` `listen.addr` adopts it in its own bump.
- NEXT (row 11 slice 2, unblocked, start without a [TASK]): interface version 2 = `Module::validate_all(&[(&str,&Value)])`
  and ports rule P5 (two ports in one schema must not collide); design is `docs/MODULES.md` section 2. Known-bad tests
  first (`tests/known_bad_modules.rs`), mutate to prove they bite, version bump + CHANGELOG, fresh-eyes trial in the PR
  body. Then logging, then paths (order in MODULES.md section 5).
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
