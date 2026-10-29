# configurant HANDOFF (rewritten 2026-10-29 by the configurant lane)

You are the `configurant` lane (cwd `C:\Projects\configurant`, repo `ThinkersJournal/configurant`). Tasks come
from the PM session (peer cwd `C:\Projects`; id rotates: `list_peers`; `send_message` takes `to_id`) and the
pm-assist lane (merge notices). Read `C:\Projects\CLAUDE.md` and `CLAUDE.md`. Run on Sonnet, never Opus. You CAN
push your own branches: push, open the PR, send [READY]. One PR at a time, not stacked; version bump + CHANGELOG
entry in each (breaking = second number pre-1.0).

## State (main = 0.5.12 at 8781499; crates.io has 0.5.4)
- WAVE 0 (PM 2026-10-28/29): rows 20 prompts+example+hint, 21 view-model (`configurant::view`), 22 fragments
  (`Schema::parse_with`), 23 form projection: ALL DONE and merged (#46, #48, #49, #50, #51). Easy-gate row 30 done
  except the card: `easyants-card.toml` is `not-ready` until a fresh-eyes trial of the README passes (it did once, on
  0.5.4) AND the example test also checks the crates.io version after the PM's publish batch (row 17); then flip it
  to `ready` with `install_url` and `example_path` (format: easyants `docs/CARD-FORMAT.md`).
- NEXT: row 26 step engine (L). Design `docs/STEP-ENGINE.md` is in the open PR (0.5.13); after the PM's answer on its
  two open questions, PR (a) schema `[[step]]` + `when` + ordering, PR (b) `Wizard`, events, `View.wizard`, resume.
  Then 28 plan executor (M-L), 29 browser renderer (L, needs HTTPAnt/WebHostAnt).
- Blocked outside: 25 SecretAnt connector (crate is a 0.0.0 placeholder), 27 RedactAnt handover (not built), 24
  WebHostAnt fragment (its lane), 31 fragment migrations (no need yet). 8/13 need CireSnave; 11 no design.
- Every PR must show a fresh-eyes trial of its hello path (PM 2026-10-29): spawn a Sonnet subagent with ONLY the
  doc snippet and a path dependency, one cargo at a time, and put the result and the gaps it found in the PR body.
- Weekly issue sweep (next ~2026-11-01): repo has had 0 issues ever; report `issues: open N, closed N, outsiders
  answered N; roadmap: open N, done N, stale N`.
- persistant 0.12.0 is merged upstream but unpublished: when published, bump and fix any exhaustive destructure of
  `ProviderUnmet` (we have none today).
- capant asked about adoption: answered A (Needs stays strings; no ConfigurAnt change).
- Box memory limit (PM 2026-10-26, until cleared): CARGO_BUILD_JOBS=2, one cargo at a time, no --workspace unless needed.
- Primary checkout stays on `main`; work in worktrees (`git worktree add -b <b> ../configurant-wt-<topic> origin/main`),
  removed after merge. A stale unregistered dir `configurant-wt-prompts` is left over (permission denied once): delete it.
- Known gap: a secret bool/int in a serde error redacts by substring (can over-redact, safe direction).
- CodeQL: a NEW alert on scan code goes to the PM. Lesson: assert on bools, never format a result into an assert message.
- Tables/maps are file-only; no `Needs` schema field. TUI/GUI binaries offer only `memory`, `fs`.

## Toolchain gotchas (this box)
MSVC `link.exe` cannot load `mspdbcore.dll` from tool shells: use a .ps1 that sets LIB (MSVC 14.51.36231 lib\x64 + Windows Kits
10.0.28000.0 {um,ucrt}\x64), `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld`, `CARGO_TARGET_DIR` under `%TEMP%`, then `& cargo @args`;
call it from Bash with `powershell -NoProfile -File`. Python edit scripts: assert each anchor count == 1; `cargo fmt` re-wraps
strings, so re-read a line before using it as an anchor (the Edit tool needs a Read first). A bash heredoc with `'` in a
Rust doc comment broke once: write source files with the Write tool. `bash` from PowerShell is the WSL launcher: run
`.github/spdx_gate.sh` from the Bash tool. Never grep all of `C:\Projects`: name the repo. Bump versions with `sed` on
Cargo.toml files + a regex on Cargo.lock, check with `cargo metadata --locked`. Linux check: `tar -c --exclude=.git . |
wsl -d lane-spike -e sh -c 'mkdir -p /tmp/cfs && cd /tmp/cfs && tar x'`, then cargo test there.
