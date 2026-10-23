# Secret sources (design; slice 1 built)

Status: design approved by the PM on 2026-10-23 (the four decisions are in section 5); nothing is implemented, and
slice 1 (the crate `configurant-secrets` and the core hook) is built in 0.2.24; no real source exists yet.

## 1. What a SOURCE is

Today a value marked `secret = true` is read from one place, its environment variable, and is refused from a file,
the command line, a default and a flag. A **source** is a second place a secret may come from: a component that,
given a **reference** (a name, never a value), returns the secret to the loader at load time.

- The schema holds the reference, not the secret: `source = "<source name>:<reference>"` on a `secret` value,
  for example `source = "vault:CLOUDFLARE_MYPROJECT_API_TOKEN"`. The reference is an identity (the vault's entry
  NAME, CIRESNAVE-EXPECTATIONS 6.12) and is not secret; it may appear in `describe`, `explain` and errors.
- A source is a trait in the separate crate `configurant-secrets` (which depends on the core), separate from
  `Module` (which validates and plans; it must not be handed a secret):

  ```rust
  pub trait SecretSource {
      fn name(&self) -> &str;                       // what a schema's `source` prefix refers to
      fn interface_version(&self) -> u32;
      fn fetch(&self, reference: &str) -> Result<SecretValue, SourceError>;
  }
  ```

  `SecretValue` is opaque: no `Debug` with the content, no `Display`, no `Clone`, no `Serialize`; it is consumed by
  the loader into the same redacted `Value` the environment path produces. `SourceError` carries the source name,
  the reference and a reason class (`NotFound`, `Denied`, `Unreachable`, `TimedOut`), never text a source received.
- Precedence for a secret (decided): environment first (an explicit override, and the path every tool already supports),
  then its source, otherwise `MissingRequired` / `NotSet` as today. File, command line and default stay refused.
- Registration is explicit, like `Modules`. Nothing is discovered, and no source is built into the core crate.
- What the core gains (small, no dependencies): the `source` schema field with its checks (S4), and one hook,
  `Loader::resolved(key -> Value)`, that takes the secrets `configurant-secrets` fetched. `configurant-secrets`
  does the fetching (`resolve(&Schema, &Sources, &env) -> resolved values`, skipping any key the environment
  already sets) and holds each fetched `SecretValue` (wiped on drop with `zeroize`) only until it is moved into
  the loader; the core's own copy is the existing redacted `Value`, which is not wiped (stated limit below).
  `Layer` gains `Source` so `explain` can say where the value came from.

## 2. How it composes

- **The vault (`with-secret`) needs no source.** It delivers a vault entry into a child process as the environment
  variable the tool expects (`vault set NAME --env VAR`). That is the supported path today and stays correct: the
  environment layer already carries it. A `vault` source would only matter for a tool that fetches without a wrapper.
- **SecretAnt** (name queued in the portfolio roadmap, not built) would ship its own adapter crate that implements
  `SecretSource`. ConfigurAnt takes no dependency on it until it exists on crates.io (CIRESNAVE rule: crates.io
  versions only); this document fixes only the trait it must meet.
- **PersistAnt** stays a `Module`. Its endpoint questions already take `env:NAME`, never a URL, because a URL may
  hold a password. A source may later supply the variable's content, by the same trait. A secret is never stored
  *in* PersistAnt data by ConfigurAnt, and a `DataTransfer` plan holds backend ids and endpoint *references*, not
  credentials.
- **Front ends** (TUI, GUI) never fetch a secret to show it. `Session::fields` still reports `<redacted>`, no text
  box opens for a secret, and the fetch happens in the loader before a front end exists.

## 3. Rules, each with a negative control

A rule is not earned until a test that should fail does fail. Every control uses a canary secret
(`hunter2-canary`) that a fake source returns, and asserts on booleans, never by formatting the secret into an
assert message (a failing assert must not print it).

| # | Rule | Negative control (must fail on the broken variant) |
|---|---|---|
| S1 | A secret from a source is never written to a file: not the config file, not the schema, not a `.bak`. | Stage and confirm an edit with a fake source loaded; the text from `Confirmed::file_text` and `schema_edit` lacks the canary. Mutate `file_text` to emit current values: the test fails. |
| S2 | Never in a command line or a child's environment. | The executed plan and a transfer job are inspected: no argv or env entry equals the canary; mutate to pass it as an argument: fails. |
| S3 | Never in `Debug`, `Display`, an error, `explain`, `help`, `describe`, a log line or a front end's screen. | The canary is pushed through each of those with a failing load, a failing validator and a serde error (the existing substring redaction); mutate `explain` to show the value: fails. |
| S4 | The schema may name a reference only on a `secret` value; `source` on a non-secret, an unknown source name, or a malformed reference is a schema error with a suggestion. | Three bad schemas each refused by name; mutate the check away: each loads. |
| S5 | A source that fails is a named error, never a silent fallback to a default or an empty value. | A fake source returning `Unreachable`, `Denied`, `TimedOut`: `load` fails with the source and reference in the error and no value; mutate to swallow the error: the load succeeds, the test fails. |
| S6 | A fetch is bounded: a source that never answers times out (as `with-secret` does for an unanswered prompt). | A fake source that blocks past the limit: the load returns `TimedOut` within the bound. |
| S7 | No cache on disk, no second copy kept by this crate: `SecretValue` has no `Clone`, no `Display` and a `Debug` that shows nothing, and `into_loader` consumes it. | The canary is pushed through `Debug` of a `SecretValue` and of `Resolved`; mutate `Debug` to show the text: fails. (The wipe on drop is `zeroize`'s and is not tested here.) |
| S8 | A reference is not a secret and is shown; a fetched value is never shown. | `explain` lists the origin `secret source `fake` (reference `...`)` with `<redacted>` as the value, and the canary appears nowhere. |
| S9 | The environment still overrides, and a key the environment sets is not even asked of its source. | `resolve` with the variable set fetches only the other secret (the fake counts its calls); a loader given both an environment and a resolved value for one key uses the environment, and `explain` lists the source below it. Mutate the order or the skip: fails. |
| S10 | The file, command line and default stay refused for a secret (unchanged). | The existing known-bad tests keep passing with a source registered. |

Stated limit, as in `with-secret` (agentlife DESIGN-REVISION-1, 254): any process running as the same user can read
what this process can. Sources narrow where a secret is stored and shown; they are not isolation from a hostile
process of the same user. A secret is held in ordinary memory (no `mlock`); `SecretValue` is wiped on drop
(`zeroize`, decision 3), but the copy inside the core's `Value` is not.

## 4. Slices, one PR each, known-bad tests first

1. The `configurant-secrets` crate (trait, `Sources`, `SecretValue`, `resolve`), the core's `source` schema field
   and `Loader::resolved` hook with the S4 checks, an in-test fake source, S1 to S3 and S5 to S10 as tests. No real
   source.
2. `explain` / `describe` / `help` show the reference and `<redacted>`; the CLI `check` reports a schema's sources
   that are not registered in this build, by name.
3. An adapter for SecretAnt, in its own crate, when SecretAnt is on crates.io.

## 5. Decisions

Taken by the PM on 2026-10-23 as technical questions (no CireSnave decision needed):

1. **DECIDED 2026-10-23: a separate crate `configurant-secrets`** that depends on the core. The core stays light and
   the work follows the one-block-one-crate rule.
2. **DECIDED 2026-10-23: environment before source.** An explicit local override wins, and the vault-wrapper path
   keeps working unchanged.
3. **DECIDED 2026-10-23: accept `zeroize`**, so `SecretValue` wipes on drop.
4. **DECIDED 2026-10-23: no `vault` source.** The `with-secret` wrapper already delivers an environment variable. A
   SecretAnt adapter comes later as its own crate, when SecretAnt exists.
