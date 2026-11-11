# Library options as a schema fragment

Status: **built in 0.5.11** (ROADMAP row 22): `Schema::parse_with(text, &[Mount::new(name, fragment)])`, tests in
`tests/known_bad_fragments.rs`. Designed 2026-10-28 for the PM's wave-0 order. Where the build differs from the
design below, a **Built:** note says so. "What exists today" describes 0.5.5, before the build.

## The idea

A library (WebHostAnt, SecretAnt, PersistAnt, an HTTP client) has options of its own: a port, a certificate
directory, a backend id. Today each application that embeds it re-declares those options in its own
`configurant.toml`, or the library invents a second config mechanism. Instead the library ships a **fragment**:
a ConfigurAnt schema for its own options, behind a Cargo feature, and the application mounts it under a name.
The application then gets the library's options in its `--help`, `explain`, TUI, GUI and wizard for free, and the
library reads them back through the same `Config`.

## What exists today

`Schema::parse(text)` takes one TOML document: a `[project]` table plus `[[value]]`, `[[table]]`, `[[map]]` and
`[[migration]]` entries. Keys are dotted (`storage.backend`), unique in the document, with optional `env` and
`flag` names that must also be unique. There is no include, import or merge, so an application cannot combine two
documents, and a library cannot contribute keys. That is the one missing piece.

## How it would work

1. **The library ships text.** Under a feature named `configurant` (off by default, so a library that does not
   want ConfigurAnt pays nothing), it exports a constant:

   ```rust
   #[cfg(feature = "configurant")]
   pub const CONFIGURANT_FRAGMENT: &str = include_str!("../configurant-fragment.toml");
   ```

   No dependency on the `configurant` crate is needed: the fragment is a `&str`. The library reads its own
   options through whatever it already uses, or through `Config::get` when the application passes a `Config` in.

2. **The fragment is a schema without a project.** It has a `[fragment]` table (`name`, `version`,
   `schema_version`, `summary`) instead of `[project]`, and its keys are written relative to the mount point:
   `port`, `tls.cert_dir`. Same `[[value]]` syntax, same checks, same `secret` rule.

3. **The application mounts it.** A new call, `Schema::parse_with(text, &[Mount::new("webhost", FRAGMENT)])`,
   adds every fragment key under the mount name (`webhost.port`, `webhost.tls.cert_dir`). A fragment value that
   declares `env = "PORT"` gets the project's `env_prefix`, the mount and that name (`MYAPP_WEBHOST_PORT`), and a
   `flag = "--port"` becomes `--webhost-port`. **Built:** a value that declares no `env` or `flag` gets none (nothing
   is derived from the key), as in an application's own schema. A default in the application's own `[[value]]` for a mounted key overrides
   the fragment's default (the application knows its context); it may not change the key's type, choices or
   `secret` mark.

4. **Mounting is checked.** A mounted key that collides with an application key, a name or flag that collides,
   an unknown mount name, a fragment whose `schema_version` the core does not know, and an override that changes
   a type are named schema errors with a suggestion, never silently merged. Two mounts of the same fragment
   under different names (two listeners) are allowed; the same name twice is not.

5. **Migrations stay with the owner (built in 0.7.3, ROADMAP row 31, first slice).** A fragment declares its own
   `config_version` in `[fragment]` (default 1) and may carry `[[migration]]` steps with keys relative to the mount
   (`from = "port"`, `to = "listen.port"`); the application's file keeps the version under the mount name
   (`[webhost] config_version = 2`). Design, as built:
   - **Same ops, same rules.** `rename`, `map_value` and `drop` only, each reversible, the chain exactly 1->2, 2->3 ...
     up to the fragment's version (`check_migrations`, reused). A broken chain is a `FragmentParse` error naming the mount.
     Mounting rewrites each op's keys to `<mount>.<key>`, so a step applies to the application's file as written.
   - **Two version lines, two chains.** `Schema::migrate` first carries the application's own chain, then each mount's,
     and returns one `Migrated` (key names in `changes`, never a value). `Schema::migrate_mount_back(text, mount, to)`
     carries one mount back down; `migrate_back` still means the application's own version.
   - **A file with no table for the mount is left alone** (nothing to migrate, no marker written). A mount table with no
     `config_version` is read as version 1, the fragment's first version.
   - **The loader reads the marker like the top-level one:** equal is fine (and the marker is not a setting), older is
     `ConfigError::MountFileOutdated` (run `configurant migrate`), newer is `MountFileNewer`, anything else is a type error.
   - **`configurant migrate SCHEMA FILE --mount NAME=FILE`** (0.7.4, repeatable) mounts the fragments first, so the command
     carries their options too. `--to` still means the application's own version; carrying one mount back is the library
     call `migrate_mount_back` only.
   - **The editor writes the markers on a file it creates (0.7.5, PM ruling 2026-11-10: a config library that creates
     files owns their format version).** `Confirmed::file_text` gives a file with no content yet the application's
     `config_version`, and a table for a mounted library that the edit itself creates that library's. **An existing
     unmarked file is not stamped**: the editor cannot know which version it is at, and stamping it current would hide
     a migration it still needs; `configurant migrate --from N` is how it gets a marker. A marker already in a file is
     never changed, and an edit that changes nothing writes nothing.
   - **Known gap:** two mounts of the same fragment under different names each keep their own marker (it works as
     built and is tested with one mount only).

6. **Tests the library ships.** A library with a fragment adds one test that parses the fragment alone and
   one that mounts it into a toy application schema, so a fragment that stops parsing fails in the library's
   CI, not in an application's.

## Rules that carry over unchanged

- A `secret` in a fragment is never shown, never in an error, `explain`, `help` or a file (CLAUDE.md rule); the
  mount cannot clear the mark.
- Unknown input stays an error with a suggestion, including a typo in a mounted key.
- A `Plan` stays data; a fragment may name a `provider` module but a module is registered by the application.
- A fragment never generates code; the library's option struct is the library's business.

## Open questions (for the PM, not CireSnave)

- Do mounts nest (a fragment that itself mounts another)? Proposed: no in the first version; the application
  mounts every fragment it needs, so the key set is flat and readable in one `help`.
- Should the fragment text be validated at the library's build time? Proposed: no; the library's test (item 6)
  does it, which needs no build script.

## Size and dependencies

Done (M). Needed nothing from another Ant. It is a precondition for "WebHostAnt and SecretAnt are configured through
ConfigurAnt" (ROADMAP rows 24 and 25).
