# configurant HANDOFF (rewritten 2026-10-22 by the configurant lane)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`). Read `C:\Projects\CLAUDE.md` and
`CLAUDE.md`. Run on Sonnet, never Opus. You cannot push here: send the PM the SHA of each PR branch. One PR
at a time, not stacked; version bump + CHANGELOG entry in each (breaking = second number pre-1.0).

## State
- Merged: PR1 0.0.1, PR2a 0.1.0, PR2b 0.2.0, **PR3 0.2.1 (#4, main 1a05e24; feature `persistant`)**.
- Local branch `pr4-tui` cut from origin/main 1a05e24, empty. **TASK: PR4 TUI front end** (PM: from origin/main, no stacking).
  Design it behind a clean interface (front end executes a `Plan` only after confirmation); read `docs/DESIGN.md`.
- Not done: *executing* a `DataTransfer` (front-end job, needs both endpoints); a `Needs` field for durability.

## Next (PM queue)
1. PR4 TUI front end (in progress), then GUI (separate crates). Scan assist (LSP) last, suggest-only.
2. Later: secret SOURCE modules (vault-style; today a secret is env-only); compile-time key check (derive macro or
   build script, verifies keys, never writes source); a lossless way to drop a key in a migration.

## Known gaps to decide
- Env/CLI lists are comma-separated; a list item containing a comma cannot be expressed there.
- Module validation is skipped for schema defaults (a missing module must not break a default-only load).
- `ValueDef.min/max` are f64: ints above 2^53 compare imprecisely.
- A secret bool/int in a serde error redacts by substring, so it can over-redact (safe direction).

## Toolchain gotchas (this box)
MSVC `link.exe` cannot load `mspdbcore.dll` from tool shells: PowerShell with LIB=...MSVC\14.51.36231\lib\x64 plus
Windows Kits 10.0.26100.0 {um,ucrt}\x64 (under `C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools`
and `C:\Program Files (x86)\Windows Kits\10\Lib`), `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld`,
`CARGO_TARGET_DIR` under `%TEMP%`. Cargo from the Bash tool fails without that. A bash heredoc with apostrophes in
the body can silently write nothing: use the Write tool. `bash` from PowerShell is the WSL launcher: run
`.github/spdx_gate.sh` from the Bash tool. After `cargo fmt`, re-read a line before using it as a mutation anchor.
