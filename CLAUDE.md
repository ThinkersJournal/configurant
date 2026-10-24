# ConfigurAnt

Applies to every agent working in this repo. Portfolio rules in `C:\Projects\CLAUDE.md` apply in full and
this file only adds to them. Read `HANDOFF.md` first and begin its first unblocked task.

- **What it is:** schema-first configuration (`configurant` crate and binary). Spec:
  `C:\Projects\CONFIGURANT-SPEC.md`; design: `docs/DESIGN.md`. CireSnave's corrections: the schema is a TOML
  file, NO generated code, modules behind a clean interface, PersistAnt optional.
- **Known-bad tests first.** Every new rule starts as a failing test in `tests/known_bad.rs`; prove a test
  bites by mutating the code under it and seeing it fail.
- **Secrets:** a value marked `secret` must never reach an error, `Debug`, `Display`, `explain`, `help`,
  a file or a command line. Add a leak test for every new output path.
- **Unknown input is an error with a suggestion**, never silently ignored.
- **A `Plan` is data**; only a front end executes it, after confirmation.
- **Repo rules:** pinned actions, `permissions: contents: read`, `deny.toml`, SPDX header on every source
  file (`bash .github/spdx_gate.sh`), MIT OR Apache-2.0, crates.io deps only, `CHANGELOG.md` entry and
  version bump per PR (the PM allocates the number), `cargo fmt` before pushing.
- Lanes can push their own branches (bot token has push on this repo): push, open the PR, send the PM [READY]. Windows linker gotchas: `HANDOFF.md`.
