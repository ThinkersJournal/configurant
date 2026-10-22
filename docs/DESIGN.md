# ConfigurAnt design (slice 1)

1. **Schema** = `configurant.toml`: `[project]` (name, version, summary, `schema_version = 1`, optional
   `env_prefix`) and `[[value]]` entries (key, type, default, min/max, choices, description, secret, env, flag,
   restart, provider, required). Parsing is strict and the schema checks itself (duplicate keys/env/flags,
   defaults that do not fit, secrets with defaults or flags, enums without choices, key/table conflicts).
2. **No generated code.** The application reads values through the library (`Config::get::<T>`), or as a
   typed view of their own struct (`Config::deserialize::<T>`, serde, by dotted key). An opt-in
   compile-time key check comes later; it does not write source.
3. **Layers**: default < file < environment < command line. A load takes its inputs as data and returns all
   errors, not the first. `Loader::build_with_order` exists so the precedence test can show it fails when
   the order is inverted.
4. **Errors** (`ConfigError`) name key and origin: `UnknownKey` (with suggestion), `WrongType`, `OutOfRange`,
   `NotAllowed`, `SecretInForbiddenSource`, `MissingRequired`, `NotSet`, `FileParse`, `Module*`. A secret's
   value is `<redacted>` wherever it would be shown.
5. **Secrets** come only from the environment in this version (a secret-source module is a later,
   deliberate extension). Not accepted from a file or the command line, no default, no flag, never in
   `help`, `explain`, `Debug` or an error.
6. **`explain(key)`** lists each layer that set the key, lowest first, and marks the one used.
7. **Modules** (`Module`, interface version 1): `list_choices(needs)`, `validate(value)`,
   `on_change(old, new) -> Plan`. A value with `provider = "x"` is validated by module `x`; with the module
   absent, defaults still load and any other value is refused naming the module.
8. **A `Plan` is data.** A module never acts. The **front end** (CLI, TUI, GUI) shows the plan and executes
   it only after the user confirms. The PersistAnt module's data transfer will be a `Plan`.
9. **CLI**: `configurant check|describe|explain|migrate`. Front ends beyond that (TUI, GUI) are separate crates, later.
10. **Tests**: known-bad inputs first (`tests/known_bad.rs`): unknown key (file, env, flag), wrong type,
    secret leak (file, flag, positional, type error, every output), precedence inversion.

11. **Migrations of config files.** A file says which version of the project's file format it is in
    (`config_version`, absent = current). The schema lists one `[[migration]]` per step, 1->2, 2->3, ...,
    up to `[project] config_version`. Steps are made of `rename` and one-to-one `map_value` only, because a
    migrated file must be carriable back and an operation that drops or invents data cannot promise that
    (a value outside a map passes through; one the map *produces* is refused as ambiguous). The library
    returns text and a key-names-only change list; a front end writes (`migrate --write` keeps `.bak`).
    It edits with `toml_edit`, so comments and layout survive. Dropping a key is deliberately not offered:
    until a lossless form exists, a removed key is migrated by the project's own code.

Later: secret-source modules; the PersistAnt module; scan assist.
