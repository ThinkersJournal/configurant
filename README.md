# ConfigurAnt

Schema-first configuration for Rust projects. Declare every value once in `configurant.toml`; the
library layers **default < file < environment < command line**, validates with named errors, and
`explain`s where each value came from. **No generated code**: the schema is a plain file that tools
and humans can edit, and the application reads values through the library. **Secrets are enforced**:
environment only, never printed.

```
configurant check   examples/configurant.toml
configurant describe examples/configurant.toml
configurant migrate examples/configurant.toml app.toml --from 1   # prints; --write saves app.toml.bak first
DEMO_PORT=9000 configurant explain examples/configurant.toml server.port -- --port 9100
```

```rust
// Opt-in typed view: `server.port` is field `port` of field `server`. Nothing is generated.
let settings: Settings = config.deserialize()?;
```

Key check (`configurant keys examples/configurant.toml src`, or `configurant::keycheck` from a `build.rs`): finds
Rust code that asks for a key the schema does not define. Known limits: it matches source text, so it sees only
`.get::<T>("k")`, `.value("k")`, `.rows("k")` and `.explain("k")` with a plain string literal; a key read through a
helper function (`fn setting(c: &Config, k: &str) { c.get::<u16>(k) }`, called as `setting(c, "server.prot")`) is
a computed name at the call inside the helper, so it is counted and not checked, and its callers are not followed;
a key built with `format!` or held in a variable is the same; another type's method named `value`, `rows` or
`explain` with a string argument is reported as if it were ours.

Status: 0.0.x. Schema, layering, validation, `explain`, serde struct binding, config-file migrations and the module
interface exist, and the PersistAnt module is behind the `persistant` feature; `configurant-tui` and `configurant-gui` edit a file with review and backup, `configurant-scan` proposes values. Design:
[docs/DESIGN.md](docs/DESIGN.md); why not figment/config/clap/confy: [docs/PRIOR-ART.md](docs/PRIOR-ART.md).
Licence: MIT OR Apache-2.0.
