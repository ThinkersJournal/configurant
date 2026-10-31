// SPDX-License-Identifier: MIT OR Apache-2.0
//! The JSON endpoints (`POST /api/event`, `GET /api/view`) against a real server on a real loopback socket
//! (`docs/BROWSER-WIZARD.md`, "Checks"): the same events as the form, the same refusals, the same file.

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

[[value]]
key = "server.port"
type = "int"
default = 8080
min = 1
max = 65535
description = "listen port"

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
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

/// `ClientResponse` is not re-exported by `httpant`, so reading one is a macro, not a function.
macro_rules! finish {
    ($res:expr) => {{
        let res = $res;
        let status = res.status().as_u16();
        let headers = res
            .headers()
            .iter()
            .map(|(k, v)| format!("{k}: {}", v.to_str().unwrap_or("?")))
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        let body = res.text().await.unwrap();
        Answer {
            status,
            headers,
            body,
        }
    }};
}

fn base(web: &Web) -> String {
    format!("http://127.0.0.1:{}", web.addr().port())
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// Visit the launch address and return the session cookie it hands out.
async fn log_in(web: &Web) -> String {
    let a = finish!(
        Client::new()
            .request(Method::GET, web.launch_url())
            .send()
            .await
            .unwrap()
    );
    assert_eq!(a.status, 303);
    a.header("set-cookie")
        .expect("a cookie")
        .strip_prefix("cw=")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

async fn get(web: &Web, path: &str, cookie: Option<&str>) -> Answer {
    let mut req = Client::new().request(Method::GET, format!("{}{path}", base(web)));
    if let Some(c) = cookie {
        req = req.header("cookie", format!("cw={c}"));
    }
    finish!(req.send().await.unwrap())
}

async fn post(
    web: &Web,
    path: &str,
    cookie: Option<&str>,
    origin: Option<&str>,
    ctype: &str,
    body: &str,
) -> Answer {
    let mut req = Client::new()
        .request(Method::POST, format!("{}{path}", base(web)))
        .header("content-type", ctype)
        .body(body.to_owned());
    if let Some(c) = cookie {
        req = req.header("cookie", format!("cw={c}"));
    }
    if let Some(o) = origin {
        req = req.header("origin", o);
    }
    finish!(req.send().await.unwrap())
}

async fn event(web: &Web, cookie: &str, body: &str) -> Answer {
    let origin = base(web);
    post(
        web,
        "/api/event",
        Some(cookie),
        Some(&origin),
        "application/json",
        body,
    )
    .await
}

async fn form(web: &Web, cookie: &str, body: &str) -> Answer {
    let origin = base(web);
    post(
        web,
        "/event",
        Some(cookie),
        Some(&origin),
        "application/x-www-form-urlencoded",
        body,
    )
    .await
}

#[tokio::test]
async fn the_json_walk_writes_the_same_file_as_the_form_walk() {
    // The same script through both front doors; the written files must be byte for byte the same.
    let (form_dir, json_dir) = (tmp(), tmp());
    let form_file = form_dir.path().join("app.toml");
    let json_file = json_dir.path().join("app.toml");
    for f in [&form_file, &json_file] {
        std::fs::write(f, "# keep me\nother = 1\n").unwrap();
    }
    let web = start(&form_file).await;
    let cookie = log_in(&web).await;
    for body in [
        "v.mode=server&action=next",
        "v.server.port=9000&action=next",
        "action=confirm",
    ] {
        form(&web, &cookie, body).await;
    }
    assert!(matches!(web.finished().await.unwrap(), Outcome::Written(_)));

    let web = start(&json_file).await;
    let cookie = log_in(&web).await;
    let first = get(&web, "/api/view", Some(&cookie)).await;
    assert_eq!(first.status, 200);
    assert!(first.json()["view"]["wizard"].is_object());
    for body in [
        r#"{"event":"set","key":"mode","text":"server"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"set","key":"server.port","text":"9000"}"#,
        r#"{"event":"next"}"#,
    ] {
        let a = event(&web, &cookie, body).await;
        assert_eq!(a.status, 200, "an event step failed");
        assert!(a.json()["finished"].is_null());
    }
    // Nothing is written before the confirm.
    assert_eq!(
        std::fs::read_to_string(&json_file).unwrap(),
        "# keep me\nother = 1\n"
    );
    let done = event(&web, &cookie, r#"{"event":"confirm"}"#).await;
    assert_eq!(done.status, 200);
    assert!(done.json()["finished"]["written"].is_string());
    assert!(matches!(web.finished().await.unwrap(), Outcome::Written(_)));
    let (a, b) = (
        std::fs::read_to_string(&form_file).unwrap(),
        std::fs::read_to_string(&json_file).unwrap(),
    );
    assert!(a.contains("port = 9000"));
    assert_eq!(a, b, "the two front doors wrote different files");
    assert!(json_dir.path().join("app.toml.bak").exists());
}

#[tokio::test]
async fn a_json_event_is_refused_without_the_cookie_the_origin_or_the_content_type() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    let origin = base(&web);
    let body = r#"{"event":"set","key":"mode","text":"server"}"#;
    let json = "application/json";
    let cases = [
        (None, Some(origin.as_str()), json, 403),
        (Some("wrong"), Some(origin.as_str()), json, 403),
        (Some(cookie.as_str()), None, json, 403),
        (Some(cookie.as_str()), Some("http://evil.test"), json, 403),
        (
            Some(cookie.as_str()),
            Some(origin.as_str()),
            "text/plain",
            415,
        ),
        (
            Some(cookie.as_str()),
            Some(origin.as_str()),
            "application/x-www-form-urlencoded",
            415,
        ),
    ];
    for (n, (c, o, t, want)) in cases.into_iter().enumerate() {
        let a = post(&web, "/api/event", c, o, t, body).await;
        assert_eq!(a.status, want, "case {n}");
    }
    assert_eq!(get(&web, "/api/view", None).await.status, 403);
    // Nothing moved; the control: the same event with everything right is accepted.
    let v = get(&web, "/api/view", Some(&cookie)).await;
    assert_eq!(v.json()["view"]["wizard"]["current"], "mode");
    assert!(
        v.json()["view"]["review"]["changes"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert_eq!(event(&web, &cookie, body).await.status, 200);
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_bad_json_event_is_a_400_that_lists_the_events() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    for (n, body) in [
        "not json",
        r#"{"event":"explode"}"#,
        r#"{"event":"next","extra":1}"#,
        r#"{"event":"set","key":"mode"}"#,
    ]
    .into_iter()
    .enumerate()
    {
        let a = event(&web, &cookie, body).await;
        assert_eq!(a.status, 400, "case {n}");
        let listed = a.body.contains("set, unset, confirm, next, back, goto");
        assert!(listed, "case {n} gave no suggestion");
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_refused_json_event_is_a_422_with_the_reason_and_nothing_moves() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    let bad = r#"{"event":"set","key":"server.port","text":"99999"}"#;
    let a = event(&web, &cookie, bad).await;
    assert_eq!(a.status, 422);
    assert_eq!(a.json()["refused"]["key"], "server.port");
    let said = a.json()["refused"]["error"]
        .as_str()
        .is_some_and(|e| !e.is_empty());
    assert!(said, "no reason given");
    let unknown = event(&web, &cookie, r#"{"event":"set","key":"nope","text":"1"}"#).await;
    assert_eq!(unknown.status, 422);
    let v = get(&web, "/api/view", Some(&cookie)).await.json();
    assert_eq!(v["view"]["wizard"]["current"], "mode");
    let staged = v["view"]["review"]["changes"]
        .as_array()
        .is_some_and(Vec::is_empty);
    assert!(staged, "something was staged");
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_secret_is_never_in_a_json_answer_and_cannot_be_set_by_json() {
    let dir = tmp();
    let file = dir.path().join("app.toml");
    let web = start(&file).await;
    let cookie = log_in(&web).await;
    let mut seen = vec![get(&web, "/api/view", Some(&cookie)).await];
    for body in [
        r#"{"event":"set","key":"api.token","text":"typed-by-hand-9"}"#,
        r#"{"event":"set","key":"mode","text":"server"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"next"}"#,
    ] {
        seen.push(event(&web, &cookie, body).await);
    }
    seen.push(get(&web, "/api/view", Some(&cookie)).await);
    assert_eq!(seen[1].status, 422, "a secret was settable by JSON");
    for (n, a) in seen.iter().enumerate() {
        let all = a.everything();
        assert!(
            !all.contains(CANARY),
            "answer {n} held the environment secret"
        );
        assert!(
            !all.contains("typed-by-hand-9"),
            "answer {n} echoed a typed secret"
        );
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
    assert!(
        !file.exists()
            || !std::fs::read_to_string(&file)
                .unwrap()
                .contains("typed-by-hand-9")
    );
}

#[tokio::test]
async fn a_json_event_after_the_end_is_refused() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    for body in [
        r#"{"event":"set","key":"mode","text":"server"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"confirm"}"#,
    ] {
        event(&web, &cookie, body).await;
    }
    let again = event(
        &web,
        &cookie,
        r#"{"event":"set","key":"mode","text":"client"}"#,
    )
    .await;
    assert_eq!(again.status, 422);
    let said = again.json()["refused"]["error"]
        .as_str()
        .is_some_and(|e| e.contains("finished"));
    assert!(said);
    assert!(matches!(web.finished().await.unwrap(), Outcome::Written(_)));
}

/// A JSON request written by hand, so the `Host` header can be anything. The cookie, `Origin`, content type and
/// body are all good, so `Host` is the only thing that can make it fail.
fn raw_api(web: &Web, method: &str, path: &str, host: &str, cookie: &str) -> String {
    let body = r#"{"event":"set","key":"mode","text":"server"}"#;
    let origin = base(web);
    let mut s = TcpStream::connect(web.addr()).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let head = [
        format!("{method} {path} HTTP/1.1"),
        format!("Host: {host}"),
        format!("Cookie: cw={cookie}"),
        format!("Origin: {origin}"),
        "Content-Type: application/json".to_owned(),
        format!("Content-Length: {}", body.len()),
        "Connection: close".to_owned(),
    ]
    .join("\u{d}\u{a}");
    write!(s, "{head}\u{d}\u{a}\u{d}\u{a}{body}").unwrap();
    let mut out = String::new();
    if let Err(e) = s.read_to_string(&mut out) {
        out.push_str(&format!("[read error: {e}]"));
    }
    out
}

// A multi-thread runtime: `raw_api` blocks its thread on a socket while the server answers from another.
#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_host_is_refused_on_the_json_routes_even_with_everything_else_right() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let cookie = log_in(&web).await;
    let port = web.addr().port();
    let routes = [("GET", "/api/view"), ("POST", "/api/event")];
    let foreign = [
        "evil.test".to_owned(),
        format!("evil.test:{port}"),
        format!("127.0.0.1:{}", port + 1),
    ];
    for (method, path) in routes {
        for (case, host) in foreign.iter().enumerate() {
            let out = raw_api(&web, method, path, host, &cookie);
            let refused = out.starts_with("HTTP/1.1 403");
            assert!(
                refused,
                "{method} {path}: foreign host case {case} was not refused"
            );
            let leaked = out.contains("\"view\"");
            assert!(
                !leaked,
                "{method} {path}: foreign host case {case} got the view"
            );
        }
        // The controls: both names a browser uses are served.
        for host in [format!("127.0.0.1:{port}"), format!("localhost:{port}")] {
            let out = raw_api(&web, method, path, &host, &cookie);
            let served = out.starts_with("HTTP/1.1 200");
            assert!(served, "{method} {path}: control host was not served");
        }
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}
