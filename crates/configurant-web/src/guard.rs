// SPDX-License-Identifier: MIT OR Apache-2.0
//! What a page on loopback must defend against (`docs/BROWSER-WIZARD.md`): a request from another program or
//! another web page. Pure functions over header text, so each rule has a test that does not need a socket.

use httpant::http::header::{COOKIE, HOST, ORIGIN};
use httpant::http::{HeaderMap, HeaderName, HeaderValue};

/// The name of the cookie that holds the visitor's session secret.
pub(crate) const COOKIE_NAME: &str = "cw";

/// Compare two secrets without stopping at the first difference.
pub(crate) fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 128 random bits as 32 hex digits.
pub(crate) fn random_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn header<'a>(headers: &'a HeaderMap, name: &HeaderName) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

/// The two names a browser on this machine uses to reach the server. Any other `Host` is refused (a page that
/// rebinds its own DNS name to 127.0.0.1 sends its own name).
pub(crate) fn host_ok(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = header(headers, &HOST) else {
        return false;
    };
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

/// A `POST` must name this server as its origin. A missing `Origin` is refused: a browser always sends one with
/// a form post, so only a program that is not a browser leaves it out.
pub(crate) fn origin_ok(headers: &HeaderMap, port: u16) -> bool {
    let Some(origin) = header(headers, &ORIGIN) else {
        return false;
    };
    origin == format!("http://127.0.0.1:{port}") || origin == format!("http://localhost:{port}")
}

/// The value of cookie `name`, if the `Cookie` header has it.
pub(crate) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    header(headers, &COOKIE)?.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        (k == name).then_some(v)
    })
}

/// Decode `%XX` and `+` of a form field or query value. A malformed escape is kept as it is.
pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len()
                && hex(bytes[i + 1]).is_some()
                && hex(bytes[i + 2]).is_some() =>
            {
                out.push(hex(bytes[i + 1]).unwrap_or(0) << 4 | hex(bytes[i + 2]).unwrap_or(0));
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// `a=1&b=2` as decoded pairs, in order.
pub(crate) fn form_pairs(text: &str) -> Vec<(String, String)> {
    text.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(p), String::new()),
        })
        .collect()
}

/// The headers every response carries: no script, no outside resource, no framing, no cache, no referrer.
pub(crate) fn security_headers() -> [(HeaderName, HeaderValue); 4] {
    [
        (
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'none'; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
            ),
        ),
        (
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ),
        (
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("no-store"),
        ),
        (
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ),
    ]
}

/// HTML-escape text for an element body or a quoted attribute. The one function every string goes through.
pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_handles_plus_percent_and_a_broken_escape() {
        assert_eq!(percent_decode("a+b%20c%2Fd"), "a b c/d");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%C3%A9"), "é");
    }

    #[test]
    fn form_pairs_keep_order_and_empty_values() {
        assert_eq!(
            form_pairs("v.a=1&action=next&v.b="),
            vec![
                ("v.a".into(), "1".into()),
                ("action".into(), "next".into()),
                ("v.b".into(), String::new())
            ]
        );
    }

    #[test]
    fn escaping_covers_the_five_characters() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    #[test]
    fn compare_needs_the_same_length_and_text() {
        assert!(ct_eq("abc", "abc"));
        assert!(!ct_eq("abc", "abd"));
        assert!(!ct_eq("abc", "ab"));
    }
}
