# Prior art (read 2026-10-21; versions are the crates.io latest that day)

What each crate is, as its crates.io description says, and why ConfigurAnt is a different thing. None of
them is a reason not to build this; each is a reason not to rebuild its part (ConfigurAnt's own
dependencies are only `serde`, `toml`, `thiserror` and, in the binary, `clap`).

- **clap 4.6.7** ("a simple to use, efficient, and full-featured command line argument parser"). Parses
  arguments into a struct you declare in code. It is an input source, not a configuration system: the
  flags are defined in Rust, so a human editing a schema file, a TUI or a GUI cannot see them. We use it
  only for our own binary; the *application's* flags come from the schema.
- **figment 0.10.19** ("a configuration library so con-free, it's unreal"). Merges providers (files,
  environment, serialized structs) into one value and extracts a struct. Layering is solid; what it does not
  have is a schema file: types and defaults live in the Rust struct, unknown keys are only caught if the
  struct denies them, and there is no notion of a secret that must never be printed, nor an `explain`.
- **config 0.15.27** ("layered configuration system for Rust applications"). Sources (files in many
  formats, environment, defaults) merged by key into a dynamic value, read back by key or deserialized.
  Same gap as figment: no declared schema, no source restrictions for secrets, no provenance report.
- **confy 2.0.0** ("boilerplate-free configuration management"). Loads and stores one serde struct in the
  OS configuration directory. Convenient for small tools; no layering, no validation beyond serde, no
  schema.

What ConfigurAnt adds: a **schema file** (so non-code tools can read and edit configuration), **secrets
that are enforced** (accepted only from the environment, never printed, redacted in `explain`), **unknown
keys are errors with a suggestion** (never silently ignored), **`explain`** (which layer set this value and
what it overrode), and **modules** (a persistence backend list, a secrets store) behind one interface.

Only the crates.io descriptions and version numbers were read for this note; the comparisons are from
their documented behaviour and were not exercised here.
