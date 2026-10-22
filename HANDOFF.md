# configurant HANDOFF (rewritten 2026-10-22 by the configurant lane)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`). Read `C:\Projects\CLAUDE.md` and
`CLAUDE.md`. Run on Sonnet, never Opus. You cannot push here: send the PM the SHA of each PR branch. One PR
at a time, not stacked; version bump + CHANGELOG entry in each (breaking = second number pre-1.0).

## State
- PR1 0.0.1 merged. PR2a 0.1.0 (`Config::deserialize`, `#[non_exhaustive]` enums) merged as #2.
- **PR2b 0.2.0 sent to the PM as [READY], branch `pr2b-migrations`** (local): `config_version` in files,
  `[[migration]]` with `rename` / one-to-one `map_value`, `Schema::migrate` / `migrate_back` (toml_edit, comments
  kept, changes are key names only), loader refuses outdated/newer files, `configurant migrate [--from|--to]
  [--write]` (`.bak` first). 26 known_bad + 4 cli tests; six mutations each bite. If the PM holds it, fix on that
  branch; if it merged, branch from `origin/main` for the next row. Check `gh pr view`, do not wait to be told.
- Version is 0.2.0 at the PM's call: the reserved key `config_version` is a breaking behaviour change.

## Next (PM queue)
1. PR3 PersistAnt module (optional crate/feature): `registry::providers`/`open_named` for `list_choices`; a changed
   backend id returns a `Plan` with a `DataTransfer`. Synapse's redb->SQLite copy is the first thing to generalise.
2. PR4 TUI front end, then GUI (separate crates). Scan assist (LSP) last, suggest-only.
3. Later: secret SOURCE modules (vault-style; today a secret is env-only); compile-time key check (derive macro or
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
