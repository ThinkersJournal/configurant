# configurant-secretant

[SecretAnt](https://crates.io/crates/secretant) as a [ConfigurAnt](https://github.com/ThinkersJournal/configurant)
secret source. A schema value marked `secret = true` may say `source = "secretant:<name>"`; this crate reads `<name>`
from a SecretAnt store when the configuration loads and hands it to `Loader::resolved` (through `Resolved::into_loader`), where it is checked and
redacted like any secret. The environment still overrides it. Design: `docs/SECRET-SOURCES.md`.

```rust,no_run
# use std::{collections::BTreeMap, path::PathBuf, time::Duration};
# use configurant::{Loader, Schema};
# use configurant_secretant::SecretAntSource;
# use configurant_secrets::{Sources, resolve};
# use secretant::{ReadConfig, ReadUnlock};
# fn demo(schema: &Schema, path: PathBuf, env: BTreeMap<String, String>) -> Result<(), String> {
let reader = ReadConfig { path, unlock: ReadUnlock::Os, timeout: Duration::from_secs(10) };
let mut sources = Sources::new();
sources.register(Box::new(SecretAntSource::open(reader).map_err(|e| format!("{e:?}"))?)).map_err(|e| format!("{e:?}"))?;
let resolved = resolve(schema, &sources, &env, Duration::from_secs(10)).map_err(|e| format!("{e:?}"))?;
let loader = resolved.into_loader(Loader::new(schema).env(env));
# Ok(())
# }
```

The store path and how to unlock it are the application's, never the schema's, so a schema holds no machine path
and no passphrase. A failure is a kind only (`NotFound`, `Denied`, `Unreachable`, `Invalid`, `TimedOut`): nothing the
store holds reaches an error. An entry that is not UTF-8 text is `Invalid`; each fetch reads the store file afresh,
so a rotated value (`secretant rotate NAME`, a new version of an existing entry) is seen on the next load.
The tests do that as a `put` on an existing name, the call the CLI makes; the CLI itself is not run by them. The snippet above is compile-checked
(`no_run`: `ReadUnlock::Os` needs the OS keyring, so it is not executed).

Limit: any process running as the same user can do what this adapter does (SecretAnt's own stated limit).

Licensed under MIT OR Apache-2.0.
