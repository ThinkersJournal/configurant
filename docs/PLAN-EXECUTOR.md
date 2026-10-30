# Plan executor

Status: **built in 0.5.19 (types, `describe`), 0.5.20 (`run`), 0.5.21 (`undo`)**; the text below is the design they follow (ROADMAP row 28). Written 2026-10-29. Facts about today's code are from
`src/module.rs` and `src/edit.rs` at 0.5.14. The must-haves below are the Humboldt installer lane's
(`docs/INSTALLER-DESIGN.md`, Humboldt PR #148, read as a message on 2026-10-29, not from the file); the rest is proposed.

## What it is for

Today a `Plan` is a list of `Action`s, `Note` or `DataTransfer { from, to }`, produced by a module and **executed by
the front end** after the user confirms; the only action any front end can run is the PersistAnt data transfer. An
installer needs the same shape for ordinary setup: make directories, write files, run a command, bring a service
up. The executor is one library that runs a confirmed plan with the guarantees an installer needs, so each
installer does not re-implement them (and get the secret handling wrong).

## Principles (from the lane that will use it)

1. **Inert until confirmed.** Building a plan and showing it (the confirm screen) changes nothing. Execution is
   bound to the plan's hash: a plan that differs from the one the user saw is refused.
2. **No shell, no secrets in plans.** A command is an argv array, never a shell string. A secret never appears in
   an argv, a file the plan writes, a log, a state file or an error; it is passed by reference and delivered at run
   time (stdin, an environment variable, or a 0600 file).
3. **Idempotent and resumable.** Every step checks whether it is already done; a re-run skips verified steps; a
   failed step stops the run.
4. **Honest about undo.** A step is undoable or says `forward-only` (a database migration never undoes). Undo runs
   in reverse for the undoable steps, reports what it could not undo, and never drops a data store.
5. **Stays inside what it declared.** It touches only declared paths, and never overwrites a file it did not create
   without showing a diff first.

## Proposed

### Steps are data (a new action kind)

`ActionKind` gains `Step(Step)` (it is `#[non_exhaustive]`, so this is additive). A `Step` has:

- `id` (unique in the plan), `title` (plain sentence, for the confirm screen);
- `action`, one of: `Mkdir { path, mode }`; `WriteFile { path, mode, content }` where `content` is a hash of bytes the
  host supplies at execution, or a template reference, never a secret; `Run { argv, cwd, timeout, env }` where
  `env` values are literals or `SecretRef("name")`; `Service { name, up | down }`;
- `check` (an idempotence precondition the executor evaluates: path exists with the mode, file hash matches,
  a command exits 0), `verify` (a read-back after applying), and `undo` (another action or `none`, which marks the
  step forward-only).

Database setup is a `Run` step: the host's own binary migrates and seeds; the executor knows no SQL.

### One entry point, three phases

`Executor::new(plan, host)`; `host` is a trait the application implements for the parts a library cannot own
(resolving a `SecretRef`, supplying file bytes by hash, service control). Then:

1. `describe() -> Description`: serialisable data for the confirm screen. Every path and mode; each argv with secrets as
   `<redacted ref>`; the services touched; per step, **create / overwrite / unchanged**; what is irreversible; the plan
   hash. It performs no change and no write; the guarantee is tested (a filesystem snapshot before and after).
2. `run(confirmed_hash) -> Report`: refused unless `confirmed_hash` equals the plan's hash and the plan came from a
   `Confirmed`. Per step: check, apply, verify, record state. The state file holds step ids and states
   (`pending | applied | verified | failed`) and the plan hash only: no values, no argv, no secrets.
3. `undo() -> Report`: undoable steps in reverse; the report lists what it could not undo.

A re-run with the same state file skips `verified` steps and continues at the first other one.

### How it joins the rest

The wizard's review step (0.5.15) shows the config changes; the plan's `describe()` is a second panel on the same
screen, and `confirm` yields both a `Confirmed` for the file text and the plan hash for the executor. The
view-model carries `describe()` as plain data, so TUI, GUI and a browser page render it alike. The existing
`DataTransfer` action keeps running through its present path until the executor can run it as a `Run`-like step.

## Checks (known-bad first)

- `describe()` changes nothing: snapshot a temp directory before and after; a mutation that writes in `describe` fails.
- `run` with a stale or wrong hash is refused and changes nothing; so is a plan edited after `describe`.
- A secret canary placed in `SecretRef` resolution appears in no argv shown, no state file, no report, no error, no
  `Debug`; a leak test per output path.
- An overwrite of a file the plan did not create is refused without a shown diff; a path outside the declared set
  (including `..` and a symlink out) is refused.
- A failed step stops the run; the state file marks it `failed`; the next run resumes there; verified steps are not
  repeated (a counter in the test host proves it).
- Undo reverses only undoable steps, reports the forward-only ones, and refuses a step that would remove a data store.
- A shell string is not accepted anywhere: `Run` takes `argv` only.

## Size and open questions

L, in three PRs: (a) the `Step` data types, validation and `describe()` with the inert guarantee; (b) `run` with
state, hash binding, resume, and the secret path; (c) `undo` and the host trait's service control. Needs nothing
from another Ant. Rows 29 (browser renderer) and the Humboldt installer build on it.

Decided by the PM (2026-10-29), all three proposals accepted with one addition each:

- Windows and Unix differ. `mode` is advisory on Windows and the confirm screen says so; `Service` is host-implemented, so the executor never calls `sc` or `systemctl`. **Added:** a file that carries or derives from a `SecretRef` is not written on Windows unless the host reports an owner-only ACL (PersistAnt has done this for SQLite since 0.9.43); otherwise the step is refused, never silently downgraded.
- `WriteFile` content may be inline for small non-secret text, up to 64 KiB, hashed into the plan; larger content is by hash from the host. **Added:** a secret is never inline (`SecretRef` only), with a test.
- Containers are `Service`; the engine is left to the host.
