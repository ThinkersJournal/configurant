# configurant-scan

Scan assist for [ConfigurAnt](https://github.com/ThinkersJournal/configurant): finds the environment variables a
Rust project reads and proposes `[[value]]` entries for its schema. It only suggests; a human accepts, and the
schema text is edited with `configurant::schema_edit::add_value`. See `docs/SCAN.md`.

Candidates are found in the source text; a language server (rust-analyzer) confirms each one resolves to
`std::env`. A call the server cannot resolve is counted as unresolved and the result is marked partial, never
presented as complete. A name that looks like a secret is proposed as `secret = true` with no default.

    configurant-scan configurant.toml --root .
    configurant-scan configurant.toml --root . --accept APP_HOST --write

Exit code 0 complete, 2 partial (the server could not resolve every call), 1 error.

Licensed under MIT OR Apache-2.0.
