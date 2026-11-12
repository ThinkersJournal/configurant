// SPDX-License-Identifier: MIT OR Apache-2.0
//! The JSON endpoints (`POST /api/event`, `GET /api/view`) against a real server on a real loopback socket
//! (`docs/BROWSER-WIZARD.md`, "Checks"): the same events as the form, the same refusals, the same file.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
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

/// An address under the session path: `http://127.0.0.1:PORT/s/<sid>/<path>`.
fn at(web: &Web, sid: &str, path: &str) -> String {
    format!("{}/s/{sid}/{path}", base(web))
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// Visit the launch address and return the session segment of the path it redirects to. No sid is set.
async fn log_in(web: &Web) -> String {
    let a = finish!(
        Client::new()
            .request(Method::GET, web.launch_url())
            .send()
            .await
            .unwrap()
    );
    assert_eq!(a.status, 303);
    assert!(a.header("set-cookie").is_none(), "the launch set a cookie");
    a.header("location")
        .expect("a redirect")
        .strip_prefix("/s/")
        .and_then(|rest| rest.strip_suffix('/'))
        .expect("the session path")
        .to_owned()
}

async fn get(web: &Web, sid: &str, path: &str) -> Answer {
    let req = Client::new().request(Method::GET, at(web, sid, path));
    finish!(req.send().await.unwrap())
}

async fn post(
    web: &Web,
    sid: &str,
    path: &str,
    origin: Option<&str>,
    ctype: &str,
    body: &str,
) -> Answer {
    let mut req = Client::new()
        .request(Method::POST, at(web, sid, path))
        .header("content-type", ctype)
        .body(body.to_owned());
    if let Some(o) = origin {
        req = req.header("origin", o);
    }
    finish!(req.send().await.unwrap())
}

async fn event(web: &Web, sid: &str, body: &str) -> Answer {
    let origin = base(web);
    post(
        web,
        sid,
        "api/event",
        Some(&origin),
        "application/json",
        body,
    )
    .await
}

async fn form(web: &Web, sid: &str, body: &str) -> Answer {
    let origin = base(web);
    post(
        web,
        sid,
        "event",
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
    let sid = log_in(&web).await;
    for body in [
        "v.mode=server&action=next",
        "v.server.port=9000&action=next",
        "action=confirm",
    ] {
        form(&web, &sid, body).await;
    }
    assert!(matches!(web.finished().await.unwrap(), Outcome::Written(_)));

    let web = start(&json_file).await;
    let sid = log_in(&web).await;
    let first = get(&web, &sid, "api/view").await;
    assert_eq!(first.status, 200);
    assert!(first.json()["view"]["wizard"].is_object());
    for body in [
        r#"{"event":"set","key":"mode","text":"server"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"set","key":"server.port","text":"9000"}"#,
        r#"{"event":"next"}"#,
    ] {
        let a = event(&web, &sid, body).await;
        assert_eq!(a.status, 200, "an event step failed");
        assert!(a.json()["finished"].is_null());
    }
    // Nothing is written before the confirm.
    assert_eq!(
        std::fs::read_to_string(&json_file).unwrap(),
        "# keep me\nother = 1\n"
    );
    let done = event(&web, &sid, r#"{"event":"confirm"}"#).await;
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
async fn a_json_event_is_refused_without_the_session_the_origin_or_the_content_type() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let origin = base(&web);
    let body = r#"{"event":"set","key":"mode","text":"server"}"#;
    let json = "application/json";
    let ok = sid.as_str();
    let cases = [
        ("wrong", Some(origin.as_str()), json, 403),
        (ok, None, json, 403),
        (ok, Some("http://evil.test"), json, 403),
        (ok, Some(origin.as_str()), "text/plain", 415),
        (
            ok,
            Some(origin.as_str()),
            "application/x-www-form-urlencoded",
            415,
        ),
    ];
    for (n, (id, o, t, want)) in cases.into_iter().enumerate() {
        let a = post(&web, id, "api/event", o, t, body).await;
        assert_eq!(a.status, want, "case {n}");
    }
    assert_eq!(get(&web, "wrong", "api/view").await.status, 403);
    // Nothing moved; the control: the same event with everything right is accepted.
    let v = get(&web, &sid, "api/view").await;
    assert_eq!(v.json()["view"]["wizard"]["current"], "mode");
    assert!(
        v.json()["view"]["review"]["changes"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert_eq!(event(&web, &sid, body).await.status, 200);
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
}

#[tokio::test]
async fn a_bad_json_event_is_a_400_that_lists_the_events() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    for (n, body) in [
        "not json",
        r#"{"event":"explode"}"#,
        r#"{"event":"next","extra":1}"#,
        r#"{"event":"set","key":"mode"}"#,
    ]
    .into_iter()
    .enumerate()
    {
        let a = event(&web, &sid, body).await;
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
    let sid = log_in(&web).await;
    let bad = r#"{"event":"set","key":"server.port","text":"99999"}"#;
    let a = event(&web, &sid, bad).await;
    assert_eq!(a.status, 422);
    assert_eq!(a.json()["refused"]["key"], "server.port");
    let said = a.json()["refused"]["error"]
        .as_str()
        .is_some_and(|e| !e.is_empty());
    assert!(said, "no reason given");
    let unknown = event(&web, &sid, r#"{"event":"set","key":"nope","text":"1"}"#).await;
    assert_eq!(unknown.status, 422);
    let v = get(&web, &sid, "api/view").await.json();
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
    let sid = log_in(&web).await;
    let mut seen = vec![get(&web, &sid, "api/view").await];
    for body in [
        r#"{"event":"set","key":"api.token","text":"typed-by-hand-9"}"#,
        r#"{"event":"set","key":"mode","text":"server"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"next"}"#,
    ] {
        seen.push(event(&web, &sid, body).await);
    }
    seen.push(get(&web, &sid, "api/view").await);
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
    let sid = log_in(&web).await;
    for body in [
        r#"{"event":"set","key":"mode","text":"server"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"next"}"#,
        r#"{"event":"confirm"}"#,
    ] {
        event(&web, &sid, body).await;
    }
    let again = event(
        &web,
        &sid,
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

/// What a hand-written request got back. Only the parts that are safe to print are exposed: the status code
/// as a number, the kind of any read error, and the byte count. The body never goes into an assert message.
struct Raw {
    text: String,
    read_error: Option<std::io::ErrorKind>,
}

impl Raw {
    fn status(&self) -> Option<u16> {
        self.text.strip_prefix("HTTP/1.1 ")?.get(..3)?.parse().ok()
    }

    /// `status=Some(403) read_error=None bytes=123`: what the client received, with no body in it.
    fn summary(&self) -> String {
        format!(
            "status={:?} read_error={:?} bytes={}",
            self.status(),
            self.read_error,
            self.text.len()
        )
    }
}

/// A JSON request written by hand, so the `Host` header can be anything. The sid, `Origin`, content type and
/// body are all good, so `Host` is the only thing that can make it fail.
fn raw_api(web: &Web, method: &str, path: &str, host: &str, sid: &str) -> Raw {
    raw_to(web.addr(), &base(web), method, path, host, sid)
}

fn raw_to(addr: SocketAddr, origin: &str, method: &str, path: &str, host: &str, sid: &str) -> Raw {
    let body = r#"{"event":"set","key":"mode","text":"server"}"#;
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let head = [
        format!("{method} /s/{sid}/{path} HTTP/1.1"),
        format!("Host: {host}"),
        format!("Origin: {origin}"),
        "Content-Type: application/json".to_owned(),
        format!("Content-Length: {}", body.len()),
        "Connection: close".to_owned(),
    ]
    .join("\u{d}\u{a}");
    write!(s, "{head}\u{d}\u{a}\u{d}\u{a}{body}").unwrap();
    let mut text = String::new();
    let read_error = s.read_to_string(&mut text).err().map(|e| e.kind());
    Raw { text, read_error }
}

/// Is this a refusal? Exactly two outcomes count (PM ruling 2026-11-12, #109): a 403 that does not carry the
/// view, or a connection the OS aborted before a single byte arrived (the refusal was sent and the socket
/// closed with request bytes unread, so the reset won the race; nothing was served and nothing leaked). Any
/// other status, any bytes without a 403, any view, any other read error, or an empty close is a failure.
fn refused(got: &Raw) -> bool {
    if got.text.contains("\"view\"") {
        return false;
    }
    matches!(
        (got.status(), got.read_error, got.text.len()),
        (Some(403), _, _) | (None, Some(std::io::ErrorKind::ConnectionAborted), 0)
    )
}

/// One round of the foreign-host check on a fresh server. Returns one line per request that was not answered
/// as it must be (a foreign `Host` is a 403 that does not carry the view; the two browser names are a 200),
/// each saying what the client received, so a failure is evidence and not only a verdict.
async fn foreign_host_round() -> Vec<String> {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let port = web.addr().port();
    let routes = [("GET", "api/view"), ("POST", "api/event")];
    let foreign = [
        "evil.test".to_owned(),
        format!("evil.test:{port}"),
        format!("127.0.0.1:{}", port + 1),
    ];
    let mut bad = Vec::new();
    for (method, path) in routes {
        for (case, host) in foreign.iter().enumerate() {
            let got = raw_api(&web, method, path, host, &sid);
            let leaked = got.text.contains("\"view\"");
            if !refused(&got) {
                bad.push(format!(
                    "{method} {path} foreign case {case}: {} leaked_view={leaked}",
                    got.summary()
                ));
            }
        }
        // The controls: both names a browser uses are served.
        for (case, host) in [format!("127.0.0.1:{port}"), format!("localhost:{port}")]
            .iter()
            .enumerate()
        {
            let got = raw_api(&web, method, path, host, &sid);
            if got.status() != Some(200) {
                bad.push(format!(
                    "{method} {path} control case {case}: {}",
                    got.summary()
                ));
            }
        }
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
    bad
}

// A multi-thread runtime: `raw_api` blocks its thread on a socket while the server answers from another.
// `CONFIGURANT_FOREIGN_HOST_ROUNDS` repeats the round on a fresh server each time (the soak job in
// `.github/workflows/foreign-host-soak.yml` sets it); the default is one.
#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_host_is_refused_on_the_json_routes_even_with_everything_else_right() {
    let rounds: usize = std::env::var("CONFIGURANT_FOREIGN_HOST_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let mut failures = Vec::new();
    for round in 0..rounds {
        for line in foreign_host_round().await {
            failures.push(format!("round {round}: {line}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} bad answer(s) in {rounds} round(s): {failures:?}",
        failures.len()
    );
}

/// A server that answers every request with `reply` (bytes, or nothing) and closes: the sabotage the refusal
/// rule must catch. Returns its address; it serves one connection.
fn fake_server(reply: &'static [u8]) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut c, _)) = listener.accept() {
            // Read the whole request (it ends with the JSON body's `}`) before answering, so the client's
            // write is never met by a reset: this fake exercises the rule, not the race.
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            while !got.ends_with(b"}") {
                match c.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            let _ = c.write_all(reply);
        }
    });
    addr
}

fn asked_of(reply: &'static [u8]) -> Raw {
    raw_to(
        fake_server(reply),
        "http://127.0.0.1:1",
        "GET",
        "api/view",
        "evil.test",
        "sid",
    )
}

// The refusal rule must bite: a server that serves the view, answers 200 without it, redirects, errors, or
// closes without a word is NOT a refusal. Only a 403 (with no view) or an abort with zero bytes is.
#[test]
fn the_refusal_rule_fails_a_server_that_answers_200_or_anything_but_a_403() {
    let served = asked_of(
        b"HTTP/1.1 200 OK
Content-Length: 14
Connection: close

{\"view\":{\"a\":1}}",
    );
    assert_eq!(served.status(), Some(200));
    assert!(!refused(&served), "a 200 that carries the view");
    let ok_no_view = asked_of(
        b"HTTP/1.1 200 OK
Content-Length: 2
Connection: close

{}",
    );
    assert!(!refused(&ok_no_view), "a 200 without the view");
    let moved = asked_of(
        b"HTTP/1.1 302 Found
Location: /
Connection: close

",
    );
    assert!(!refused(&moved), "a 3xx");
    let broken = asked_of(
        b"HTTP/1.1 500 Oops
Connection: close

",
    );
    assert!(!refused(&broken), "a 5xx");
    let view_in_403 = asked_of(
        b"HTTP/1.1 403 Forbidden
Connection: close

{\"view\":1}",
    );
    assert!(!refused(&view_in_403), "a 403 that leaks the view");
    let silent = asked_of(b"");
    assert!(
        !refused(&silent),
        "a clean close with no bytes is not an abort"
    );
    let forbidden = asked_of(
        b"HTTP/1.1 403 Forbidden
Connection: close

",
    );
    assert!(refused(&forbidden), "a plain 403 is a refusal");
}

#[test]
fn the_refusal_rule_accepts_an_abort_with_zero_bytes_and_nothing_else() {
    let abort = |kind, text: &str| Raw {
        text: text.to_owned(),
        read_error: Some(kind),
    };
    use std::io::ErrorKind::{ConnectionAborted, ConnectionReset};
    assert!(refused(&abort(ConnectionAborted, "")));
    assert!(
        !refused(&abort(ConnectionReset, "")),
        "only an abort, not any read error"
    );
    assert!(
        !refused(&abort(ConnectionAborted, "HTTP/1.1 200 OK")),
        "an abort after bytes"
    );
    assert!(
        !refused(&abort(ConnectionAborted, "partial")),
        "an abort after any bytes"
    );
}

/// A refused request whose body the server does not want must still get its 403 (configurant#111, httpant#70).
/// The client writes `size` body bytes (stopping if the server closes first), then reads to the end.
fn refused_with_body(web: &Web, sid: &str, size: usize) -> (Raw, bool) {
    let mut s = TcpStream::connect(web.addr()).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let head = format!(
        "GET /s/{sid}/api/view HTTP/1.1\r\nHost: evil.test\r\nConnection: close\r\nContent-Length: {size}\r\n\r\n"
    );
    let mut wrote_all = s.write_all(head.as_bytes()).is_ok();
    let chunk = vec![b'x'; 65536];
    let mut sent = 0;
    while wrote_all && sent < size {
        let n = chunk.len().min(size - sent);
        wrote_all = s.write_all(&chunk[..n]).is_ok();
        sent += n;
    }
    let mut text = String::new();
    let read_error = s.read_to_string(&mut text).err().map(|e| e.kind());
    (Raw { text, read_error }, wrote_all)
}

// STRICT: no abort is accepted here, unlike the soak rule. These sizes are the ones that deliver the 403 today.
// A 1 MB or 5 MB unread body is reset instead (httpant#70); its acceptance test is in that issue and is added
// with httpant's fix, not ignored here.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_request_with_an_unread_body_still_gets_its_403() {
    let dir = tmp();
    let web = start(&dir.path().join("app.toml")).await;
    let sid = log_in(&web).await;
    let mut bad = Vec::new();
    for size in [50usize, 16_000] {
        for round in 0..20 {
            let (got, wrote_all) = refused_with_body(&web, &sid, size);
            if got.status() != Some(403) || got.text.contains("\"view\"") {
                bad.push(format!(
                    "size {size} round {round}: {} wrote_all={wrote_all}",
                    got.summary()
                ));
            }
        }
    }
    let _ = web.idle_timeout(Duration::from_millis(1)).finished().await;
    assert!(
        bad.is_empty(),
        "{} of 40 not a clean 403: {:?}",
        bad.len(),
        &bad[..bad.len().min(6)]
    );
}
