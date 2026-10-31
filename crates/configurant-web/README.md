# configurant-web

The browser front end of [ConfigurAnt](https://github.com/ThinkersJournal/configurant): the schema wizard, served as plain HTML pages
on loopback by [HTTPAnt](https://crates.io/crates/httpant), for one visitor and one run. Design: `docs/BROWSER-WIZARD.md` in the repository.

It decides nothing itself. The questions, their order, validation, the secrets rule and the review all come from
`configurant::wizard::Wizard`; a form post becomes the same events the terminal and window front ends send. Nothing is written
until the review is confirmed, and then `<file>.bak` is saved first.

## Use it

```text
cargo add configurant configurant-web
cargo add tokio --features rt-multi-thread,macros
```

```rust,no_run
use configurant::{Loader, Needs, Schema};
use configurant_web::{Outcome, Setup, Web};

#[tokio::main]
async fn main() -> Result<(), configurant_web::WebError> {
    let web = Web::start(|| {
        let schema = Schema::parse(r#"
[project]
name = "demo"
version = "1"
summary = "demo"
schema_version = 1

[[value]]
key = "port"
type = "int"
default = 8080
description = "listen port"
"#).unwrap();
        let config = Loader::new(&schema).build().unwrap();
        Setup { config, modules: None, needs: Needs::default(), file: "demo.toml".into() }
    })
    .await?;
    println!("Open this address in your browser: {}", web.launch_url());
    if let Outcome::Written(path) = web.finished().await? {
        println!("saved {}", path.display());
    }
    Ok(())
}
```

Run it, open the address it prints, answer, review, confirm: `demo.toml` now holds `port = ...`.
`file` is relative to the directory the program runs in. If the file already exists its old text is saved as `demo.toml.bak` first (a new file has nothing to back up).

A ready-made program is in the package: `cargo run --example web_wizard -- examples/demo.schema.toml --file demo.toml`
(or your own schema in place of the demo one).

Your program prints the launch address; this crate never opens a browser. The address works once: the first visit swaps its token for a
session path (`/s/<random>/`) the browser is redirected to; there is no cookie, so no other local server can receive the session. A request outside that path, with another `Host`, or a post from another origin is refused. The session address stays in the browser history for the run (design note, "Known limit").

Your own page instead: `GET /s/<session>/api/view` and `POST /s/<session>/api/event` (the session segment is in the `Location` of the launch address's redirect; the library's `Event` as `application/json`, for example
`{"event":"set","key":"port","text":"9000"}`) go through the same checks and the same wizard as the pages. A refused event is
`422` with the library's message; a body that is not an event is `400` listing the events.

Secrets: a `secret` value has no text box and never appears in a page; it comes only from its environment variable or secret source.

Licence: MIT or Apache-2.0, at your option.
