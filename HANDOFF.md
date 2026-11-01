# configurant HANDOFF (rewritten 2026-10-31 by the configurant lane)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (Synapse role `pm@ciresnave`) and the pm-assist lane (merge notices). Read `C:\Projects\CLAUDE.md` and `CLAUDE.md`. Run on Sonnet, never Opus. You CAN
push your own branches: push, open the PR, send [READY]. One PR at a time, not stacked; version bump + CHANGELOG
entry in each (breaking = second number pre-1.0).

## State (main = 0.6.3, e8f856d; crates.io has all six crates at 0.6.3, tag v0.6.3; the PM publishes)
- **Rows 29 and 32 are done and published:** `configurant-web` (browser wizard) now serves everything under a random session path
  `/s/<session>/` with no cookie (0.6.0 breaking, first published as 0.6.3; 0.6.0 to 0.6.2 were never published). `persistant` is
  0.14.3. The patent record (`docs/PATENT-SWEEP.md`) covers rows 29 and 32; closest hit US8863248B2, outside as read, with
  re-read triggers (a credential passed from a launching program; a cookie brought back). The registry flip for `configurant-web`
  waits for one outside use (CireSnave). `easyants-card.toml` describes `configurant` only; no web wording (asked the PM, no answer).
WAIT: persistant-kv 0.16.2 on crates.io | check: curl -sf -A configurant-lane https://crates.io/api/v1/crates/persistant-kv/0.16.2 | wake: 2026-11-01T18:00:00Z
- That wait unblocks ROADMAP row 7 (PersistAnt progress callback): read persistant's CHANGELOG for the callback, bump `persistant`, build it. A background poll (5 min) is armed in the live session; re-arm it after a restart.
- **No unblocked row.** Every open row names an outside blocker: 31 waits for an adopter, 27 on RedactAnt, 4/25 on SecretAnt, 7 on
  PersistAnt, 8 and 13 on CireSnave, 6 and 10 on a request. Begin work when a blocker lifts, a [TASK] arrives, or the weekly issue
  sweep is due (next ~2026-11-07: report `issues: open N, closed N, outsiders answered N; roadmap: open N, done N, stale N`).
- Known gaps, not tasks: the browser session address stays in the browser history for the run (`docs/BROWSER-WIZARD.md` "Known limit");
  a secret bool/int in a serde error redacts by substring (over-redacts, safe); one unexplained PARTIAL (exit 2) of the
  `configurant-scan` real-rust-analyzer test on the windows runner (the test now says why it failed).
- Consumers: only `synapse/crates/synapsed` depends on configurant (`0.2.34`); adopting 0.6.x and the `ports` module is the
  Synapse lane's call. Messaging is Synapse only (PM notice 2026-10-31: claude-peers is out): `mcp__synapse__send` / `ack`.
- Every PR that adds a hello path must show a fresh-eyes trial (PM 2026-10-29): a Sonnet subagent with ONLY the doc snippet and a
  path dependency, one cargo at a time; put the result and the gaps in the PR body. A library `Loader` reads the environment
  only via `.env(map)`.
- Box memory limit (PM 2026-10-26, until cleared): CARGO_BUILD_JOBS=2, one cargo at a time, no --workspace unless needed.
- Primary checkout stays on `main`; work in worktrees, removed after merge. The detached `%TEMP%\.tmp*\wt` worktree in
  `git worktree list` is not ours.
- CodeQL: a NEW alert goes to the PM. Lesson (twice now, scan and web tests): assert on bools, never format a response or result into an assert message; use a case number.
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
