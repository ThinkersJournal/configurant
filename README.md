# ConfigurAnt

Schema-first configuration for Rust projects. Declare every value once in `configurant.toml`; the
library layers **default < file < environment < command line**, validates with named errors, and
`explain`s where each value came from. **No generated code**: the schema is a plain file that tools
and humans can edit, and the application reads values through the library. **Secrets are enforced**:
environment only, never printed.

```
configurant check   examples/configurant.toml
configurant describe examples/configurant.toml
DEMO_PORT=9000 configurant explain examples/configurant.toml server.port -- --port 9100
```

Status: 0.0.x. Schema, layering, validation, `explain` and the module interface exist; the TUI/GUI,
schema migration, serde struct binding and the PersistAnt module are later. Design:
[docs/DESIGN.md](docs/DESIGN.md); why not figment/config/clap/confy: [docs/PRIOR-ART.md](docs/PRIOR-ART.md).
Licence: MIT OR Apache-2.0.
