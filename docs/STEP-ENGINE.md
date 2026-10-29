# Step engine

Status: **design, nothing built** (ROADMAP row 26). Written 2026-10-29. Facts about today's code are from
`src/view.rs` and `src/schema.rs` at 0.5.12; the rest is proposed.

## What it is for

An installer or first-run asks a stranger a handful of questions in a sensible order, hides the ones that do not
apply, lets them go back, and shows a review before anything is written. Today `view::Driver` serves a whole
settings page: every field, one review. The step engine is the same thing as a **sequence**: one engine, driven by
the same JSON events, so a terminal, a window and a browser page all ask the same questions in the same order and
none of them re-implements the order.

## What exists today

`Driver::view()` returns fields (with prompt, example, hint, choices), `sections`, `file_only` and the review;
`Driver::apply(Event)` takes `set`, `unset`, `confirm`. Nothing orders the fields, nothing hides one, nothing
remembers where the person is.

## Proposed

1. **Steps are schema data.** Optional `[[step]]` entries after the values:

   ```toml
   [[step]]
   id = "network"
   title = "Where should it listen?"
   intro = "Pick the port the app answers on. Most people keep the default."
   keys = ["server.port", "server.host"]
   when = { key = "mode", equals = "server" }     # optional
   ```

   A step shows its keys' prompts, examples and hints (rows 20, 23). `intro` is plain sentences, no markup.

2. **No steps declared means one step per section.** The row-23 sections already group the keys, so a schema with
   no `[[step]]` still gets a wizard with no extra work from its author. Keys in no declared step go to a final
   step, "More settings", that a renderer may fold (advanced keys are listed last).

3. **`when` is the only logic, and it is data.** `{ key, equals }`, `{ key, not_equals }` or `{ key, set = true }`,
   read from the staged value if any, else the current one. No expressions, no scripts. A hidden step is skipped
   by `next` and `back`; its keys keep whatever they hold and are not asked.

4. **Events grow by three:** `next`, `back`, `goto { step }` (only to a step already reached). `next` is refused,
   with the named error and the key, while a key in the current step is `required` and unset or has a refused
   staged value; the person stays where they are. The last `next` lands on the review, whose only exits are `back`
   and `confirm`.

5. **The view says where you are.** `View.wizard`: `steps[]` (id, title, state `done | current | todo | hidden`),
   `current`, `can_back`, `can_next`, and the current step's `intro` and `keys`. A renderer that ignores `wizard`
   still works as a settings page.

6. **Resume.** `WizardState { current, reached }` is serialisable; `Wizard::resume(session, state)` puts a person
   back, so an installer that is interrupted (or a browser reload) continues. It holds step ids only, never a
   value: staged values live in the session as today, and a host that wants them across a restart saves the
   session's own staged changes (a separate row, not this one).

7. **Where it lives.** A `Wizard` wraps a `Driver` (`configurant::wizard`), adds nothing to `Session`, and writes
   nothing: `confirm` and `take_confirmed` are the driver's. The TUI, GUI and browser renderers adopt it after it
   exists; none of them is part of this row.

## Checks (known-bad first)

- `next` past a required key that is unset, and past one with a refused value: refused, stays.
- A hidden step is skipped both ways; revealing it by changing its `when` key makes `next` stop there.
- `goto` to a step not yet reached is refused; an unknown step id is refused with a suggestion.
- A `[[step]]` naming an unknown key, naming a key twice, with an empty `title`, or a `when` on an unknown key is a
  `SchemaError`. A secret key in a step is shown as "set by environment or secret source", never asked.
- No secret value in any `wizard` JSON (canary), and no `[[step]]` text reaches an error as a value.
- Resume with a stale step id (the schema changed) lands on the first step with a note, not a panic.

## Size and needs

L, in two PRs: (a) schema `[[step]]` plus `when` parsing and validation, and the pure ordering function with its
tests; (b) `Wizard`, the events, `View.wizard`, resume. Needs nothing from another Ant. Rows 28 (plan executor) and
29 (browser renderer) build on it.

## Open questions (for the PM)

- Should `when` allow `all`/`any` of several conditions in the first version? Proposed: no; one condition, and a
  second need is a reason to add `all`.
- A step with `keys` all secret (an API token step): show it as a note and skip, or stop and tell the person to
  set the variable first? Proposed: show the note and let `next` pass; the installer's own preflight checks the
  secret.
