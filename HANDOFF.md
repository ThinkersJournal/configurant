# configurant HANDOFF (rewritten 2026-11-11 by the configurant lane)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (Synapse role `pm@ciresnave`) and the pm-assist lane (merge notices). Read `C:\Projects\CLAUDE.md` and `CLAUDE.md`. Run on Sonnet, never Opus. You CAN
push your own branches: push, open the PR, send [READY]. One PR at a time, not stacked; version bump + CHANGELOG
entry in each (breaking = second number pre-1.0; the PM allocates the number at the gate). End every GitHub post with `— lane: configurant`.

## State (main = 0.7.6 once the docs PR lands; before it, 0.7.5 at 619ae6d; crates.io has all six crates at 0.7.2, tag v0.7.2; 0.7.3 onward are unpublished; the PM publishes)
- **Row 31 (fragment migrations) is done, PM 2026-11-10 "build ahead in small slices":** 0.7.3 (#102) a fragment carries `config_version` and
  `[[migration]]` (keys relative to the mount), `Schema::migrate` runs the app chain then each mount's, `Schema::migrate_mount_back`, the loader reads
  `[mount] config_version` (`MountFileOutdated`/`MountFileNewer`); 0.7.4 (#103) `configurant migrate --mount NAME=FILE`; 0.7.5 (#104) the editor
  writes both markers on a new file (PM option A). An existing unmarked file is left unmarked on purpose (`migrate --from N` stamps it).
  Gaps, in `docs/SCHEMA-FRAGMENTS.md`: two mounts of one fragment under two names are untested; `--to` on the CLI is the app's version only.
- **Row 33 (projection) is done, 0.7.1:** `Config::project`, `configurant project --format env|wrangler|compose` (CireSnave D7, WebHostAnt the customer). WebHostAnt and synapse were told 0.7.2 is on crates.io. Not checked: a real deploy of the output to wrangler or Compose. Embeddable question flow and host write-back are NOT built, by ruling.
- **Row 27 is done, 0.7.2:** redaction is `redactant` 0.1.2. Forms under 4 bytes are matched directly in `scrub` (`src/config.rs`): keep that (PM declined a redactant fallback until a second consumer needs it).
- **Config::deserialize empty tables, 0.7.0 Breaking:** an unset table is absent; a non-Option struct with all keys unset needs `#[serde(default)]`.
- **Rows 6, 7, 13, 29, 32 are done** (browser wizard under `/s/<session>/`, no cookie; `configurant transfer`; `json-schema`; transfer progress). Registry flip for `configurant-web` waits for one outside use (CireSnave). Patent record `docs/PATENT-SWEEP.md` covers rows 29 and 32.
WAIT: weekly issue sweep and any new ROADMAP row | check: gh api repos/ThinkersJournal/configurant/issues --jq length | wake: 2026-11-14T09:00:00Z
- **Nothing unblocked:** 4/25 wait on SecretAnt, 8 on a person with a screen reader (CireSnave), 10 on a consumer request (PM 2026-10-23: Rust only first), 24 on the WebHostAnt lane, 12 on each consumer's own lane. Open issues: 0 (read 2026-11-09, none ever filed).
- **First task on restart:** the weekly issue sweep (close fixed issues with evidence, comment status on the rest, answer outsiders within 3 days,
  `Part of #N` in PRs); report `issues: open N, closed N, outsiders answered N; roadmap: open N, done N, stale N`. Then the next ROADMAP row if one has appeared.
- Known gaps, not tasks: the browser session address stays in the browser history for the run (`docs/BROWSER-WIZARD.md` "Known limit");
  a secret bool/int in a serde error redacts by substring (over-redacts, safe); one unexplained PARTIAL (exit 2) of the
  `configurant-scan` real-rust-analyzer test on the windows runner.
- Consumers: only `synapse/crates/synapsed` depends on configurant (`0.2.34`); adopting 0.7.x is the Synapse lane's call. Messaging is Synapse only: `mcp__synapse__send` (needs `to` and `body`) / `ack`.
- Every PR that adds a hello path must show a fresh-eyes trial (PM 2026-10-29): a Sonnet subagent with ONLY the doc snippet and a
  path dependency, one cargo at a time; put the result and the gaps in the PR body. A library `Loader` reads the environment only via `.env(map)`.
- Box memory limit (PM 2026-10-26, until cleared): CARGO_BUILD_JOBS=2, one cargo at a time, no --workspace unless needed. A multi-crate test run can pass 10 minutes: run it in the background.
- Primary checkout stays on `main`; work in worktrees, removed after merge. The detached `%TEMP%\.tmp*\wt` worktrees in `git worktree list` are not ours.
- CodeQL: a NEW alert goes to the PM. Lesson: assert on bools, never format a response or result into an assert message.
- Tables/maps are file-only; no `Needs` schema field. TUI/GUI binaries offer only `memory`, `fs`.

## Toolchain gotchas (this box)
MSVC `link.exe` cannot load `mspdbcore.dll` from tool shells: use `%TEMP%\cg.ps1` (sets LIB for MSVC 14.51.36231 + Windows Kits 10.0.28000.0,
`CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld`, `CARGO_TARGET_DIR` under `%TEMP%`, then `& cargo @args`); call it from Bash with
`powershell -NoProfile -File "$TEMP/cg.ps1" <cargo args>`. If gone, recreate it and match the MSVC dir with `-like`, not a backslash regex.
Python edit scripts: assert each anchor count == 1; `cargo fmt` re-wraps strings, so re-read a line before using it as an anchor. Write source
files with the Write tool, not a heredoc. `bash` from PowerShell is the WSL launcher: run `.github/spdx_gate.sh` from the Bash tool. Never grep all of
`C:\Projects`: name the repo. Bump versions with `sed` on the Cargo.toml files (also the `version = "x", path` lines) plus the six `configurant*`
entries in Cargo.lock (check each preceding `name =` line: third-party crates share numbers); check with `--locked`.
Test runs: grep for `FAILED|^error` AND `test result`, so a quiet run is not read as a pass. Windows junctions stand in for symlinks in tests (`mklink /J`).
