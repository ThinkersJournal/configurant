# ConfigurAnt

```rust
use configurant::{Loader, Schema};
fn main() {
    let toml = r#"
project = { name = "app", version = "1", summary = "demo", schema_version = 1 }
value = [{ key = "server.port", type = "int", default = 8080, description = "port" }]
"#;
    let schema = Schema::parse(toml).unwrap();
    let c = Loader::new(&schema).build().unwrap(); // default < file < env < command line
    assert_eq!(c.get::<i64>("server.port").unwrap(), 8080);
}
```

Install with `cargo add configurant`, paste it into `src/main.rs` and `cargo run`. It prints nothing: it succeeds when the
assertion holds. The comment is the layering order (a later source overrides an earlier one): the value is `8080` here
because only the schema's default is set; a `--flag`, an environment variable or a config file would change it. The
`persistant` features are optional and off by default. This snippet is `examples/quickstart.rs`; CI runs it
(`tests/quickstart_example.rs`), so it cannot go stale.

Each value in the schema takes: `key`, `type`, `default`, `min`, `max`, `choices`, `description`, `prompt`, `hint`, `example`, `section`, `advanced`, `secret`, `env`, `source`, `flag`, `restart`, `provider`, `required`. An unknown field is an error that lists these.

Schema-first configuration for Rust projects. Declare every value once in `configurant.toml`; the
library layers **default < file < environment < command line**, validates with named errors, and
`explain`s where each value came from. **No generated code**: the schema is a plain file that tools
and humans can edit, and the application reads values through the library. **Secrets are enforced**:
environment only, never printed.

The commands below need the binary: `cargo install configurant` (the examples are in the [repository](https://github.com/ThinkersJournal/configurant/tree/main/examples); the other crates are `configurant-tui`, `configurant-gui` and `configurant-scan`).

```
configurant check   examples/configurant.toml
configurant describe examples/configurant.toml
configurant migrate examples/configurant.toml app.toml --from 1   # prints; --write saves app.toml.bak first
DEMO_PORT=9000 configurant explain examples/configurant.toml server.port -- --port 9100
```

```rust
// Opt-in typed view: `server.port` is field `port` of field `server`. Nothing is generated.
let settings: Settings = config.deserialize()?; // Settings: your own #[derive(Deserialize)] struct
```

Key check (`configurant keys examples/configurant.toml src`, or `configurant::keycheck` from a `build.rs`): finds
Rust code that asks for a key the schema does not define. Known limits: it matches source text, so it sees only
`.get::<T>("k")`, `.value("k")`, `.rows("k")` and `.explain("k")` with a plain string literal; a key read through a
helper function (`fn setting(c: &Config, k: &str) { c.get::<u16>(k) }`, called as `setting(c, "server.prot")`) is
a computed name at the call inside the helper, so it is counted and not checked, and its callers are not followed;
a key built with `format!` or held in a variable is the same; another type's method named `value`, `rows` or
`explain` with a string argument is reported as if it were ours.

Status: 0.5.x (pre-1.0; a breaking change moves the second number). Schema, layering, validation, `explain`, serde struct binding, config-file migrations and the module
interface exist, and the PersistAnt module is behind the `persistant` feature; `configurant-tui` and `configurant-gui` edit a file with review and backup, `configurant-scan` proposes values. Design:
[docs/DESIGN.md](https://github.com/ThinkersJournal/configurant/blob/main/docs/DESIGN.md); why not figment/config/clap/confy: [docs/PRIOR-ART.md](https://github.com/ThinkersJournal/configurant/blob/main/docs/PRIOR-ART.md).
Licence: MIT OR Apache-2.0.
