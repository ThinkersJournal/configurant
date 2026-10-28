# Library options as a schema fragment

Status: **design, nothing built** (ROADMAP row 22). Written 2026-10-28 for the PM's wave-0 order. Everything
below the heading "What exists today" is read from `src/schema.rs` at 0.5.5; everything else is proposed.

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
   adds every fragment key under the mount name (`webhost.port`, `webhost.tls.cert_dir`). Environment names get
   the project's `env_prefix` plus the mount (`MYAPP_WEBHOST_PORT`) unless the fragment's `env` says otherwise,
   and flags become `--webhost-port`. A default in the application's own `[[value]]` for a mounted key overrides
   the fragment's default (the application knows its context); it may not change the key's type, choices or
   `secret` mark.

4. **Mounting is checked.** A mounted key that collides with an application key, a name or flag that collides,
   an unknown mount name, a fragment whose `schema_version` the core does not know, and an override that changes
   a type are named schema errors with a suggestion, never silently merged. Two mounts of the same fragment
   under different names (two listeners) are allowed; the same name twice is not.

5. **Migrations stay with the owner.** A fragment may carry its own `[[migration]]` steps; they run against
   the mounted keys, so the library can rename an option and an application's existing file still loads.
   The fragment declares its own `config_version`, kept in the file under the mount name.

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

M. Needs nothing from another Ant. It is a precondition for "WebHostAnt and SecretAnt are configured through
ConfigurAnt" (ROADMAP rows 24 and 25).
