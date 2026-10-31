<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# The browser wizard renderer

Status: **slice (a) built** in `crates/configurant-web` (0.5.41); (b) the JSON endpoint and (c) the example, README hello path and fresh-eyes trial are not. Below is the design as written 2026-10-30, with the HTTPAnt section re-checked against the published source. ROADMAP row 29. Written 2026-10-30 by the configurant lane for the PM. Facts about
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

1. **Bind loopback only**, never `0.0.0.0`. The code binds `127.0.0.1:0` and has no parameter for another address, so
   there is nothing to refuse; the test checks that `Web::addr()` is a loopback address. Random port from the operating
   system, printed in the launch URL.
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

## What we need from HTTPAnt (verified against httpant 0.5.19, 2026-10-31)

Read from the published crate source (`httpant`, `httpant-core`, `httpant-server`, `httpant-client` 0.5.19, downloaded from crates.io) and exercised by `crates/configurant-web/tests/web.rs`, which passes over real sockets.

1. **Verified.** `Server::serve(std::net::TcpListener)` takes a listener we bind ourselves on `127.0.0.1:0`, so the port is known before the handlers are built (they need it for the `Host` and `Origin` checks); `RunningServer::addr()` reports it.
2. **Verified.** `Request` and `Response` are `http::Request<Body>` and `http::Response<Body>`; headers are read and set with the `http` crate (re-exported as `httpant::http`), and a `303` is a status plus `Location`. The client lets a test set `Cookie` and `Origin`; a `Host` other than the URL's needed a raw socket, which the test uses.
3. **Verified.** `Body::bytes(limit)` and `Body::text(limit)` stop at a limit we choose (64 KiB; a larger post is a 413 in the test), `Limits` defaults apply before a handler runs, and `RunningServer::shutdown()` waits for requests in flight (30 s default grace), so the page that says "Saved" is delivered before the server stops.
4. **Verified.** `Handler` is implemented for `Fn(Request) -> impl Future<Output = impl IntoResponse> + Clone + Send + Sync + 'static`; a `move` closure holding an `Arc` is enough. There are no extractors yet (the query string and the form body are parsed by hand in `guard.rs`), and no fallback route was needed.
5. **Verified for the default adapter.** The crate uses `Server::new()` (hyper) and no feature of `httpant`. It does not yet let the application choose another adapter; that is a small addition if asked for, and untested on the others.
6. **Verified.** `httpant::Client` (`request(method, url)`, `.header`, `.body`, `.send()`, `.text()`) is what the tests use; it does not follow redirects, which the tests rely on to read the `303`.

HTTPAnt ROADMAP row 10 ("read ConfigurAnt's API before wiring settings") means HTTPAnt may in time depend on
`configurant` for its own settings. That is not a cycle: `configurant-web` depends on `httpant` and `configurant`;
the `configurant` core depends on neither.

## Known limit: the cookie is not scoped to a port

Cookies are scoped by host, not by port, and `SameSite` compares sites without the port. The session cookie `cw` is set for
`127.0.0.1`, so a browser sends it to **any** server on `127.0.0.1`, whatever its port. If a hostile program is already
listening on another port of this machine and the person's browser is made to visit it while the wizard is open, that
program receives the cookie in the request and can replay it to the wizard's port from outside the browser, with an
`Origin` it writes itself, and so edit and confirm. (From inside a browser the replay is stopped: a page on another port has
a different `Origin`, so the post is refused, and the `HttpOnly` cookie cannot be read by script.) This is a reasoned
limit, not something a test demonstrates. It needs a hostile listener already running on the machine plus a browser visit
to it, which is why slice (a) ships with it written down. The mitigation is ROADMAP row 32: drop the cookie after the
first load and carry the session in a per-page hidden field bound to the page (or a path token), so no cookie is left for
another port to receive.

## WebHostAnt, the other direction

An application that **is** hosted (a deployed Worker with settings and secrets) could expose a "configure me" page.
That would be this same renderer running next to the app, with WebHostAnt's `AppManifest` settings as the schema
fragment (ROADMAP row 24, held on the WebHostAnt lane) and its `Target` doing the apply. It needs authentication a loopback
token cannot give and a different threat model; it is **out of scope for row 29** and is only written down so
the choice of crate boundaries here does not preclude it: the actor takes any `Schema` and any "what happens on
confirm" callback, and knows nothing of files.

## Checks (known-bad first, each shown to bite by mutation)

- Wrong or missing token, wrong `Host`, wrong or missing `Origin` on a `POST`: `403`, state unchanged (one test per rule; mutate each check out and see it fail).
- The server's address is a loopback address (there is no option to bind another).
- Every response carries the headers of rule 5; the CSP has no `'unsafe-inline'`.
- Escaping: `<script>`, a double quote, an ampersand and a very long value through every rendered string.
- Secret canary and hand-made `POST` as above, over the HTML and the JSON surfaces.
- Event parity: for a fixed script of events, the HTML flow and `POST /api/event` leave the wizard in the same state
  as calling `Wizard::apply` directly.
- Reload and back button: a reload of any step shows the same step and repeats nothing; `back` and `goto` behave as in `STEP-ENGINE`.
- After `confirm` the file is written once with `<file>.bak`, a second `confirm` is refused, and the server then stops.
- An unknown event name is `400` with the library's suggestion; the response does not echo a `text` field.

## Which mutant kills which test (slice a, 2026-10-31)

| Mutant (the check removed or weakened) | Test that fails |
|---|---|
| `Host` check always true | `a_foreign_host_is_refused_even_with_the_cookie` |
| `Origin` check always true / not run on POST | `a_post_from_another_origin_or_with_none_is_refused_and_changes_nothing` |
| session cookie check always true | `a_request_without_the_session_cookie_gets_nothing` |
| launch token not consumed | `the_launch_address_works_once_and_a_wrong_token_never` |
| `ct_eq` always true | `guard::tests::compare_needs_the_same_length_and_text` |
| `<` not escaped | `guard::tests::escaping_covers_the_five_characters` |
| secret gets a text box (`if f.secret ...` off) | `a_secret_is_never_in_any_response_and_has_no_text_box` |
| post limit 64 MiB | `a_post_body_over_the_limit_is_refused` |
| backup not written | `a_walk_to_the_review_and_a_confirm_writes_the_file_once_with_a_backup` |
| CSP loses `frame-ancestors`, or gains `unsafe-inline` | `every_response_carries_the_policy_headers` |
| `Wizard::plain` stops redacting a secret (core) | `plain_gives_the_value_in_force_without_quotes_and_never_a_secret` |
| `save_with_backup` overwrites an existing backup (core) | `save_with_backup_keeps_the_old_text_and_refuses_when_the_backup_exists` |

## The JSON endpoints (slice b)

Same server, same cookie, same `Host` and `Origin` checks, same policy headers, same actor thread; no second
implementation of any rule.

- `GET /api/view` answers `200` with `{"view": View, "refused": null, "finished": null}`. `View` is the
  library's own `Serialize` type, so a secret is already redacted in it.
- `POST /api/event` takes one `Event` as `view.rs` defines it (`{"event":"set","key":"server.port","text":"9000"}`,
  `unset`, `next`, `back`, `goto` with `step`, `confirm`) as `application/json`, and answers with the same body shape.
  A `confirm` writes the file with `save_with_backup`, exactly as the form does, and `finished` then reads
  `{"written": "<path>"}` or `{"failed": "<why>"}`.
- Statuses: `403` for a missing or wrong cookie, a missing or foreign `Origin`, or a foreign `Host`; `415` when the
  content type is not `application/json` (a cross-site form cannot send it); `413` over 64 KiB; `400` for a body that is
  not an event, with the list of events and never a quoted value; `422` when the library refused the event, with
  `refused.error` (its own message, suggestion included) and the view unchanged; `200` otherwise.
- A field the event does not take (`{"event":"next","extra":1}`) is a `400`. serde's `deny_unknown_fields` does not catch
  that for a unit variant of an internally tagged enum, so `parse_event` checks the names itself.
- A secret has no way in: `set` on a secret key is refused with `422`, and no answer holds a secret from the environment
  or one a client tried to type (`a_secret_is_never_in_a_json_answer_and_cannot_be_set_by_json`).

| Mutant | Test that fails |
|---|---|
| content-type check off | `a_json_event_is_refused_without_the_cookie_the_origin_or_the_content_type` |
| a refusal answers `200` | `a_refused_json_event_is_a_422_with_the_reason_and_nothing_moves` (and two more) |
| extra field allowed | `a_bad_json_event_is_a_400_that_lists_the_events` |
| `Host` check skipped on `/api/*`, or `check()` dropped from `api_get_view` | `a_foreign_host_is_refused_on_the_json_routes_even_with_everything_else_right` (raw socket; cookie, `Origin`, content type and body all good, so `Host` is the only reason) |

## Open questions (for the PM)

- Crate name `configurant-web` (matches `-tui`, `-gui`)? Proposed: yes.
- Does the binary open the browser itself? Proposed: **no**, it prints the launch URL; opening a program is
  an execution and belongs behind a flag in a later PR.
- Is a loopback-only first version acceptable, with the hosted direction left to a later row? Proposed: yes.
- Is running a plan from a browser ever wanted? Proposed: not in this row.
- HTTPAnt's release is the blocker. Ask the HTTPAnt lane which release will have items 1 to 6 above, and re-read its API then; this
  note is to be corrected against the real source before any code is written.
