# Design note: executing a `DataTransfer` (PersistAnt module)

Status: decided and built (0.2.7; wired into the TUI in 0.2.9 and the GUI after, see `TRANSFER-UI.md`).
Facts below were read at configurant `origin/main` 6fb8dd3 and PersistAnt `origin/main` (workspace 0.9.14).

## Today

`PersistantModule::on_change` returns `Plan { actions: [DataTransfer { from, to }] }` when the stored backend
id changes between two ids the build offers. A `Plan` is data; a front end shows it, and nothing copies. The
module owns no endpoints (path or URL, possibly a secret), so it cannot open a backend by itself.

## What PersistAnt gives us

- `registry::open_named(id, &Endpoint, &ProviderNeeds) -> Handle` (async). The gate is checked twice, a wrong
  endpoint kind is refused before any I/O. `Endpoint` is `None | Path | Url | Secret(ConnectionSecret)`;
  `Endpoint::url_env(name)` reads the URL from the environment only at open and never prints it.
- `Handle::kv()` is a `KvBackend`. Relevant methods: `ordered_scan_values(prefix, after, limit) -> ValuePage`
  (one consistent snapshot per page; optional, `Unsupported` by default), `batch`, `put`, `put_if_absent`,
  `ttl_remaining` (optional).
- Only the key-value kind is in `registry` handles. Blob and relational data are out of scope here.

## Proposal

**1. Where it lives.** `PersistantModule::transfer(&self, from: &Source, to: &Target, needs: &Needs) ->
Result<TransferReport, TransferError>`, in `src/persistant.rs`. The core `Module` trait is unchanged
(`INTERFACE_VERSION` stays 1): execution is a method of this module, not of the trait, so other modules are not
forced to grow one. `Source`/`Target` pair an id with a `persistant::registry::Endpoint`; the front end builds
them. The module still never reads a config file or chooses an endpoint.

**2. Algorithm** (copy, verify, then and only then the front end writes the config file):

1. Open both ends with `open_named`. Either failure stops with nothing touched.
2. Refuse a non-empty target (a count, never contents), unless the caller passes `allow_existing`, in which case
   a key already present is kept and counted as `skipped`.
3. Page the source (page size 256) and `put` each record into the target one by one, with the time to live it has
   left. (Built: `BatchOp::put` carries no TTL, so `batch` is not used.) `ordered_scan_values` is used where the
   backend has it; `memory` and `fs` (blob-based) scan keys only, so there each value is read with `get`.
4. Verify: a second scan of both sides compares key sets and values byte for byte; the report holds counts and
   the first differing *keys* (a key is not a secret; a value is never reported).
5. The source is never modified or deleted. The operator removes it by hand after the new backend has run.

Ordering with the file write matters: `edit::Confirmed` yields new file text and the plan. The front end runs the
transfer first and writes the file only on `Ok`, so a failed copy leaves the old backend still configured and its
data intact.

**3. Secrets.** An endpoint URL is a secret. Front ends take an environment variable name (`Endpoint::url_env`),
never a typed URL; `TransferError` and its `Debug` carry ids, counts and keys only. Leak tests: a postgres URL
with a password in `Endpoint::Url`, forced through every error path, must not appear in `Display`, `Debug`, the
report or the plan description.

**4. Front ends.** TUI and GUI show the plan, ask for the two endpoints (a path, or an env var name), show
counts as they go, and report the verify result. The CLI gets `configurant transfer` only if wanted; the library
method is enough for the front ends we have.

**5. Async.** `open_named` is async and configurant is synchronous. Recommend: the method is `async fn`, and a
small `transfer_blocking` wrapper behind a new feature `persistant-blocking` builds a current-thread Tokio
runtime. The core and the default features gain no runtime.

## Known-bad tests first (`tests/known_bad_persistant.rs`)

Each is written failing, then the code under it is mutated to confirm it bites.

- Non-empty target is refused and unchanged; `allow_existing` keeps its values.
- Unknown id, wrong endpoint kind (a path for a URL backend): refused before I/O, nothing created.
- Source is byte-identical after a transfer, and after one that fails midway (target made to fail on page 2).
- A failed transfer returns `Err`, so the front end's file write never runs (test the front-end ordering in
  `edit` tests with a stub module).
- Verify catches a deliberately corrupted copy (target wrapper flips one byte).
- Secret endpoint never appears in any error, `Debug` or report.
- Real round trip memory -> fs and fs -> sqlite (dev-dependency feature `registry-sqlite`), 1,000 keys, binary
  and empty values, a key needing a second page.

## Decisions (PM, 2026-10-22, all as recommended)

1. **TTL:** read `ttl_remaining` per key and `put` with it; a source that returns `Unsupported` is refused with a
   named error.
2. **Quiescence:** the application must be stopped; the verify pass reports differences; no locking.
3. **Async:** `async fn`, plus a `transfer_blocking` wrapper behind feature `persistant-blocking` (current-thread
   Tokio). The core and default features gain no runtime.
4. **Leases, blob, relational:** not copied; the report says so.
5. **Version:** this docs PR is 0.2.6 (after #8 merges at 0.2.5); the implementation is additive, the next patch,
   allocated by the PM at gate.

## As built (0.2.7)

`PersistantModule::transfer` / `transfer_blocking`, `copy_between` for two backends you already hold, and
`TransferEnd`, `TransferOptions`, `TransferReport`, `TransferError`. Tests: `tests/known_bad_transfer.rs`, each
mutation-checked. Wiring into the TUI and GUI was done afterwards (`TRANSFER-UI.md`). Not done: a `configurant transfer` command (see `ROADMAP.md`).
