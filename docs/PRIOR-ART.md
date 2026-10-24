# Prior art (reviewed 2026-10-24; versions and licences are crates.io / GitHub as of that day)

For each project: what it is, what it does that matters here, and a verdict, **adopt** (we take the idea, or the
crate), **differ** (we deliberately do otherwise, with the reason) or **n/a**. Nothing here copies code; where a
verdict says "adopt", it is an idea or a dependency under its own licence. Licences are the SPDX values
crates.io or the GitHub API reported; the crate/repo link is the source.

How each was checked is stated per row: **read** = I read the repo README or source file named; **desc** = only the
registry/forge description. Nothing was run. (The 2026-10-21 version of this note was `desc` only and said figment
has no provenance report; that was wrong, see figment.)

## Rust

| Project | Licence | Checked | What it is | Verdict |
|---|---|---|---|---|
| [clap](https://github.com/clap-rs/clap) 4.6.7 | MIT OR Apache-2.0 | desc | Argument parser, flags declared in Rust. | **Adopt** for our own binary only (already). **Differ**: the application's flags come from the schema, so a TUI/GUI/human editing a TOML file sees them. |
| [figment](https://github.com/SergioBenitez/Figment) 0.10.19 | MIT OR Apache-2.0 (crates.io; the GitHub API reports Apache-2.0) | read (`src/figment.rs`, `src/error.rs`) | Providers merged into one value, then `extract` into a struct. **Has provenance**: `Figment::find_metadata(key)` and `Error::origins()` name the provider (and its source) behind a value. Profiles. | **Adopt** the idea that an error names the layer that supplied the bad value (we already do; `explain` goes further and lists what it overrode). **Differ**: types/defaults live in the Rust struct, no schema file, no notion of a value that must never be printed. Not a dependency: it would put the type system, not a TOML file, in charge. Last release 2024-05-17. |
| [config](https://github.com/rust-cli/config-rs) 0.15.27 | MIT OR Apache-2.0 | read (README) | Layered sources (files in several formats, env, defaults), dotted paths (`redis.port`, `databases[0].name`), a `Format` trait to add formats. | **Adopt** the extensibility-by-trait shape (our `Module`, `SecretSource`). **Differ**: dynamic value read by key; no declared schema, no secret handling, no unknown-key error unless the target struct denies it. |
| [confy](https://github.com/rust-cli/confy) 2.0.0 | MIT/X11 OR Apache-2.0 | desc | Load/store one serde struct in the OS config dir. | **Differ**: no layering or validation. Adopt nothing; the OS config-dir convention is a front-end choice we can make without it. |
| [envy](https://github.com/softprops/envy) 0.4.2 | MIT | desc | Deserialize environment variables into a struct. Last release 2021-01-04. | **Differ**: our env layer is one layer among several and is declared per value (name, type, secret). |
| [dotenvy](https://github.com/allan2/dotenvy) 0.15.7 | MIT | desc | Loads `.env` into the process environment. | **Differ** (deliberate): a `.env` file would be a second place for a secret to sit on disk, against the rule that a secret comes from the environment or a declared source. If ever wanted it is a `SecretSource`, not a layer. |
| [schemars](https://github.com/GREsau/schemars) 1.2.3 | MIT | desc | Generates JSON Schema from Rust types. | **Differ**: it goes Rust to schema, ours is schema (TOML) with no generated code (CireSnave's correction). **Possible later**: export a ConfigurAnt schema *as* JSON Schema for editors; not scheduled. |
| [twelf](https://github.com/bnjjj/twelf) 0.15.0 | MIT | desc | Layered configuration derive macro (files, env, clap). | **Differ**: a derive macro is generated code, which the project forbids. |
| [serde_with](https://github.com/jonasbb/serde_with/) 3.24.0 | MIT OR Apache-2.0 | desc | serde helper adapters. | **n/a**: we do not deserialize into user types. |

## Other languages (design lessons, not dependencies)

| Project | Licence | Checked | What it is | Verdict |
|---|---|---|---|---|
| [pydantic-settings](https://github.com/pydantic/pydantic-settings) | MIT | read (`docs/index.md`) | Typed settings from env, dotenv, secrets sources; default `extra='forbid'`. | **Adopt**: unknown input is an error by default (the same rule as ours); sources are pluggable and ordered. **Differ**: types are Python classes, not a file a TUI can edit. Its docs warn that a name mismatch between sources can make lower-priority values vanish silently (with `extra='ignore'`), the failure our "unknown key is an error with a suggestion" is meant to prevent. |
| [Dynaconf](https://github.com/dynaconf/dynaconf) | MIT | read (README) | Layered environments, validators, a `.secrets.*` file kept out of git, Vault and Redis loaders, a CLI (`init, list, write, validate, export`). | **Adopt** the CLI verbs idea (we have `check`, `explain`, `describe`). **Differ**: a `.secrets.toml` holds secrets in a file; we refuse that. Its Vault loader is the same slot as our `SecretSource`; ours is a separate crate. |
| [Viper](https://github.com/spf13/viper) | MIT | read (README) | Go: defaults, file, env, flags, remote KV (etcd, Consul, Firestore), live watching. | **Adopt**: a documented precedence order, as `explain` makes it visible. **Differ**: keys are case-insensitive and untyped by default, no schema, no secret class. Live watching and remote KV are out of scope. |
| [Hydra](https://github.com/hydra-ecosystem/hydra) (repo moved from facebookresearch) | MIT | desc | Composes configuration from groups, with command-line overrides. | **n/a** for now: composing config *groups* is a bigger problem than layering; not scheduled. |
| [Typesafe Config / HOCON](https://github.com/lightbend/config) | Apache-2.0 (file `LICENSE-2.0.txt`; the GitHub API reports none) | desc | JVM config library and format. | **Differ**: a new file format; we use plain TOML. |
| [CUE](https://github.com/cue-lang/cue), [Pkl](https://github.com/apple/pkl) | Apache-2.0 | desc | Configuration languages with validation. | **Differ**: a language to learn and run; ConfigurAnt's schema is data. They are the right answer when configuration needs computation; we say no to computation. |
| [Dhall](https://github.com/dhall-lang/dhall-lang) | BSD-3-Clause | desc | Typed, total configuration language. | **Differ**, as CUE/Pkl. |
| [patrickt/configurant](https://github.com/patrickt/configurant) | Apache-2.0 | desc (last push 2022-11-04) | Haskell library loading configuration from environment variables. | **n/a**: shares the name only. No code or text is taken. |

## What survives the review

ConfigurAnt's combination is the same as on 2026-10-21, with the figment correction: a **schema file** (non-code
tools can read and edit it), **secrets that are enforced** (environment or a declared source, never printed or
written, redacted in `explain`), **unknown keys are errors with a suggestion**, **`explain`** (the layer and what it
overrode; figment names the supplying provider, `explain` also names what was overridden), and **modules** behind
one interface. Dynaconf and pydantic-settings are closest in spirit; both are tied to Python classes, and
Dynaconf keeps secrets in a file.

## Provenance

No third-party text or code is copied into this repository from any project above.
