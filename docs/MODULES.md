# Other modules: ports, paths, logging

Status: **ports (P1 to P5) built in 0.5.25 and 0.5.27; paths and logging are design only** (ROADMAP row 11, spec section 4 "Other modules"). Written 2026-10-30. Facts about
today's code are from `src/module.rs`, `src/schema.rs` and `src/value.rs` at 0.5.23, `examples/synapsed.toml` and
`examples/antspawn.toml`; the rest is proposed.

## 1. What a module can do today, and what it cannot

A value with `provider = "x"` is checked by module `x` (`Module::validate(&Value)`), and a module can list choices
(`list_choices(&Needs)`) and return a `Plan` when a value changes (`on_change(old, new)`). A default is checked too.
Three limits shape everything below:

1. **`validate` sees one value and no key.** It cannot compare two values (two ports that collide) and cannot tell
   a listen port from a client port.
2. **A module's options are the application's, not the schema's.** The PersistAnt module's minimum durability is a
   setting on the module, because there is no schema field for it (DESIGN item 12, ROADMAP row 5). The same rule
   here: a project that wants `.allow_zero()` or `.suggest(..)` builds that on the module and registers it. The one
   exception, ruled by the PM on 2026-10-30, is privileged ports (P3): the schema key declares it, with a reason.
3. **The loader is pure.** It takes its inputs as data and touches no file and no socket. A rule that needs the
   machine (does the directory exist, is the port free) is therefore a **check run by a front end** (`configurant
   check`, `Plan` steps), never a load error. This keeps every load test deterministic and every load safe on a
   host where the path does not exist yet (an installer's whole job is to create it).

Each module is a `Module` implementation in `configurant::modules::<name>`, behind its own Cargo feature
(`ports`, `paths`, `logging`), so a project that wants none carries none. Registering it is the project's choice;
absent, the existing rule holds (defaults load, any other value is refused naming the module).

## 2. Ports (`provider = "ports"`)

**A project declares** an `int` value (`min`/`max` still work) or a `string` that is an address:

```toml
[[value]]
key = "listen.addr"
type = "string"
provider = "ports"
default = "127.0.0.1:7878"
```

**The loader validates** (pure functions of the text):

| # | Rule | Negative control (must fail to load) |
|---|---|---|
| P1 | An `int` is 1 to 65535; `0` ("any port") only with `.allow_zero()` | `port = 0`; `port = 65536`; `port = -1` |
| P2 | A `string` is `host:port` or `[v6]:port`, the port by P1, the host an IP literal or a DNS name (labels of 1 to 63 of letters, digits, `-`, no leading/trailing `-`, 253 total) | `7878` (no host), `localhost:` , `:80`, `::1:80` (unbracketed v6), `a b:80`, `-x.test:80`, `[::1]:99999` |
| P3 | A port below 1024 is refused unless the schema key declares `privileged = "<reason>"` (a non-empty reason string; the error for a refused port names the key and says how to declare it, and `explain()` and `help` show the reason for a key that declares it) | `127.0.0.1:80` without the declaration; the same value accepted with `privileged = "public web port, runs as a service"`; `privileged = ""` is a schema error |
| P4 | The error names the key and the rule and never echoes a value that is `secret` (a port is not secret, but an address string can hold a credential by mistake: `user:pw@host:80` is refused by P2 and the text is `<redacted>` if the key is secret) | a `secret` key holding `user:pw@h:80` leaks nothing in `Display`, `Debug`, `explain` |
| P5 | Two ports in one schema must not collide (see below) | `listen.addr` and `admin.addr` both `:7878` |

P5 needs what limit 1 forbids. The smallest honest change is **interface version 2** (built in 0.5.27): one more
default method, `fn validate_all(&self, values: &[(&str, &Value)]) -> Vec<(String, String)>` (key, message),
called once per load with the winning value of every key that names this module, sorted by key; the default
returns nothing. A module that speaks version 1 still registers (`MIN_INTERFACE_VERSION` is 1, `INTERFACE_VERSION`
is 2; any other version is refused). The loader attaches the origin of the winning value to each message and
redacts it for a `secret` key. **P5 as built:** two keys collide when they bind the same port and their hosts
overlap (the same host without regard to case, or either side a wildcard: `0.0.0.0`, `[::]`, or a bare `int`
port, which means every host); port `0` never collides; each key at fault gets its own error naming the other key
and never a value. A key whose winning value was already refused is not in the comparison. The interactive
`edit::Session` does not run `validate_all` yet.

**`list_choices`**: for a `Needs`-less value it returns the well-known alternatives only if the project asks
(`.suggest(&[8080, 8081, 8443])`); otherwise empty. It never probes.

**`on_change`**: none. A port change is a restart (`restart = true` in the schema, which already exists).

**Not a load rule, a `check` step:** "is this port free". A front end may offer it behind confirmation; a refusal
is a warning, since the check races the bind it predicts.

**First customer: `synapsed`.** `examples/synapsed.toml` declares `listen.addr` as a plain `string` whose
description says "synapsed refuses a value that is not an address": the schema cannot say so today, and the
refusal happens late, in the daemon. With `provider = "ports"` the same refusal happens at load, with a key and a
suggestion. Second customer: AntSpawn's probe addresses. Both are adopted by their own lanes (ROADMAP row 12).

**Using it (feature `ports`):**

```toml
# Cargo.toml:  configurant = { version = "0.5", features = ["ports"] }
```

```rust
use configurant::modules::ports::Ports;
use configurant::{Loader, Modules, Schema};

let schema = Schema::parse(r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "listen.addr"
type = "string"
default = "127.0.0.1:7920"
description = "where to listen"
env = "DEMO_LISTEN"
provider = "ports"
# privileged = "public web port, runs as a service"   # needed for any port below 1024
"#).unwrap();

let mut modules = Modules::new();
modules.register(Box::new(Ports::new())).unwrap();
let config = Loader::new(&schema).modules(&modules).build().unwrap();
assert_eq!(config.get::<String>("listen.addr").unwrap(), "127.0.0.1:7920");
```

A value that breaks a rule fails the load with an `Errors` list; `Display` of an entry reads, for
`DEMO_LISTEN=127.0.0.1:80`:

```text
`listen.addr` from environment variable `DEMO_LISTEN` was rejected by module `ports`: ports P3: a port below 1024
needs the privileges of a service; declare `privileged = "<reason>"` on `listen.addr` in the schema to allow it
```

To try it without touching the process environment, pass `.env(BTreeMap<String, String>)` to the `Loader`
(`Loader::new(&schema).modules(&modules).env(map).build()`). `privileged` goes in the `[[value]]` table, is a
non-empty string (an empty one is a schema error) and is valid only with `provider = "ports"`. The key may be an
`int` (a bare port) instead of a `string` (`host:port`); any other type is refused. A refusal names the key and
the rule (`P1` range, `P2` address shape, `P3` below 1024 without `privileged`) and never quotes the value.

**Two listeners (P5).** Give each its own `[[value]]` (with `type`, `default`, `description` and `env` as in the
example above) and `provider = "ports"`. If both end up on the same port, from any layer, the load fails with one
error per key, in key order, each naming the other key and the layer that set it (`from the schema default`,
`from environment variable ...`). `.env(..)` replaces the process environment for that load, so a variable not in
the map falls back to its default.

```rust
use std::collections::BTreeMap;
// `schema` has `listen.addr` (env LISTEN, default 127.0.0.1:7878) and `admin.addr` (env ADMIN, default 127.0.0.1:7879)
let env = BTreeMap::from([("ADMIN".to_owned(), "127.0.0.1:7878".to_owned())]);
let errors = Loader::new(&schema).modules(&modules).env(env).build().unwrap_err(); // `Errors`, public field `.0`
assert_eq!(errors.0.len(), 2);
// each: ports P5: this key binds the same port as `<the other key>`; give each listener its own port
```

## 3. Paths (`provider = "paths"`)

`type = "path"` already exists and accepts any text. The module adds the rules a path needs to be safe to use.

**A project declares** a `path` value; the application chooses the module settings: `Paths::new().directory()` or
`.file()`, `.within(<root>)` (a lexical root, for example the project's home), `.relative()` or `.absolute()`.

**The loader validates** (lexically, no filesystem access):

| # | Rule | Negative control |
|---|---|---|
| T1 | Not empty, no NUL, no control characters | `""`, `"a\0b"`, `"a\nb"` |
| T2 | With `.within(root)`: after lexical normalisation the path stays under `root`; `..` that climbs out and an absolute path outside are refused | `../etc/x`, `a/../../x`, `/etc/x`, and on Windows `C:\other`, `\\server\share` when root is on `D:` |
| T3 | `.relative()` refuses a rooted path, `.absolute()` refuses a relative one | `/x` under `.relative()`, `x` under `.absolute()` |
| T4 | Windows: no reserved device name as a component (`CON`, `NUL`, `COM1`...), no trailing dot or space, no `:` except a drive prefix | `logs/CON`, `x.`, `a:b` (the same text is accepted on Unix for the component names that are legal there: the test states the platform it asserts on) |
| T5 | A trailing separator means directory: refused for `.file()` | `out/` under `.file()` |

Symlinks and junctions are **not** resolved at load (that is filesystem access); the plan executor already
re-checks real paths with `Problem` values and keeps its own root rule (PLAN-EXECUTOR.md), and `paths` reuses
those names instead of inventing a second set.

**`on_change(old, new)`** returns a `Plan` with a `Mkdir` step for a new directory value and **never** a step that
removes or moves the old one; moving data is a transfer, which is a different, explicit command. The executor,
not the module, applies it, after confirmation.

**Not a load rule, a `check` step:** exists, is a directory/file, is writable by this user.

**First customer: `synapsed` `home.dir`** (a path where "unset means the platform default"), then AntSpawn's
`mounts.*.guest` paths, whose cross-field rules ("guest paths stay inside the work dir") are AntSpawn's own today
(`examples/antspawn.toml` header) and are T2 with `.within(work_dir)`. T2 with a root that is itself another
value needs P5's `validate_all`, so that part waits for interface version 2.

## 4. Logging (`provider = "logging"`)

**A project declares** a `string` (or `enum`) value holding a level or a filter, and optionally a `path` value for
a log file, which uses `provider = "paths"`.

**The loader validates:**

| # | Rule | Negative control |
|---|---|---|
| L1 | A level is one of `off error warn info debug trace` (case-insensitive, stored lower-case) | `verbose`, `warning`, `""` , `inf` (suggestion: `info`) |
| L2 | A filter is comma-separated `target=level` directives plus at most one bare default level, each target a Rust path (`a::b_c`) | `synapse=loud`, `=debug`, `a b=info`, `info,debug` (two defaults) |
| L3 | `.max_level(Level::Info)` refuses a level above it: a production build that must never emit `trace` (it can carry request bodies) | `trace` under `.max_level(Info)`; `info` accepted |

`list_choices` returns the levels, least to most verbose, each with a reason ("what you see: errors only").
`on_change`: none (restart or a live reload is the application's business).

The module only validates and describes; it does not install a subscriber. The application maps the validated
text to `tracing`, `log` or `env_logger` itself, so ConfigurAnt gains no logging dependency.

**First customer: `synapse`**, whose `config/example.toml` and `config/synapse-auth.toml` carry `log_level = "info"`
and whose binaries build a `tracing_subscriber` from a flag. Nothing validates that string today.

## 5. Which is slice 1: ports

| | Ports | Paths | Logging |
|---|---|---|---|
| New machinery | none: pure text parsing | platform rules (Windows names, roots), a `Plan` step | none, but a filter grammar |
| Needs interface v2 | only P5 (cut from slice 1) | only the `within` another value case | no |
| Gap in today's schema | **real**: `listen.addr` is an unchecked `string` and the daemon refuses it late | partial: `path` exists | small: an `enum` with `choices` already covers levels; only filters (L2) are new |
| Rules in slice 1 | P1 to P4 | T1 to T5 | L1 to L3 |
| Test surface | one pure function | needs a per-platform test matrix | one pure function |

**Slice 1 is ports P1 to P4** (no interface change, no filesystem, no new schema field, one feature flag, a real
first customer with a real gap). Logging is close in size, but an `enum` already gives it L1, so what slice 1 would
add is mostly L2, with less to prove. Paths is the largest: the rules differ by platform and `on_change` produces
`Plan` steps, so it comes last. Order: **ports (P1 to P4), then interface v2 with P5, then logging, then paths.**

## 6. Rules for every module

- Known-bad tests first (`tests/known_bad_modules.rs`): each row above is a test that fails to load, and the
  accepted-with-option control beside it; each is proven to bite by mutating the rule and watching it fail.
- A module error names the key, the rule number and a suggestion, and never a secret value.
- A module never acts: anything beyond validation is a `Plan`, shown and confirmed by a front end.
- Adoption in a project's schema is that project's lane's call; ConfigurAnt only ships the module.

## 7. Decisions and answers

PM, 2026-10-30, on PR #63:

1. **Accepted:** slice 1 = ports P1 to P4; interface version 2 (`validate_all`) is slice 2.
2. **Privileged ports (below 1024) are refused by default** and allowed only where the schema key declares it
   explicitly, with a reason string that the error and `explain()` show. Negative control: the same value accepted
   with the declaration and refused without (P3). This adds one optional schema field, `privileged` (a string),
   which is valid only on a value whose `provider` is `ports`; a schema that sets it elsewhere fails its own check.
3. **Ask the synapse lane** whether `synapsed`'s `listen.addr` wants this as first customer. **Answer, relayed by
   the PM on 2026-10-30: YES.** `synapsed` is the willing first customer for `ports` on `listen.addr` (loopback
   `127.0.0.1:7920`, non-privileged); it adopts the module after it ships, in a latest-deps bump PR with no behaviour
   change. Slice 1 was built on that answer (0.5.25).
