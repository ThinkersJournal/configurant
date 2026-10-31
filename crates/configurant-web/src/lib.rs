// SPDX-License-Identifier: MIT OR Apache-2.0
//! The browser front end of ConfigurAnt: the wizard of `configurant::wizard`, served as plain HTML pages on
//! loopback by HTTPAnt, for one visitor and one run. Design and the reasons for every rule below:
//! `docs/BROWSER-WIZARD.md`.
//!
//! The pages decide nothing. Every question, its order, its validation, the secrets rule and the review come from
//! the library's `Wizard`; a form post becomes the same events a terminal or a window sends. Nothing is written
//! until the person has read the review and pressed the button, and then the file is written the way the other
//! front ends write it: `<file>.bak` first (refused if it exists), then the file.
//!
//! What a page on loopback must still defend against (any program, and any web page open in the person's
//! browser, can send requests to `127.0.0.1`):
//!
//! - the server binds `127.0.0.1` on a port the system picks, never anything else;
//! - the launch address carries a one-time token that is swapped for a session cookie
//!   (`HttpOnly; SameSite=Strict`); a request without the cookie is refused;
//! - a `Host` other than `127.0.0.1:PORT` or `localhost:PORT` is refused (DNS rebinding);
//! - a `POST` whose `Origin` is not this server is refused (cross-site forms);
//! - every response says: no script, no outside resource, no framing, no cache, no referrer;
//! - a secret has no text box, is never in a page, and a hand-made post for one is refused.
//!
//! The binary or program that starts this prints the launch address; it does not open a browser.
//!
//! ```no_run
//! use configurant::{Loader, Needs, Schema};
//! use configurant_web::{Outcome, Setup, Web};
//!
//! # async fn run() -> Result<(), configurant_web::WebError> {
//! let web = Web::start(|| {
//!     let schema = Schema::parse(r#"
//! project = { name = "demo", version = "1", summary = "demo", schema_version = 1 }
//! value = [{ key = "port", type = "int", default = 8080, description = "listen port" }]
//! "#).unwrap();
//!     let config = Loader::new(&schema).build().unwrap();
//!     Setup { config, modules: None, needs: Needs::default(), file: "demo.toml".into() }
//! })
//! .await?;
//! println!("Open this address in your browser: {}", web.launch_url());
//! match web.finished().await? {
//!     Outcome::Written(path) => println!("saved {}", path.display()),
//!     other => println!("not saved: {other:?}"),
//! }
//! # Ok(())
//! # }
//! ```

mod actor;
mod guard;
mod page;

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use httpant::http::header::{CONTENT_TYPE, LOCATION, SET_COOKIE};
use httpant::http::{Method, StatusCode};
use httpant::{Body, IntoResponse, Request, Response, RunningServer, Server};
use tokio::sync::oneshot;

pub use actor::Setup;
use actor::{Action, Msg, Snapshot, Submit};

/// A text box post is small; anything bigger is not from our page.
const MAX_POST: usize = 64 * 1024;

/// How the run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// The person confirmed and the file was written (or created).
    Written(PathBuf),
    /// The person confirmed and the file could not be written; the reason is the text.
    Failed(String),
    /// Nobody confirmed: the wizard sat idle for longer than the idle timeout.
    Abandoned,
}

/// Why the wizard could not start.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WebError {
    #[error("cannot listen on 127.0.0.1: {0}")]
    Bind(String),
    #[error("cannot make a launch token: {0}")]
    Token(String),
    #[error("the web server failed: {0}")]
    Server(String),
}

struct Shared {
    port: u16,
    /// The one-time token in the launch address; `None` once it has been swapped for a cookie.
    launch: Mutex<Option<String>>,
    /// The cookie value that proves a request comes from the browser that used the launch address.
    session: String,
    tx: mpsc::Sender<Msg>,
    /// A refusal to show on the next page load, once.
    flash: Mutex<Option<String>>,
    last: Mutex<Instant>,
}

/// A running wizard server. Print [`Web::launch_url`], then [`Web::finished`] waits for the end.
pub struct Web {
    server: RunningServer,
    launch_url: String,
    done: oneshot::Receiver<Outcome>,
    shared: Arc<Shared>,
    idle: Duration,
}

impl Web {
    /// Start the wizard. `build` runs on the wizard's own thread and returns what to ask about, so the
    /// configuration and the modules never cross threads. Returns once the server is listening.
    pub async fn start(build: impl FnOnce() -> Setup + Send + 'static) -> Result<Web, WebError> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| WebError::Bind(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| WebError::Bind(e.to_string()))?
            .port();
        let token = guard::random_token().map_err(|e| WebError::Token(e.to_string()))?;
        let session = guard::random_token().map_err(|e| WebError::Token(e.to_string()))?;
        let (tx, rx) = mpsc::channel();
        let (done_tx, done) = oneshot::channel();
        std::thread::spawn(move || actor::run(build, rx, done_tx));
        let shared = Arc::new(Shared {
            port,
            launch: Mutex::new(Some(token.clone())),
            session,
            tx,
            flash: Mutex::new(None),
            last: Mutex::new(Instant::now()),
        });
        let (home, event, css) = (shared.clone(), shared.clone(), shared.clone());
        let server = Server::new()
            .get("/", move |req: Request| {
                let s = home.clone();
                async move { secure(get_home(s, req).await) }
            })
            .post("/event", move |req: Request| {
                let s = event.clone();
                async move { secure(post_event(s, req).await) }
            })
            .get("/style.css", move |req: Request| {
                let s = css.clone();
                async move { secure(get_css(s, req)) }
            })
            .serve(listener)
            .await
            .map_err(|e| WebError::Server(e.to_string()))?;
        Ok(Web {
            server,
            launch_url: format!("http://127.0.0.1:{port}/?t={token}"),
            done,
            shared,
            idle: Duration::from_secs(30 * 60),
        })
    }

    /// The address to open in a browser. It works once: the first visit swaps its token for a cookie.
    pub fn launch_url(&self) -> &str {
        &self.launch_url
    }

    /// The address the server listens on (always loopback).
    pub fn addr(&self) -> std::net::SocketAddr {
        self.server.addr()
    }

    /// Stop after this long without a request (default 30 minutes).
    pub fn idle_timeout(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// Wait until the file is written (or cannot be) or the wizard sits idle, then stop the server.
    pub async fn finished(self) -> Result<Outcome, WebError> {
        let Web {
            server,
            mut done,
            shared,
            idle,
            ..
        } = self;
        let outcome = loop {
            tokio::select! {
                got = &mut done => break got.unwrap_or(Outcome::Abandoned),
                () = tokio::time::sleep(Duration::from_millis(200)) => {
                    let quiet = shared.last.lock().map(|t| t.elapsed()).unwrap_or_default();
                    if quiet >= idle {
                        break Outcome::Abandoned;
                    }
                }
            }
        };
        server
            .shutdown()
            .await
            .map_err(|e| WebError::Server(e.to_string()))?;
        Ok(outcome)
    }
}

/// Add the headers every response carries.
fn secure(mut res: Response) -> Response {
    for (name, value) in guard::security_headers() {
        res.headers_mut().insert(name, value);
    }
    res
}

fn refuse(status: StatusCode, why: &str) -> Response {
    (status, why.to_owned()).into_response()
}

fn touch(s: &Shared) {
    if let Ok(mut last) = s.last.lock() {
        *last = Instant::now();
    }
}

/// The checks every request passes: a known `Host`, and for a post an `Origin` that is this server.
fn check(s: &Shared, req: &Request) -> Option<Response> {
    if !guard::host_ok(req.headers(), s.port) {
        return Some(refuse(
            StatusCode::FORBIDDEN,
            "this server answers only to 127.0.0.1 and localhost",
        ));
    }
    if req.method() == Method::POST && !guard::origin_ok(req.headers(), s.port) {
        return Some(refuse(
            StatusCode::FORBIDDEN,
            "a form post must come from this page",
        ));
    }
    None
}

fn has_session(s: &Shared, req: &Request) -> bool {
    guard::cookie(req.headers(), guard::COOKIE_NAME).is_some_and(|c| guard::ct_eq(c, &s.session))
}

fn see_other(to: &str) -> Response {
    let mut res = Response::new(Body::empty());
    *res.status_mut() = StatusCode::SEE_OTHER;
    if let Ok(v) = to.parse() {
        res.headers_mut().insert(LOCATION, v);
    }
    res
}

fn html(text: String) -> Response {
    let mut res = Response::new(Body::from_bytes(text));
    res.headers_mut().insert(
        CONTENT_TYPE,
        httpant::http::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    res
}

fn get_css(s: Arc<Shared>, req: Request) -> Response {
    if let Some(res) = check(&s, &req) {
        return res;
    }
    if !has_session(&s, &req) {
        return refuse(StatusCode::FORBIDDEN, "open the launch address first");
    }
    let mut res = Response::new(Body::from_bytes(page::CSS));
    res.headers_mut().insert(
        CONTENT_TYPE,
        httpant::http::HeaderValue::from_static("text/css; charset=utf-8"),
    );
    res
}

async fn ask(s: &Shared, msg: impl FnOnce(oneshot::Sender<Snapshot>) -> Msg) -> Option<Snapshot> {
    let (tx, rx) = oneshot::channel();
    s.tx.send(msg(tx)).ok()?;
    rx.await.ok()
}

async fn get_home(s: Arc<Shared>, req: Request) -> Response {
    if let Some(res) = check(&s, &req) {
        return res;
    }
    touch(&s);
    // The launch address: swap its one-time token for the session cookie, then drop it from the URL.
    let query = req.uri().query().unwrap_or("");
    if let Some((_, token)) = guard::form_pairs(query).into_iter().find(|(k, _)| k == "t") {
        let ok = s
            .launch
            .lock()
            .ok()
            .and_then(|mut slot| match slot.as_deref() {
                Some(expected) if guard::ct_eq(expected, &token) => slot.take(),
                _ => None,
            })
            .is_some();
        if !ok {
            return refuse(
                StatusCode::FORBIDDEN,
                "this launch address was used already or is wrong; start the wizard again",
            );
        }
        let mut res = see_other("/");
        if let Ok(v) = format!(
            "{}={}; HttpOnly; SameSite=Strict; Path=/",
            guard::COOKIE_NAME,
            s.session
        )
        .parse()
        {
            res.headers_mut().insert(SET_COOKIE, v);
        }
        return res;
    }
    if !has_session(&s, &req) {
        return refuse(
            StatusCode::FORBIDDEN,
            "open the launch address that the program printed",
        );
    }
    let Some(mut snap) = ask(&s, Msg::Page).await else {
        return refuse(StatusCode::SERVICE_UNAVAILABLE, "the wizard has stopped");
    };
    if let Some(error) = s.flash.lock().ok().and_then(|mut f| f.take()) {
        snap.refused = Some(configurant::view::Refusal {
            key: String::new(),
            error,
        });
    }
    html(page::render(&snap))
}

async fn post_event(s: Arc<Shared>, req: Request) -> Response {
    if let Some(res) = check(&s, &req) {
        return res;
    }
    if !has_session(&s, &req) {
        return refuse(StatusCode::FORBIDDEN, "open the launch address first");
    }
    touch(&s);
    let body = match req.into_body().text(MAX_POST).await {
        Ok(text) => text,
        Err(_) => {
            return refuse(StatusCode::PAYLOAD_TOO_LARGE, "that form post is too large");
        }
    };
    let mut fields = Vec::new();
    let mut action = None;
    for (name, text) in guard::form_pairs(&body) {
        if let Some(key) = name.strip_prefix("v.") {
            fields.push((key.to_owned(), text));
        } else if name == "action" {
            action = Some(text);
        }
    }
    let action = match action.as_deref() {
        Some("next") => Action::Next,
        Some("back") => Action::Back,
        Some("confirm") => Action::Confirm,
        Some(other) if other.starts_with("goto:") => {
            Action::Goto(other["goto:".len()..].to_owned())
        }
        _ => {
            return refuse(
                StatusCode::BAD_REQUEST,
                "the action must be one of: next, back, confirm, goto:STEP",
            );
        }
    };
    let Some(snap) = ask(&s, |tx| Msg::Submit(Submit { fields, action }, tx)).await else {
        return refuse(StatusCode::SERVICE_UNAVAILABLE, "the wizard has stopped");
    };
    if snap.finished.is_some() {
        return html(page::render(&snap));
    }
    if let (Some(r), Ok(mut f)) = (&snap.refused, s.flash.lock()) {
        *f = Some(r.error.clone());
    }
    see_other("/")
}
