<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# The browser wizard renderer

Status: **design, nothing built** (ROADMAP row 29). Written 2026-10-30 by the configurant lane for the PM. Facts about
ConfigurAnt are from `origin/main` at 4220fa1 (0.5.36): `src/view.rs`, `src/wizard.rs`, `docs/GUI.md`,
`docs/STEP-ENGINE.md`. Facts about HTTPAnt and WebHostAnt are from their `README.md`, `ROADMAP.md` and (HTTPAnt)
`docs/HTTPANT-DESIGN.md` on `origin/main` as fetched 2026-10-30; **no HTTPAnt source was read and nothing was run**, so
every statement about its API is **UNVERIFIED** until its first release exists (the design says its own option A is
"provisional until CireSnave decides").

## What it is for

The TUI and the GUI already draw one `edit::Session`. A browser page is the third front end, for a person who has
a browser and no terminal habit: an installer says "open this address", and a plain page asks the same questions in
the same order as the other two, shows the same review, and writes the same file. The questions, the order, the
validation, the secrets rule and the review all come from `wizard::Wizard`; the page decides none of them.

Row 21 (`view::Driver`, `Event`, `Reply`, all `serde`) and row 26 (`wizard::Wizard`, `WizardView`, `WizardState`) are done
for this reason. What is missing is something that **serves** them. That is this row.

## The one fact that decides the shape

The wizard **writes a local file** (and later may run a local plan). So the page must be served **by the machine that
holds the file, on loopback, for the length of one run**: `127.0.0.1`, a random port, one visitor, stops when the wizard
confirms or is abandoned. That is an HTTP server library's job (HTTPAnt), not a hosting product's (WebHostAnt puts a
web app on a remote host and proves it is there; it has nothing to say about a page that must reach a file on the
person's own disk). WebHostAnt has a different, later use for the same renderer (see "WebHostAnt" below).

## Options

### A. A small crate `configurant-web`, server-rendered HTML forms, on HTTPAnt (recommended)

A new workspace crate beside `configurant-tui` and `configurant-gui`; the core crate stays free of any HTTP type.

- **One wizard, one actor.** `Driver<'a>` borrows a `Config`, so it cannot sit in shared state across requests. The
  crate starts one thread that builds the `Config`, `Session`, `Driver` and `Wizard` inside itself and owns them;
  handlers send it an `Event` (or a "give me the view") over a channel and get the `Reply` back. Nothing needs to
  be `Send` or `'static` except the messages, which are plain data. One launch is one wizard: this is a local
  tool for one person, not a multi-tenant service.
- **Pages without JavaScript.** `GET /` renders the current `View` (step title, intro, one labelled control per
  key from `FieldView`: prompt, example, hint, choices as a `<select>` or radios) as HTML. A button posts a form that
  becomes an `Event` (`set` plus `next`, `back`, `goto`, `confirm`); the server answers `303` to `GET /` so a reload
  never repeats an event. A refusal (`Reply.refused`) is shown above the control it names, with the library's own
  message, which already includes the suggestion. A page that works with no script, no build step and no asset
  pipeline is also the most accessible one: native controls, native focus order, native screen-reader support, none
  of which we have to prove the way `docs/GUI.md` has to.
- **A JSON endpoint on the same events** (`POST /api/event` with the `Event` JSON of `view.rs`, `GET /api/view`) for
  an application that wants its own page, and for the tests. It is the same actor call, so it cannot disagree with
  the HTML page. Second slice, not the first.
- **Resume is free.** The state lives on the server in the actor, so a browser reload or a closed tab loses nothing.
  `WizardState` (step ids only) is still what a host saves to survive a restart of the *program*.

Pros: smallest surface, no front-end toolchain, no second implementation of any rule, accessible by default,
works in any browser including one with scripts off. Cons: a full page per step (acceptable for a handful of
questions); live "as you type" validation is not offered (the error appears on the step's own submit).

### B. A single-page app (JSON API plus JavaScript that draws `View`)

The server only serves a static page and the JSON endpoint of A; the script draws the form. Pros: live updates,
no page flashes. Cons: a JavaScript or WASM build step in a crate that has none, a second place the view is drawn
(and tested), the accessibility work of A done by hand in script, and a content-security policy that must allow
the script. The cost is high and the benefit small for a question-by-question wizard. Keep as a later option
once the JSON endpoint exists; it needs no change to the core.

### C. Serve through WebHostAnt

Rejected for the first version, for the reason above: WebHostAnt deploys an app to a host (Cloudflare is its first
target) and proves the deployment from the host's own state. A wizard that edits a file on the visitor's own
machine has nothing to deploy. It stays a candidate for the **other** direction, below.

### D. Embed a desktop web view (a wry or Tauri window)

Rejected: that is the GUI row again with a heavier dependency tree (native web-view libraries and their system
components), and it needs no HTTP server at all, so it does not answer "how does the wizard serve over HTTPAnt".

## Recommendation

Option A, in three PRs: (a) the crate, the actor, the HTML pages and the safety rules below, tested over real loopback
sockets; (b) the JSON endpoint; (c) the `configurant-web` binary or example, the README hello-path and the fresh-eyes
trial. **Blocked until HTTPAnt has a published release** with what the "What we need from HTTPAnt" list asks for:
this repo takes crates.io versions only (CireSnave 2026-10-07), so no path or git dependency is possible while HTTPAnt
sits at 0.0.0 placeholders. Size: L, as the roadmap says.

## What a page on loopback must still defend against

A server on `127.0.0.1` is reachable by every other program and every **web page** open in the person's browser. A
hostile page can send a request to `http://127.0.0.1:<port>` (and, through DNS rebinding, can read the answer). The
wizard can write a file, so each of these is a rule with a known-bad test:

1. **Bind loopback only**, never `0.0.0.0`; refuse a bind address that is not loopback unless the caller passes an
   explicit, loudly named option. Random port from the operating system, printed in the launch URL.
2. **A launch token.** The launch URL carries a random 128-bit token (`http://127.0.0.1:PORT/?t=...`); the first
   request exchanges it for a cookie (`HttpOnly; SameSite=Strict; Path=/`) and redirects to a URL without it. Every other request without the
   cookie, or with a wrong token, is `403`, constant-time compared. A page on another origin cannot learn the token.
3. **`Host` check** (DNS rebinding): only `127.0.0.1:PORT` and `localhost:PORT` are accepted; any other `Host` is `403`.
4. **`Origin` check on every `POST`** (cross-site request forgery): a missing or different `Origin` is `403`. No CORS
   headers are ever sent.
5. **Response headers:** `Content-Security-Policy: default-src 'none'; style-src 'self'; form-action 'self'; frame-ancestors 'none'`,
   `Referrer-Policy: no-referrer`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`. No external font, script,
   image or analytics, ever.
6. **Limits:** a small request-body limit (HTTPAnt `ServeOptions` has body and header limits), an idle timeout after which the
   server stops, and a stop after `confirm` once the host has taken the result.
7. **Escaping:** every string from the schema (prompt, hint, description) and every echoed value is HTML-escaped by one
   function; a test feeds `<script>` and a quote through each field.

## Secrets

The same rule as the GUI. `View` already carries `<redacted>` for a secret, so the page never holds the value and has nothing
to render, put in an `autocomplete` attribute, cache, or log. **No form control is ever offered for a `secret` key**; the
step shows "set by environment or secret source" (STEP-ENGINE). Known-bad tests:

- Canary: load a real secret, walk every step and the review, and search **every response** (HTML, JSON, error
  pages, headers, the `Location` of every redirect) for the canary text: absent; for `<redacted>`: present.
- A hand-made `POST` with `set` on a secret key is refused, and the response (and the access log, if any) does not contain the posted text.
- Request bodies and query strings are **never logged**, and the launch token is not logged after it is exchanged.
- A secret never appears in a URL (no GET carries a value), so it cannot reach history, a referrer or a proxy log.

## Who executes what

Same as the TUI and GUI (`docs/GUI.md`): the page shows the review and the plan and sends `confirm`; **the crate itself
writes `<file>.bak` then the file** after `confirm`, refusing if the backup exists, exactly as the GUI binary does,
through `take_confirmed()` and `file_text(existing)`. A module `Plan`, and later an `ExecPlan` (row 28), stays data. The first
version **does not execute commands from a browser at all**: a plan is listed as NOT DONE, as the GUI lists a
`DataTransfer`, and running one is a separate decision for the PM and CireSnave, because "any web page could
cause a command to run" is a different risk class from "any web page could change one config file". The tokens and
checks above are what make even the file write safe.

## What we need from HTTPAnt (all UNVERIFIED until it is published and read)

Read against the HTTPAnt design at fetch time; each item is a question for its lane, not an assumption.

1. Bind to `127.0.0.1:0` and report the chosen address (`server.addr()` in the design's hello-world).
2. Read request headers (`Host`, `Origin`, `Cookie`) and set response headers (`Set-Cookie`, CSP, `Location`), with
   a `303` response: these are the `http` crate's own types, so they should be free.
3. A request-body size limit and a header limit (HTTPAnt ROADMAP row 2 says done) and a graceful shutdown with a
   grace period (row 3 says done), so the server can stop after `confirm` without holding the process.
4. Handlers that are async closures over shared state, so one actor handle can be cloned into each route.
5. The default (hyper) adapter is enough; this crate names no web framework and so works on whichever adapter
   the application picks with `Server::with_adapter`.
6. `httpant-client` (or the conformance raw-TCP client) for the crate's own tests, as a crates.io dev-dependency.

HTTPAnt ROADMAP row 10 ("read ConfigurAnt's API before wiring settings") means HTTPAnt may in time depend on
`configurant` for its own settings. That is not a cycle: `configurant-web` depends on `httpant` and `configurant`;
the `configurant` core depends on neither.

## WebHostAnt, the other direction

An application that **is** hosted (a deployed Worker with settings and secrets) could expose a "configure me" page.
That would be this same renderer running next to the app, with WebHostAnt's `AppManifest` settings as the schema
fragment (ROADMAP row 24, held on the WebHostAnt lane) and its `Target` doing the apply. It needs authentication a loopback
token cannot give and a different threat model; it is **out of scope for row 29** and is only written down so
the choice of crate boundaries here does not preclude it: the actor takes any `Schema` and any "what happens on
confirm" callback, and knows nothing of files.

## Checks (known-bad first, each shown to bite by mutation)

- Wrong or missing token, wrong `Host`, wrong or missing `Origin` on a `POST`: `403`, state unchanged (one test per rule; mutate each check out and see it fail).
- A bind to a non-loopback address is refused unless the explicit option is given.
- Every response carries the headers of rule 5; the CSP has no `'unsafe-inline'`.
- Escaping: `<script>`, a double quote, an ampersand and a very long value through every rendered string.
- Secret canary and hand-made `POST` as above, over the HTML and the JSON surfaces.
- Event parity: for a fixed script of events, the HTML flow and `POST /api/event` leave the wizard in the same state
  as calling `Wizard::apply` directly.
- Reload and back button: a reload of any step shows the same step and repeats nothing; `back` and `goto` behave as in `STEP-ENGINE`.
- After `confirm` the file is written once with `<file>.bak`, a second `confirm` is refused, and the server then stops.
- An unknown event name is `400` with the library's suggestion; the response does not echo a `text` field.

## Open questions (for the PM)

- Crate name `configurant-web` (matches `-tui`, `-gui`)? Proposed: yes.
- Does the binary open the browser itself? Proposed: **no**, it prints the launch URL; opening a program is
  an execution and belongs behind a flag in a later PR.
- Is a loopback-only first version acceptable, with the hosted direction left to a later row? Proposed: yes.
- Is running a plan from a browser ever wanted? Proposed: not in this row.
- HTTPAnt's release is the blocker. Ask the HTTPAnt lane which release will have items 1 to 6 above, and re-read its API then; this
  note is to be corrected against the real source before any code is written.
