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

async fn send(
    method: Method,
    url: &str,
    cookie: Option<&str>,
    origin: Option<&str>,
    body: &str,
) -> Answer {
    let mut req = Client::new().request(method.clone(), url);
    if let Some(c) = cookie {
        req = req.header("cookie", format!("cw={c}"));
    }
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

/// Visit the launch address and return the session cookie it hands out.
async fn log_in(web: &Web) -> String {
    let a = send(Method::GET, web.launch_url(), None, None, "").await;
    assert_eq!(a.status, 303, "{}", a.everything());
    let set = a.header("set-cookie").expect("a cookie");
    assert!(
        set.contains("HttpOnly") && set.contains("SameSite=Strict"),
        "{set}"
    );
    set.strip_prefix("cw=")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

async fn page(web: &Web, cookie: &str) -> Answer {
    send(
        Method::GET,
        &format!("{}/", base(web)),
        Some(cookie),
        None,
        "",
    )
    .await
}

async fn post(web: &Web, cookie: &str, body: &str) -> Answer {
    let origin = base(web);
    send(
        Method::POST,
        &format!("{}/event", base(web)),
        Some(cookie),
        Some(&origin),
        body,
    )
    .await
}

/// A request written by hand, so the `Host` header can be anything.
fn raw(web: &Web, host: &str, cookie: &str) -> String {
    let mut s = TcpStream::connect(web.addr()).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(
        s,
        "GET / HTTP/1.1\r\nHost: {host}\r\nCookie: cw={cookie}\r\nConnection: close\r\n\r\n"
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
async fn a_request_without_the_session_cookie_gets_nothing() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let a = send(Method::GET, &format!("{}/", base(&web)), None, None, "").await;
    assert_eq!(a.status, 403, "{}", a.everything());
    assert!(!a.body.contains("What do you want"));
    let wrong = page(&web, "not-the-cookie").await;
    assert_eq!(wrong.status, 403);
    // The control: the cookie from the launch address works.
    let cookie = log_in(&web).await;
    assert_eq!(page(&web, &cookie).await.status, 200);
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn the_launch_address_works_once_and_a_wrong_token_never() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let wrong = send(
        Method::GET,
        &format!("{}/?t=0000", base(&web)),
        None,
        None,
        "",
    )
    .await;
    assert_eq!(wrong.status, 403);
    assert!(wrong.header("set-cookie").is_none());
    let cookie = log_in(&web).await;
    // The same address a second time: refused, so a leaked URL in a history cannot log anyone in.
    let again = send(Method::GET, web.launch_url(), None, None, "").await;
    assert_eq!(again.status, 403);
    assert!(again.header("set-cookie").is_none());
    assert_eq!(page(&web, &cookie).await.status, 200);
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

// A multi-thread runtime: `raw` blocks its thread on a socket while the server answers from another.
#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_host_is_refused_even_with_the_cookie() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    let port = web.addr().port();
    for host in [
        "evil.test",
        &format!("evil.test:{port}"),
        &format!("127.0.0.1:{}", port + 1),
    ] {
        let out = raw(&web, host, &cookie);
        assert!(out.starts_with("HTTP/1.1 403"), "{host}: {out}");
        assert!(!out.contains("What do you want"), "{host}");
    }
    // The controls: both names a browser uses.
    for host in [format!("127.0.0.1:{port}"), format!("localhost:{port}")] {
        let out = raw(&web, &host, &cookie);
        assert!(out.starts_with("HTTP/1.1 200"), "{host}: {out}");
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_post_from_another_origin_or_with_none_is_refused_and_changes_nothing() {
    let dir = tmp();
    let file = dir.path().join("app.toml");
    let web = start(&file).await;
    let cookie = log_in(&web).await;
    let url = format!("{}/event", base(&web));
    let body = "v.mode=server&action=next";
    for origin in [
        None,
        Some("http://evil.test"),
        Some("http://127.0.0.1:1"),
        Some("null"),
    ] {
        let a = send(Method::POST, &url, Some(&cookie), origin, body).await;
        assert_eq!(a.status, 403, "{origin:?}: {}", a.everything());
    }
    // Nothing moved: still on the first step, still "client".
    let p = page(&web, &cookie).await;
    assert!(p.body.contains("What do you want to run?"), "{}", p.body);
    assert!(
        p.body.contains("<option value=\"client\" selected>"),
        "{}",
        p.body
    );
    // The control: the same post from this page's own origin is taken.
    assert_eq!(post(&web, &cookie, body).await.status, 303);
    assert!(
        page(&web, &cookie)
            .await
            .body
            .contains("Where should it listen?")
    );
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn every_response_carries_the_policy_headers() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    let url = base(&web);
    let answers = [
        page(&web, &cookie).await,
        send(
            Method::GET,
            &format!("{url}/style.css"),
            Some(&cookie),
            None,
            "",
        )
        .await,
        send(Method::GET, &format!("{url}/"), None, None, "").await, // a refusal
        post(&web, &cookie, "action=next").await,
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
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn schema_text_is_escaped_in_the_page() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    let p = page(&web, &cookie).await;
    assert!(!p.body.contains("<script>"), "{}", p.body);
    assert!(
        p.body
            .contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;mode&quot;"),
        "{}",
        p.body
    );
    // A value is escaped too: a refused post echoes what was typed, as text.
    post(&web, &cookie, "v.mode=%3Cb%3Ex&action=next").await;
    let q = page(&web, &cookie).await;
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
    let cookie = log_in(&web).await;
    assert!(
        page(&web, &cookie)
            .await
            .body
            .contains("What do you want to run?")
    );
    assert_eq!(
        post(&web, &cookie, "v.mode=server&action=next")
            .await
            .status,
        303
    );
    let net = page(&web, &cookie).await;
    assert!(
        net.body.contains("Where should it listen?") && net.body.contains("value=\"8080\""),
        "{}",
        net.body
    );
    assert_eq!(
        post(&web, &cookie, "v.server.port=9000&action=next")
            .await
            .status,
        303
    );
    let review = page(&web, &cookie).await;
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
    let done = post(&web, &cookie, "action=confirm").await;
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
    post(&web, &cookie, "action=confirm").await;
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    assert_eq!(web.finished().await.unwrap(), Outcome::Written(file));
}

#[tokio::test]
async fn a_bad_value_is_refused_with_the_librarys_message_and_the_step_stays() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    post(&web, &cookie, "v.mode=server&action=next").await;
    let r = post(&web, &cookie, "v.server.port=99999&action=next").await;
    assert_eq!(r.status, 303);
    let p = page(&web, &cookie).await;
    assert!(
        p.body.contains("role=\"alert\"") && p.body.contains("Where should it listen?"),
        "{}",
        p.body
    );
    // The message is shown once.
    assert!(!page(&web, &cookie).await.body.contains("role=\"alert\""));
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn an_unknown_action_is_a_400_that_lists_the_choices() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    for body in ["action=nxt", "v.mode=server", ""] {
        let a = post(&web, &cookie, body).await;
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
    let cookie = log_in(&web).await;
    let mut seen = Vec::new();
    seen.push(page(&web, &cookie).await);
    post(&web, &cookie, "v.mode=server&action=next").await;
    let net = page(&web, &cookie).await;
    assert!(net.body.contains("cannot be typed here"), "{}", net.body);
    assert!(!net.body.contains("v.api.token"), "no control for a secret");
    seen.push(net);
    // A hand-made post for the secret is refused and echoes nothing of what it carried.
    let forged = "tok-posted-canary";
    seen.push(post(&web, &cookie, &format!("v.api.token={forged}&action=next")).await);
    seen.push(page(&web, &cookie).await);
    post(&web, &cookie, "v.server.port=9000&action=next").await;
    seen.push(page(&web, &cookie).await); // the review
    seen.push(
        send(
            Method::GET,
            &format!("{}/style.css", base(&web)),
            Some(&cookie),
            None,
            "",
        )
        .await,
    );
    for a in &seen {
        let all = a.everything();
        assert!(!all.contains(CANARY), "the secret leaked: {all}");
        assert!(!all.contains(forged), "the posted secret was echoed: {all}");
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_post_body_over_the_limit_is_refused() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    let big = format!("v.mode={}&action=next", "a".repeat(100 * 1024));
    let a = post(&web, &cookie, &big).await;
    assert_eq!(a.status, 413, "{}", a.everything());
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
