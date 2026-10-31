# configurant HANDOFF (rewritten 2026-10-30 by the configurant lane; idle, nothing unblocked)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`; `send_message` takes `to_id`) and the
pm-assist lane (merge notices). Read `C:\Projects\CLAUDE.md` and `CLAUDE.md`. Run on Sonnet, never Opus. You CAN
push your own branches: push, open the PR, send [READY]. One PR at a time, not stacked; version bump + CHANGELOG
entry in each (breaking = second number pre-1.0).

## State (main = 0.5.35 at 063d035; crates.io has 0.5.33, tagged v0.5.33; the PM publishes)
- **IDLE, correctly:** every open ROADMAP row names its blocker in the "Blocked on" column; none is mine to start (PM
  2026-10-30: row 31 stays until an adopter asks). Begin work only when a blocker lifts, a [TASK] arrives, or the weekly
  issue sweep is due (next ~2026-11-07: report `issues: open N, closed N, outsiders answered N; roadmap: open N, done N, stale N`).
- Done: rows 1-2, 5, 9, 11 (ports, logging, paths), 14-16, 19-23, 26, 28, 30. Known gaps, not tasks: a secret bool/int in a serde error redacts by
  substring (over-redacts, safe).
- Consumers: only `synapse/crates/synapsed` depends on configurant (`0.2.34`); adopting 0.5.x and the `ports` module is the
  Synapse lane's call.
- Every PR that adds a hello path must show a fresh-eyes trial (PM 2026-10-29): a Sonnet subagent with ONLY the doc snippet and a
  path dependency, one cargo at a time; put the result and the gaps in the PR body. A library `Loader` reads the environment
  only via `.env(map)`.
- Box memory limit (PM 2026-10-26, until cleared): CARGO_BUILD_JOBS=2, one cargo at a time, no --workspace unless needed.
- Primary checkout stays on `main`; work in worktrees, removed after merge. The detached `%TEMP%\.tmp*\wt` worktree in
  `git worktree list` is not ours.
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
