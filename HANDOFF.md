# configurant HANDOFF (rewritten 2026-10-21 by the agentlife lane, who built PR1)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`). Read `C:\Projects\CLAUDE.md` and
`CLAUDE.md`. Run on Sonnet, never Opus. You cannot push here: send the PM the SHA of each PR branch.

## Done in PR1 (0.0.1, version is the PM's to adjust)
Schema + strict parser + self-checks; layering default < file < env < CLI (`Loader`, `build_with_order`);
validation with named errors (all collected); `explain`; secrets enforced (env only, redacted everywhere);
module interface (`Module`, `Modules`, `ChoiceList` demo, `Plan` as data); `configurant check|help|explain`;
16 known-bad tests in `tests/known_bad.rs`; docs/DESIGN.md, docs/PRIOR-ART.md. One leak test was proven to
bite by mutation (un-redacting the type-error text fails it); the tests were written before the code but
were not run red one by one.

## Next (PM-approved additions, none started)
1. `Config::deserialize::<MyStruct>()` via serde: opt-in struct binding, still no codegen.
2. Schema versioning + migration of existing config files; each migration a tested reversible step.
3. Secret SOURCE modules (vault-style): deliberately not in PR1; today a secret is env-only.
4. PersistAnt module (separate crate or feature): `registry::providers`/`open_named` for `list_choices`; a changed
   backend id returns a `Plan` with a `DataTransfer`. Synapse's redb->SQLite copy is the first thing to generalise.
5. TUI front end, then GUI (separate crates). Scan assist (LSP) last, suggest-only.
6. Compile-time key check (derive macro or build script): verifies keys, never writes source.

## Known gaps to decide
- Env/CLI lists are comma-separated; a list item containing a comma cannot be expressed there.
- Module validation is skipped for schema defaults (a missing module must not break a default-only load).
- `ValueDef.min/max` are f64: ints above 2^53 compare imprecisely.

## Toolchain gotchas (this box)
MSVC `link.exe` cannot load `mspdbcore.dll` from tool shells: PowerShell with LIB=...MSVC\14.51.36231\lib\x64 plus
Windows Kits 10.0.26100.0 {um,ucrt}\x64 (under `C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools`
and `C:\Program Files (x86)\Windows Kits\10\Lib`), `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld`,
`CARGO_TARGET_DIR` under `%TEMP%`. A bash heredoc with apostrophes in the body can silently write nothing: use the Write tool.
