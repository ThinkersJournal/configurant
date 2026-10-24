# ConfigurAnt roadmap

Rows that come from the advisory CodeRipper check. Baseline: `coderipper sweep --no-config --profile extended
--workspace --all-features` 0.8.19 at main 9a87cf8 (the default `classic` profile reports nothing). One row per
failing rule; the only silencing is a waiver with its reason in `.coderipper.toml`.

| Rule | Count | Verdict | Row |
|---|---|---|---|


Waived 2026-10-24 (PM ruling): SUP-002 and SEC-006 for RUSTSEC-2026-0370, -0371 and -0372 (`zvariant`, `zbus_names`, `zvariant_derive`: unmaintained, no vulnerability, reached only by `configurant-gui` through `eframe` -> `accesskit_unix`), recorded in `.coderipper.toml`. Re-check on every `eframe`/`accesskit` bump. `cargo deny` does not list them, so `deny.toml` needs no ignore.
