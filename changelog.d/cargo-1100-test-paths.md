kind: configurant-scan tests find their fake language server on Cargo 1.100
---
Cargo 1.100 puts test executables under `target/<profile>/build/<pkg>/<hash>/out/`, so the `cli` tests, which located the
`fake_lsp` example from `current_exe()`, failed 2 of 5 (`a_server_that_cannot_resolve_makes_the_scan_partial_with_exit_2`,
`server_args_reach_the_server`). The fake server is now a bin of the package (`configurant-scan-fake-lsp`, test support
only, behind the off-by-default `test-support` feature that a self dev-dependency turns on for tests, so `cargo install` does not ship it) and the tests take its path from `CARGO_BIN_EXE_configurant-scan-fake-lsp`, the only path Cargo promises on every
layout. Verified on cargo 1.100.0: red before, green after. Found by the PM's scan of tracked files. No library change.
