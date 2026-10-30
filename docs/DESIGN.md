# ConfigurAnt design (slice 1)

1. **Schema** = `configurant.toml`: `[project]` (name, version, summary, `schema_version = 1`, optional
   `env_prefix`) and `[[value]]` entries (key, type, default, min/max, choices, description, secret, env, flag,
   restart, provider, required). Parsing is strict and the schema checks itself (duplicate keys/env/flags,
   defaults that do not fit, secrets with defaults or flags, enums without choices, key/table conflicts).
2. **No generated code.** The application reads values through the library (`Config::get::<T>`), or as a
   typed view of their own struct (`Config::deserialize::<T>`, serde, by dotted key). The opt-in key check
   (`configurant::keycheck`, `configurant keys <schema> <src dir>`, or from a `build.rs`) reads source text and
   reports every `.get::<T>("k")`, `.value("k")`, `.rows("k")` or `.explain("k")` whose literal key the schema
   does not define, with a suggestion; it does not write source. It is a text match, not type-aware: a computed
   name is counted and never guessed, and another type's method of the same name with a string argument is
   reported too (spell that key as a variable).
3. **Layers**: default < file < environment < command line. A load takes its inputs as data and returns all
   errors, not the first. `Loader::build_with_order` exists so the precedence test can show it fails when
   the order is inverted.
4. **Errors** (`ConfigError`) name key and origin: `UnknownKey` (with suggestion), `WrongType`, `OutOfRange`,
   `NotAllowed`, `SecretInForbiddenSource`, `MissingRequired`, `NotSet`, `FileParse`, `Module*`. A secret's
   value is `<redacted>` wherever it would be shown.
5. **Secrets** come from the environment, or from a source a `secret` value declares
   (`source = "<source>:<reference>"`, fetched by `configurant-secrets`; [SECRET-SOURCES.md](SECRET-SOURCES.md)); the
   environment wins. Not accepted from a file or the command line, no default, no flag, never in
   `help`, `explain`, `Debug` or an error.
6. **`explain(key)`** lists each layer that set the key, lowest first, and marks the one used.
7. **Modules** (`Module`, interface version 2, version 1 modules still register): `list_choices(needs)`, `validate(value)`,
   `on_change(old, new) -> Plan`. A value with `provider = "x"` is validated by module `x`; with the module
   absent, defaults still load and any other value is refused naming the module. With it present, a default is
   checked too: a default its module rejects fails the load, naming the key. `validate_all(values)` (version 2)
   sees every winning value that names the module at once, for rules that compare keys (two ports that collide).
8. **A `Plan` is data.** A module never acts. The **front end** (CLI, TUI, GUI) shows the plan and executes
   it only after the user confirms. The PersistAnt module's data transfer will be a `Plan`.
9. **CLI**: `configurant check|describe|explain|migrate`. Front ends beyond that are separate crates: `configurant-tui` exists; `configurant-gui` exists too.
10. **Tests**: known-bad inputs first (`tests/known_bad.rs`): unknown key (file, env, flag), wrong type,
    secret leak (file, flag, positional, type error, every output), precedence inversion.

11. **Migrations of config files.** A file says which version of the project's file format it is in
    (`config_version`, absent = current). The schema lists one `[[migration]]` per step, 1->2, 2->3, ...,
    up to `[project] config_version`. Steps are made of `rename` and one-to-one `map_value` only, because a
    migrated file must be carriable back and an operation that drops or invents data cannot promise that
    (a value outside a map passes through; one the map *produces* is refused as ambiguous). The library
    returns text and a key-names-only change list; a front end writes (`migrate --write` keeps `.bak`).
    It edits with `toml_edit`, so comments and layout survive. Dropping a key is offered only in its lossless
    form, `{ op = "drop", key = "a.b", value = "x" }`: it removes the key while it holds exactly that string,
    refuses any other value (`DropMismatch`, which never shows the value), and carrying back writes
    `a.b = "x"` again. A file that never had the key gains it when carried back; a value that is not a
    constant string is still migrated by the project's own code.

12. **The PersistAnt module** (feature `persistant`, `configurant::persistant`). The five-step contract of
    PersistAnt's registry seen from configuration: needs in (capability ids that must hold, plus an order of
    importance), the backends this build offers out (`registry::providers`), the pick stored as the value of
    a `provider = "persistant"` key, and a changed pick returns a `Plan` with a `DataTransfer { from, to }`.
    The module owns no backend list (a PersistAnt upgrade reaches every project whose needs a new backend
    meets) and never opens a backend: the front end executes the plan, with both endpoints, after
    confirmation. The minimum durability is the module's setting (default `Process`), since `Needs` has no
    field for it. `Module::list_choices` cannot report an error, so it returns an empty list and
    `try_list_choices` carries the reason.

13. **Front ends** (`configurant::edit`). A `Session` stages edits against a loaded `Config`, through the same
    checks as a load; a secret is never editable. `review()` is display only; `Review::confirm()` is the one
    way to a `Confirmed`, which alone yields new file text and the plan to execute. A front end (the
    `configurant-tui` and `configurant-gui` crates) draws, asks, writes and executes; the library decides what is valid.

Built since: secret sources ([SECRET-SOURCES.md](SECRET-SOURCES.md), slices 1-2) and scan assist ([SCAN.md](SCAN.md)). What is open is listed in `ROADMAP.md`.
