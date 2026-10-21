# ConfigurAnt design (slice 1)

1. **Schema** = `configurant.toml`: `[project]` (name, version, summary, `schema_version = 1`, optional
   `env_prefix`) and `[[value]]` entries (key, type, default, min/max, choices, description, secret, env, flag,
   restart, provider, required). Parsing is strict and the schema checks itself (duplicate keys/env/flags,
   defaults that do not fit, secrets with defaults or flags, enums without choices, key/table conflicts).
2. **No generated code.** The application reads values through the library (`Config::get::<T>`). A struct
   binding via serde and an opt-in compile-time key check come later; neither writes source.
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
9. **CLI**: `configurant check|describe|explain`. Front ends beyond that (TUI, GUI) are separate crates, later.
10. **Tests**: known-bad inputs first (`tests/known_bad.rs`): unknown key (file, env, flag), wrong type,
    secret leak (file, flag, positional, type error, every output), precedence inversion.

Later: schema versioning and tested reversible migrations; `serde` struct binding; secret-source modules;
the PersistAnt module; scan assist.
