# configurant-web

The browser front end of [ConfigurAnt](https://github.com/ThinkersJournal/configurant): the schema wizard, served as plain HTML pages
on loopback by [HTTPAnt](https://crates.io/crates/httpant), for one visitor and one run. **Early:** the server, pages and safety
rules exist; the JSON endpoint, a ready-made program and the first-use trial do not yet. Design: `docs/BROWSER-WIZARD.md` in the repository.

It decides nothing itself. The questions, their order, validation, the secrets rule and the review all come from
`configurant::wizard::Wizard`; a form post becomes the same events the terminal and window front ends send. Nothing is written
until the review is confirmed, and then `<file>.bak` is saved first.

```rust,no_run
use configurant::{Loader, Needs, Schema};
use configurant_web::{Outcome, Setup, Web};

# async fn run() -> Result<(), configurant_web::WebError> {
let web = Web::start(|| {
    let schema = Schema::parse(r#"
project = { name = "demo", version = "1", summary = "demo", schema_version = 1 }
value = [{ key = "port", type = "int", default = 8080, description = "listen port" }]
"#).unwrap();
    let config = Loader::new(&schema).build().unwrap();
    Setup { config, modules: None, needs: Needs::default(), file: "demo.toml".into() }
})
.await?;
println!("Open this address in your browser: {}", web.launch_url());
if let Outcome::Written(path) = web.finished().await? {
    println!("saved {}", path.display());
}
# Ok(())
# }
```

Your program prints the launch address; this crate never opens a browser. The address works once: the first visit swaps its token for a
cookie. A request without the cookie, with another `Host`, or a post from another origin is refused.

Secrets: a `secret` value has no text box and never appears in a page; it comes only from its environment variable or secret source.

Licence: MIT or Apache-2.0, at your option.
