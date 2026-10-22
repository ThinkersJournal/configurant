# Design note: running a transfer from the TUI and GUI

Status: proposal; the questions at the end need an answer before building. Facts read at `origin/main` 1096b9d.

## Today

Both front ends build `Session::new(&config, None)`: no modules. A change of a `provider = "persistant"` key
therefore produces no plan at all, and the "NOT DONE: ... cannot run data transfers" line only appears if a
caller passes modules. `PersistantModule::transfer` exists (0.2.7) but needs three things a front end does not
have: the project's `Needs`, an `Endpoint` for each of the two backends, and a way to run async work.

## Proposal

**Flow.** list -> edit -> review -> confirm, as now. If the confirmed plan holds a `DataTransfer`, one more step
before anything is written:

1. *Endpoints*: ask for the source and the target endpoint (below).
2. *Copy*: run `transfer` on a worker thread (`transfer_blocking`), the UI shows "copying..." and stays
   responsive (the TUI event loop and the GUI frame loop must not block).
3. On `Ok(report)`: show the report, then write the file (`.bak` first, as now). On `Err`: show the error, **write
   nothing**, return to the list; the old backend is still configured and its data untouched.
4. A deliberate escape, never the default: `w` / "Write the file without copying" with a one-line warning ("the
   data stays in `<old>` and the new backend starts empty"). Needed for a first-time switch with nothing stored.

**Modules.** `Modules` holds `Box<dyn Module>`, so the concrete `PersistantModule` (with `transfer`) is out of
reach once registered. The front end builds two cheap `PersistantModule`s from the same settings: one registered
for choice lists and plans, one kept for `transfer`. No change to the `Module` trait.

**Needs.** There is no `Needs` field in the schema yet. For now both front ends take repeatable flags
`--require <capability>` and `--prefer <capability>` (and `--min-durability volatile|process|machine`, default
`process`, as the module). A schema field is the proper home and stays a later row.

**Endpoints.** The configuration stores only the backend id. The front end asks per side, and asks only for the
kind the registry says that backend opens from (`Listing::endpoint`): nothing for `memory`, a path for `fs`, a URL
for the database backends. A URL is a secret, so it is **never typed**: the user gives an environment variable
name (`env:APP_DB_URL`) and `Endpoint::url_env` reads it at open. A shared helper in the core,
`persistant::parse_endpoint(text, spec) -> Result<Endpoint, String>`, does the parsing so both front ends agree and
a literal `postgres://user:pw@...` is refused with a message that does not echo it. Repeatable flags
`--endpoint <id>=<path|env:NAME>` prefill the answers (scripted use); they are never echoed back in full.

**Secrets.** The transfer error and report carry ids, counts and keys only (0.2.7). The endpoints screen shows a
path as typed, an env name as typed, never a resolved URL. Leak tests for every new output path: the TUI buffer
and the GUI accessibility tree after a failed transfer with a URL in the environment.

**Tests, known-bad first.** (1) A transfer failure leaves the file unwritten and `.bak` absent. (2) A literal URL
as an endpoint answer is refused and appears nowhere on screen. (3) Escape-hatch write copies nothing. (4) The UI
stays responsive during the copy (a slow backend wrapper; a key press is handled before it ends). (5) `Needs`
flags reach the module. Each mutation-checked. Real fs -> memory transfer end to end through `App`.

## Open questions

1. **Needs from flags now, schema field later?** Recommend yes.
2. **Endpoints asked in the UI, URLs only as `env:NAME`, prefilled by `--endpoint`?** Recommend yes.
3. **Offer "write without copying"?** Recommend yes, as a deliberate key with a warning.
4. **Progress:** a spinner only (the library has no progress callback). Recommend yes; a callback later if wanted.
5. **Scope split:** core helper `parse_endpoint` + TUI first (one PR), GUI second (another), since one PR at a
   time. Recommend yes.
