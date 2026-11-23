kind: httpant 0.9.0
---
`configurant-web` now depends on `httpant` 0.9.0 (was 0.7.10). No source change was needed: `cargo check --all-targets` is clean and the `configurant-web` tests (4 + 10 + 15 + 1) all pass, including the strict 403-with-unwanted-body test (configurant#111).
