// SPDX-License-Identifier: MIT OR Apache-2.0
//! A language server that answers `initialize` and then knows nothing: every definition request gets `null`.
//! The `cli` tests start it to see what the binary does when a server cannot resolve a call. Test support
//! only (an example, so `cargo test` builds it beside the binary).

use std::io::{BufRead, Write};

fn read_message(input: &mut impl BufRead) -> Option<String> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; len?];
    input.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

/// The text of a JSON field's value, up to the next `,` or `}` (enough for a numeric or short string id).
fn field<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("\"{name}\":");
    let rest = body[body.find(&key)? + key.len()..].trim_start();
    let end = rest.find([',', '}'])?;
    Some(rest[..end].trim())
}

fn send(out: &mut impl Write, body: &str) {
    let _ = write!(out, "Content-Length: {}\r\n\r\n{body}", body.len());
    let _ = out.flush();
}

fn main() {
    // `--die` makes the server exit at once, so a test can tell that an argument reached it.
    if std::env::args().any(|a| a == "--die") {
        std::process::exit(3);
    }
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    while let Some(body) = read_message(&mut input) {
        let method = field(&body, "method").unwrap_or("").trim_matches('"');
        if method == "exit" {
            break;
        }
        // A notification has no id and wants no answer.
        let Some(id) = field(&body, "id") else {
            continue;
        };
        let result = if method == "initialize" {
            r#"{"capabilities":{"definitionProvider":true}}"#
        } else {
            "null"
        };
        send(
            &mut out,
            &format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result}}}"#),
        );
    }
}
