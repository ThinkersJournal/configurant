# configurant HANDOFF (rewritten 2026-10-22 by the configurant lane)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`). Read `C:\Projects\CLAUDE.md` and
`CLAUDE.md`. Run on Sonnet, never Opus. You cannot push here: send the PM the SHA of each PR branch. One PR
at a time, not stacked; version bump + CHANGELOG entry in each (breaking = second number pre-1.0).

## State
- Merged: PR1 0.0.1, PR2a 0.1.0, PR2b 0.2.0, PR3 0.2.1, PR4 0.2.2 (#5), AntSpawn schema 0.2.3 (#6, main 6d31b90).
- **Arrays of tables + maps (0.2.4) = branch `core-tables-maps`, worktree `C:\Projects\configurant-tables`, to be
  sent to the PM as a SHA for the gate.** Known-bad tests in `tests/known_bad_tables.rs` (mutation-checked: secret
  field, value echo, unknown row key each fail their test); `examples/antspawn.toml` now has mount/output/job.env.
  If the gate asks for changes, fix in that worktree. Local `pr4-tui` here is merged: delete it when this tree is idle.
- Not done: executing a `DataTransfer`; a `Needs` field; the TUI passes no modules; tables/maps are file-only (not
  editable in `edit::Session`, no env/flag); no `min_rows`/`max_rows`; a higher layer would replace a whole array.

## Next
1. After the gate: re-express AntSpawn's own spec loading through configurant (AntSpawn lane's call).
2. GUI front end later. Scan assist (LSP) last. Secret SOURCE modules; compile-time key check; lossless key drop.

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
