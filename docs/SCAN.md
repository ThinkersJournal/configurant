# Design note: scan assist (LSP)

Status: decided; slice 1 built in 0.2.12 (answers at the end). Text candidates plus LSP go-to-definition to
confirm `std::env`, not `find_references`: workspace symbols do not index the standard library by default. Facts read at configurant
`origin/main` 76a293e and `lsp-bridge` 0.2.0 (crates.io, CireSnave's own crate, MIT OR Apache-2.0).

## Today

The spec (item 5) asks for an optional, later "LSP-backed scan [that] proposes likely configuration variables
to add to the schema; it only suggests, a human accepts." Nothing exists. A schema is a hand-written TOML
file; the TUI/GUI edit *values*, never the schema.

## What lsp-bridge gives us

`LspBridge::{register_server, start_server, open_document, find_references, get_workspace_symbols,
get_document_symbols, go_to_definition, get_hover, shutdown}` (async, tokio), with server lifecycle and
crash restart. It returns `lsp_types` positions and symbols. It gives **locations**, not meaning: an LSP
cannot say "this string literal is a config key". So the scan is two steps: ask the language server where a
known *reader API* is called, then read the call's literal argument from the source text at that location.

## Proposal

**1. Where it lives.** A new optional crate `configurant-scan` (not a feature of the core, so `configurant`
keeps its dependency set and its "no generated code, no async" shape). It depends on `configurant` (for
`Schema`) and `lsp-bridge`. The core gains nothing; the scan reads a `Schema`, returns data.

**2. Reader patterns are data.** A `ReaderPattern { language, symbol, arg_index, kind }` names an API whose
call sites carry a key: `std::env::var`, `std::env::var_os`, `env!`/`option_env!` (literal), and
`Config::get`/`Config::deserialize` of this very library (a key the code asks for that the schema lacks).
Built-in set for Rust first; further languages are more patterns plus a server command, not new code. The
set is a TOML file the user may extend, in the same spirit as the schema.

**3. Algorithm.** (a) Register the server for the language and `start_server`. (b) For each pattern,
`get_workspace_symbols(symbol)` to find the definition, then `find_references` for its call sites. (c) At each
site read the file text (the scan reads files; the server is not trusted to return content) and take the
literal argument; a non-literal argument is reported as `dynamic` with its location, never guessed.
(d) Diff against the schema: a literal key with no `[[value]]` whose `env` or key matches is a **proposal**.
(e) Shut the server down.

**4. Output is a `Proposals` value, data only** (a `Plan`-style rule): `Proposal { key_guess, env, sites:
Vec<Site{file,line}>, default_seen: Option<String>, confidence }`. `default_seen` is a literal passed to
`unwrap_or`/`unwrap_or_else` on the same expression, found by text, and only kept if it parses as a bool,
integer or short string. Nothing is written to the schema. The front end shows proposals; **accepting** one
calls a new `schema_edit::add_value(&Schema text, Proposal) -> text` using `toml_edit` (comments and layout
survive, as in `migrate`), through the same self-check as a schema load, and the user writes the file.

**5. Secrets.** A scan can see that `std::env::var("DB_PASSWORD")` exists; it never sees the value (it reads
source, not the environment). A name matching `password|secret|token|key|credential|dsn` is proposed with
`secret = true` pre-set and **no default and no flag** (the schema's own rule), and a literal default seen
beside such a name is dropped, not shown. A leak test covers every output path (`Debug`, `Display`, the
proposal list, the rendered TOML), as for every new output path in this repo.

**6. Failure is loud.** A missing server binary, a server that does not answer inside a timeout, or an LSP
error is a `ScanError` naming the server command. A server that answers with zero references for a pattern
whose symbol it did find is reported as `0 sites` beside the symbol (not as "nothing to add"). Unknown
language is an error with a suggestion (the supported list). A scan that was cut short reports itself as
partial; it never returns an empty proposal list as if complete.

**7. CLI and front ends.** `configurant scan --lang rust [--server CMD] [--root DIR]` prints proposals
(read-only; works without a TTY). The TUI and GUI get a later "Add from scan" step that uses the same
`Proposals` type; not in the first slice.

**8. Tests (known-bad first).** Fixture projects with: a literal env read; a dynamic key; a secret-looking
name with a literal default (default must not appear anywhere); a key already in the schema (no proposal); a
server that is absent; a server that dies mid-scan (partial). The server is faked by a scripted stdio LSP
stub in the test crate, so CI needs no rust-analyzer; one real rust-analyzer test runs on a self-hosted
runner if CireSnave wants it (CLAUDE.md 5.1g), not `#[ignore]`.

## Build order

1. `configurant-scan` with the stub-server tests and Rust patterns; `schema_edit::add_value` in core
   (additive; usable without a scan). 2. `configurant scan` subcommand. 3. TUI/GUI "Add from scan".

## Questions for the PM / CireSnave

1. Own crate `configurant-scan` (recommended) or a `scan` feature of the core? A feature would pull tokio and
   lsp-bridge into every consumer's lockfile.
2. Rust only first (recommended), or also Python/TypeScript patterns in slice 1?
3. Is a real rust-analyzer test on the self-hosted runner wanted, or is the scripted stub enough?
4. `lsp-bridge` 0.2.0's README example says `lsp-bridge = "0.1"` and its build badge points at a placeholder
   repo; harmless here, but worth fixing in that crate before we depend on it.

## Answers (PM, 2026-10-23)

1. Own crate `configurant-scan`. 2. Rust only first. 3. Stub tests in CI plus one real rust-analyzer test,
installed on the runner with `rustup component add rust-analyzer rust-src`. 4. `lsp-bridge` is CireSnave's
(`ciresnave/lsp-bridge`); its README fix went to 0.2.1.

## Built in slice 1

`configurant-scan` with `Resolver` (trait; `LspResolver` over `lsp-bridge`), `scan`, `Proposals`;
`configurant::schema_edit::add_value`. Not built: the `configurant scan` subcommand and TUI/GUI "Add from scan".
`lsp-bridge` 0.2 sends `rootUri = "file://" + path`, which is not a URI for a Windows path, so rust-analyzer
never loaded the workspace; `LspResolver` passes the working directory and `linkedProjects` instead.
