// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad requests to a real server on a real loopback socket (`docs/BROWSER-WIZARD.md`, "Checks"). Each safety
//! rule has a test that a request breaking it is refused and changes nothing, and a control that a good request
//! is not.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use configurant::{Loader, Needs, Schema};
use configurant_web::{Outcome, Setup, Web};
use httpant::Client;
use httpant::http::Method;

const CANARY: &str = "canary-s3cr3t-value";

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "mode"
type = "enum"
choices = ["server", "client"]
default = "client"
description = "what to run"
prompt = "Pick <script>alert(1)</script> & \"mode\""

[[value]]
key = "server.port"
type = "int"
default = 8080
min = 1
max = 65535
description = "listen port"
hint = "Most people keep the default."
example = "9000"

[[value]]
key = "api.token"
type = "string"
secret = true
env = "API_TOKEN"
description = "a token"

[[step]]
id = "mode"
title = "What do you want to run?"
keys = ["mode"]

[[step]]
id = "net"
title = "Where should it listen?"
keys = ["server.port", "api.token"]
when = { key = "mode", equals = "server" }
"#;

async fn start(file: &Path) -> Web {
    let file = file.to_path_buf();
    Web::start(move || {
        let schema = Schema::parse(SCHEMA).unwrap();
        let env = BTreeMap::from([("API_TOKEN".to_owned(), CANARY.to_owned())]);
        let config = Loader::new(&schema).env(env).build().unwrap();
        Setup {
            config,
            modules: None,
            needs: Needs::default(),
            file,
        }
    })
    .await
    .unwrap()
}

struct Answer {
    status: u16,
    headers: String,
    body: String,
}

impl Answer {
    fn everything(&self) -> String {
        format!("{}\n{}", self.headers, self.body)
    }
    fn header(&self, name: &str) -> Option<String> {
        self.headers
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{name}: ")).map(str::to_owned))
    }
}

async fn send(method: Method, url: &str, origin: Option<&str>, body: &str) -> Answer {
    let mut req = Client::new().request(method.clone(), url);
    if let Some(o) = origin {
        req = req.header("origin", o);
    }
    if method == Method::POST {
        req = req
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body.to_owned());
    }
    let res = req.send().await.unwrap();
    let status = res.status().as_u16();
    let headers = res
        .headers()
        .iter()
        .map(|(k, v)| format!("{k}: {}", v.to_str().unwrap_or("?")))
        .collect::<Vec<_>>()
        .join("\n");
    let body = res.text().await.unwrap();
    Answer {
        status,
        headers,
        body,
    }
}

fn base(web: &Web) -> String {
    format!("http://127.0.0.1:{}", web.addr().port())
}

/// An address under the session path: `http://127.0.0.1:PORT/s/<sid>/<path>`.
fn at(web: &Web, sid: &str, path: &str) -> String {
    format!("{}/s/{sid}/{path}", base(web))
}

/// Visit the launch address and return the session segment of the path it redirects to. No cookie is set.
async fn log_in(web: &Web) -> String {
    let a = send(Method::GET, web.launch_url(), None, "").await;
    assert_eq!(a.status, 303, "{}", a.everything());
    assert!(a.header("set-cookie").is_none(), "the launch set a cookie");
    let to = a.header("location").expect("a redirect");
    to.strip_prefix("/s/")
        .and_then(|rest| rest.strip_suffix('/'))
        .expect("the session path")
        .to_owned()
}

async fn page(web: &Web, sid: &str) -> Answer {
    send(Method::GET, &at(web, sid, ""), None, "").await
}

async fn post(web: &Web, sid: &str, body: &str) -> Answer {
    let origin = base(web);
    send(Method::POST, &at(web, sid, "event"), Some(&origin), body).await
}

/// A request written by hand, so the `Host` header can be anything.
fn raw(web: &Web, host: &str, sid: &str) -> String {
    let mut s = TcpStream::connect(web.addr()).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(
        s,
        "GET /s/{sid}/ HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    if let Err(e) = s.read_to_string(&mut out) {
        out.push_str(&format!("[read error: {e}]"));
    }
    out
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[tokio::test]
async fn it_listens_on_loopback_only() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    assert!(web.addr().ip().is_loopback(), "{}", web.addr());
    assert!(web.launch_url().starts_with("http://127.0.0.1:"));
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_request_outside_the_session_path_gets_nothing() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let root = send(Method::GET, &format!("{}/", base(&web)), None, "").await;
    assert_eq!(root.status, 403);
    assert!(!root.body.contains("What do you want"));
    let wrong = page(&web, "not-the-session").await;
    assert_eq!(wrong.status, 403);
    assert!(!wrong.body.contains("What do you want"));
    // The control: the path from the launch address works.
    let sid = log_in(&web).await;
    assert_eq!(page(&web, &sid).await.status, 200);
    // A session path from an earlier run is no use against this one, even after this one's launch is used.
    let other_dir = tmp();
    let other = start(&other_dir.path().join("app.toml")).await;
    let earlier = log_in(&other).await;
    assert_ne!(earlier, sid);
    assert_eq!(page(&web, &earlier).await.status, 403);
    assert_eq!(page(&other, &sid).await.status, 403);
    assert_eq!(page(&other, &earlier).await.status, 200);
    let _ = other
        .idle_timeout(Duration::from_millis(1))
        .finished()
        .await;
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn the_old_cookie_era_routes_are_gone() {
    // 0.5.46 served these at the root behind a cookie; now only the session path serves anything.
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let origin = base(&web);
    for path in ["event", "api/event"] {
        let a = send(
            Method::POST,
            &format!("{}/{path}", base(&web)),
            Some(&origin),
            "action=next",
        )
        .await;
        assert!(matches!(a.status, 403 | 404), "POST /{path}");
    }
    for path in ["api/view", "style.css", "event"] {
        let a = send(Method::GET, &format!("{}/{path}", base(&web)), None, "").await;
        assert!(matches!(a.status, 403 | 404), "GET /{path}");
        assert!(!a.body.contains("What do you want"));
    }
    // The control: the same routes under the session path answer.
    assert_eq!(
        send(Method::GET, &at(&web, &sid, "api/view"), None, "")
            .await
            .status,
        200
    );
    assert_eq!(
        send(Method::GET, &at(&web, &sid, "style.css"), None, "")
            .await
            .status,
        200
    );
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn the_launch_address_works_once_and_a_wrong_token_never() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let wrong = send(Method::GET, &format!("{}/?t=0000", base(&web)), None, "").await;
    assert_eq!(wrong.status, 403);
    assert!(wrong.header("set-cookie").is_none());
    let sid = log_in(&web).await;
    // The same address a second time: refused, so a leaked URL in a history cannot log anyone in.
    let again = send(Method::GET, web.launch_url(), None, "").await;
    assert_eq!(again.status, 403);
    assert!(again.header("set-cookie").is_none());
    assert_eq!(page(&web, &sid).await.status, 200);
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

// A multi-thread runtime: `raw` blocks its thread on a socket while the server answers from another.
#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_host_is_refused_even_with_the_session_path() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let port = web.addr().port();
    // Failure messages carry a case number only, never the response (it holds request-derived text).
    for (case, host) in [
        "evil.test".to_owned(),
        format!("evil.test:{port}"),
        format!("127.0.0.1:{}", port + 1),
    ]
    .iter()
    .enumerate()
    {
        let out = raw(&web, host, &sid);
        let refused = out.starts_with("HTTP/1.1 403");
        assert!(refused, "foreign host case {case} was not refused");
        let leaked = out.contains("What do you want");
        assert!(!leaked, "foreign host case {case} got the page");
    }
    // The controls: both names a browser uses.
    for (case, host) in [format!("127.0.0.1:{port}"), format!("localhost:{port}")]
        .iter()
        .enumerate()
    {
        let out = raw(&web, host, &sid);
        let served = out.starts_with("HTTP/1.1 200");
        assert!(served, "control host case {case} was not served");
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_post_from_another_origin_or_with_none_is_refused_and_changes_nothing() {
    let dir = tmp();
    let file = dir.path().join("app.toml");
    let web = start(&file).await;
    let sid = log_in(&web).await;
    let url = at(&web, &sid, "event");
    let body = "v.mode=server&action=next";
    for origin in [
        None,
        Some("http://evil.test"),
        Some("http://127.0.0.1:1"),
        Some("null"),
    ] {
        let a = send(Method::POST, &url, origin, body).await;
        assert_eq!(a.status, 403, "{origin:?}: {}", a.everything());
    }
    // Nothing moved: still on the first step, still "client".
    let p = page(&web, &sid).await;
    assert!(p.body.contains("What do you want to run?"), "{}", p.body);
    assert!(
        p.body.contains("<option value=\"client\" selected>"),
        "{}",
        p.body
    );
    // The control: the same post from this page's own origin is taken.
    assert_eq!(post(&web, &sid, body).await.status, 303);
    assert!(
        page(&web, &sid)
            .await
            .body
            .contains("Where should it listen?")
    );
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn the_pages_link_relative_so_every_request_stays_under_the_session_path() {
    // A link to `/event` or `/style.css` would leave the session path (and miss the routes).
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let first = page(&web, &sid).await;
    assert!(first.body.contains("action=\"event\""));
    assert!(first.body.contains("href=\"style.css\""));
    for bad in ["=\"/", "='/"] {
        assert!(!first.body.contains(bad), "an absolute link: {bad}");
    }
    // Resolved against the page address, the form and the stylesheet are the session routes, and they answer.
    assert_eq!(
        send(Method::GET, &at(&web, &sid, "style.css"), None, "")
            .await
            .status,
        200
    );
    // The end page, which a post answers directly, links the stylesheet the same way.
    post(&web, &sid, "v.mode=server&action=next").await;
    post(&web, &sid, "v.server.port=9000&action=next").await;
    let end = post(&web, &sid, "action=confirm").await;
    assert_eq!(end.status, 200);
    assert!(end.body.contains("href=\"style.css\"") && !end.body.contains("=\"/"));
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn every_response_carries_the_policy_headers() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let answers = [
        page(&web, &sid).await,
        send(Method::GET, &at(&web, &sid, "style.css"), None, "").await,
        send(Method::GET, &format!("{}/", base(&web)), None, "").await, // a refusal
        post(&web, &sid, "action=next").await,
    ];
    for a in &answers {
        let csp = a.header("content-security-policy").unwrap_or_default();
        assert!(
            csp.contains("default-src 'none'") && csp.contains("frame-ancestors 'none'"),
            "{}",
            a.everything()
        );
        assert!(
            !csp.contains("unsafe-inline") && !csp.contains("script-src"),
            "{csp}"
        );
        assert_eq!(a.header("referrer-policy").as_deref(), Some("no-referrer"));
        assert_eq!(a.header("cache-control").as_deref(), Some("no-store"));
        assert_eq!(
            a.header("x-content-type-options").as_deref(),
            Some("nosniff")
        );
        assert!(
            a.header("access-control-allow-origin").is_none(),
            "no CORS, ever"
        );
        assert!(a.header("set-cookie").is_none(), "no cookie, ever");
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn schema_text_is_escaped_in_the_page() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let p = page(&web, &sid).await;
    assert!(!p.body.contains("<script>"), "{}", p.body);
    assert!(
        p.body
            .contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;mode&quot;"),
        "{}",
        p.body
    );
    // A value is escaped too: a refused post echoes what was typed, as text.
    post(&web, &sid, "v.mode=%3Cb%3Ex&action=next").await;
    let q = page(&web, &sid).await;
    assert!(!q.body.contains("<b>x"), "{}", q.body);
    assert!(q.body.contains("role=\"alert\""), "{}", q.body);
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_walk_to_the_review_and_a_confirm_writes_the_file_once_with_a_backup() {
    let dir = tmp();
    let file = dir.path().join("app.toml");
    std::fs::write(&file, "# keep me\nother = 1\n").unwrap();
    let web = start(&file).await;
    let sid = log_in(&web).await;
    assert!(
        page(&web, &sid)
            .await
            .body
            .contains("What do you want to run?")
    );
    assert_eq!(
        post(&web, &sid, "v.mode=server&action=next").await.status,
        303
    );
    let net = page(&web, &sid).await;
    assert!(
        net.body.contains("Where should it listen?") && net.body.contains("value=\"8080\""),
        "{}",
        net.body
    );
    assert_eq!(
        post(&web, &sid, "v.server.port=9000&action=next")
            .await
            .status,
        303
    );
    let review = page(&web, &sid).await;
    assert!(
        review.body.contains("<h1>Review</h1>") && review.body.contains("server.port"),
        "{}",
        review.body
    );
    // Nothing is written before the confirm.
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "# keep me\nother = 1\n"
    );
    let done = post(&web, &sid, "action=confirm").await;
    assert_eq!(done.status, 200);
    assert!(done.body.contains("Saved"), "{}", done.body);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("# keep me")
            && text.contains("port = 9000")
            && text.contains("mode = \"server\""),
        "{text}"
    );
    let bak = dir.path().join("app.toml.bak");
    assert_eq!(
        std::fs::read_to_string(&bak).unwrap(),
        "# keep me\nother = 1\n"
    );
    // A second confirm writes nothing more.
    post(&web, &sid, "action=confirm").await;
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    assert_eq!(web.finished().await.unwrap(), Outcome::Written(file));
}

#[tokio::test]
async fn a_bad_value_is_refused_with_the_librarys_message_and_the_step_stays() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    post(&web, &sid, "v.mode=server&action=next").await;
    let r = post(&web, &sid, "v.server.port=99999&action=next").await;
    assert_eq!(r.status, 303);
    let p = page(&web, &sid).await;
    assert!(
        p.body.contains("role=\"alert\"") && p.body.contains("Where should it listen?"),
        "{}",
        p.body
    );
    // The message is shown once.
    assert!(!page(&web, &sid).await.body.contains("role=\"alert\""));
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn an_unknown_action_is_a_400_that_lists_the_choices() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    for body in ["action=nxt", "v.mode=server", ""] {
        let a = post(&web, &sid, body).await;
        assert_eq!(a.status, 400, "{body}");
        assert!(a.body.contains("next, back, confirm"), "{}", a.body);
        assert!(!a.body.contains("nxt"), "the bad word is not echoed");
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_secret_is_never_in_any_response_and_has_no_text_box() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let mut seen = Vec::new();
    seen.push(page(&web, &sid).await);
    post(&web, &sid, "v.mode=server&action=next").await;
    let net = page(&web, &sid).await;
    assert!(net.body.contains("cannot be typed here"), "{}", net.body);
    assert!(!net.body.contains("v.api.token"), "no control for a secret");
    seen.push(net);
    // A hand-made post for the secret is refused and echoes nothing of what it carried.
    let forged = "tok-posted-canary";
    seen.push(post(&web, &sid, &format!("v.api.token={forged}&action=next")).await);
    seen.push(page(&web, &sid).await);
    post(&web, &sid, "v.server.port=9000&action=next").await;
    seen.push(page(&web, &sid).await); // the review
    seen.push(send(Method::GET, &at(&web, &sid, "style.css"), None, "").await);
    for a in &seen {
        let all = a.everything();
        assert!(!all.contains(CANARY), "the secret leaked: {all}");
        assert!(!all.contains(forged), "the posted secret was echoed: {all}");
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

/// What a raw post got back: the status line, or the fact that the server hung up on a body it would not take.
#[derive(Debug, PartialEq)]
enum Posted {
    Status(String),
    HungUp,
}

/// A post written by hand. A server that refuses a body it has not finished reading may close the socket while
/// the client is still writing, and Windows then reports a reset or abort; on that path there is no response to
/// read, so a hang-up (and only those error kinds) counts as the refusal. Any other error panics.
fn raw_post(web: &Web, sid: &str, body: &str) -> Posted {
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset};
    let origin = base(web);
    let port = web.addr().port();
    let head = [
        format!("POST /s/{sid}/event HTTP/1.1"),
        format!("Host: 127.0.0.1:{port}"),
        format!("Origin: {origin}"),
        "Content-Type: application/x-www-form-urlencoded".to_owned(),
        format!("Content-Length: {}", body.len()),
        "Connection: close".to_owned(),
    ]
    .join("\u{d}\u{a}");
    let mut s = TcpStream::connect(web.addr()).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    let hung_up =
        |e: &std::io::Error| matches!(e.kind(), BrokenPipe | ConnectionAborted | ConnectionReset);
    let sent = s
        .write_all(format!("{head}\u{d}\u{a}\u{d}\u{a}").as_bytes())
        .and_then(|()| s.write_all(body.as_bytes()));
    if let Err(e) = &sent {
        assert!(
            hung_up(e),
            "the write failed for another reason: {:?}",
            e.kind()
        );
    }
    let mut out = Vec::new();
    if let Err(e) = s.read_to_end(&mut out) {
        assert!(
            hung_up(&e),
            "the read failed for another reason: {:?}",
            e.kind()
        );
    }
    match String::from_utf8_lossy(&out).lines().next() {
        Some(line) if !line.is_empty() => Posted::Status(line.to_owned()),
        _ => Posted::HungUp,
    }
}

// A multi-thread runtime: `raw_post` blocks its thread on a socket while the server answers from another.
#[tokio::test(flavor = "multi_thread")]
async fn a_post_body_over_the_limit_is_refused() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    // Over the 64 KiB limit: refused with 413, or the server hung up before the client finished writing.
    let big = format!("v.mode={}&action=next", "a".repeat(100 * 1024));
    let refused = match raw_post(&web, &sid, &big) {
        Posted::HungUp => true,
        Posted::Status(line) => line.starts_with("HTTP/1.1 413"),
    };
    assert!(refused, "an over-limit body was not refused");
    // It changed nothing: still on the first step.
    let still = page(&web, &sid).await;
    assert!(still.body.contains("What do you want to run?"));
    // The control: a normal body on the same server is accepted (a redirect to the next page).
    let ok = raw_post(&web, &sid, "v.mode=server&action=next");
    assert_eq!(ok, Posted::Status("HTTP/1.1 303 See Other".to_owned()));
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn an_idle_wizard_stops_and_reports_abandoned() {
    let dir = tmp();
    let file = dir.path().join("app.toml");
    let web = start(&file).await.idle_timeout(Duration::from_millis(300));
    assert_eq!(web.finished().await.unwrap(), Outcome::Abandoned);
    assert!(!file.exists(), "nothing was written");
}
