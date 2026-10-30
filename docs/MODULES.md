# Other modules: ports, paths, logging

Status: **design, nothing built** (ROADMAP row 11, spec section 4 "Other modules"). Written 2026-10-30. Facts about
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
   here: a project that wants to allow privileged ports builds `Ports::new().allow_privileged()` and registers it.
   No new schema field in any slice.
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
| P3 | A port below 1024 is refused unless `.allow_privileged()` | `127.0.0.1:80` under the default; accepted with the option (the control for the control) |
| P4 | The error names the key and the rule and never echoes a value that is `secret` (a port is not secret, but an address string can hold a credential by mistake: `user:pw@host:80` is refused by P2 and the text is `<redacted>` if the key is secret) | a `secret` key holding `user:pw@h:80` leaks nothing in `Display`, `Debug`, `explain` |
| P5 | Two ports in one schema must not collide (see below) | `listen.addr` and `admin.addr` both `:7878` |

P5 needs what limit 1 forbids. The smallest honest change is **interface version 2**: one more default method,
`fn validate_all(&self, values: &[(&str, &Value)]) -> Vec<(String, String)>` (key, message), called once per load
with every value that names this module; the default returns nothing, so every existing module keeps working
and the interface stays versioned as the spec requires. P5 is slice 2, not slice 1.

**`list_choices`**: for a `Needs`-less value it returns the well-known alternatives only if the project asks
(`.suggest(&[8080, 8081, 8443])`); otherwise empty. It never probes.

**`on_change`**: none. A port change is a restart (`restart = true` in the schema, which already exists).

**Not a load rule, a `check` step:** "is this port free". A front end may offer it behind confirmation; a refusal
is a warning, since the check races the bind it predicts.

**First customer: `synapsed`.** `examples/synapsed.toml` declares `listen.addr` as a plain `string` whose
description says "synapsed refuses a value that is not an address": the schema cannot say so today, and the
refusal happens late, in the daemon. With `provider = "ports"` the same refusal happens at load, with a key and a
suggestion. Second customer: AntSpawn's probe addresses. Both are adopted by their own lanes (ROADMAP row 12).

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

## 7. Questions for the PM

1. Is "slice 1 = ports P1 to P4" accepted, with interface version 2 (`validate_all`) as slice 2?
2. P3 refuses privileged ports by default. Is the default right, or should the default be permissive with
   `.deny_privileged()`? (Recommend refusing: a daemon asking for port 80 on a laptop is almost always a mistake.)
3. Should the `synapse` lane be asked now to say whether it wants `listen.addr` and `log_level` checked this way, so
   slice 1 is built for a customer who has said yes?
