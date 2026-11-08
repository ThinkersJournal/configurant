// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad: `Config::deserialize` must leave a table absent when nothing under it is set, so a
//! nested `Option<struct>` comes back `None`, not `Some(default)` (WebServAnt finding, 2026-11-08).

use configurant::{Loader, Schema};

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1
env_prefix = "DEMO_"

[[value]]
key = "server.port"
type = "int"
default = 8080
description = "listen port"

[[value]]
key = "tls.cert.path"
type = "string"
description = "certificate file"

[[value]]
key = "tls.cert.chain"
type = "string"
description = "chain file"
"#;

#[derive(Debug, serde::Deserialize)]
struct Settings {
    server: Server,
    tls: Option<Tls>,
}

#[derive(Debug, serde::Deserialize)]
struct Server {
    port: u16,
}

#[derive(Debug, serde::Deserialize)]
struct Tls {
    cert: Option<Cert>,
}

#[derive(Debug, serde::Deserialize)]
struct Cert {
    path: Option<String>,
    chain: Option<String>,
}

fn env(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn a_table_with_nothing_set_under_it_is_none() {
    let s = Schema::parse(SCHEMA).unwrap();
    let st: Settings = Loader::new(&s).build().unwrap().deserialize().unwrap();
    assert_eq!(st.server.port, 8080);
    assert!(st.tls.is_none());
}

#[test]
fn a_table_with_one_value_set_is_some_and_its_sibling_none() {
    let s = Schema::parse(SCHEMA).unwrap();
    let st: Settings = Loader::new(&s)
        .file("a.toml", "[tls.cert]\npath = \"c.pem\"\n")
        .env(env(&[]))
        .build()
        .unwrap()
        .deserialize()
        .unwrap();
    let cert = st.tls.and_then(|t| t.cert);
    assert!(cert.is_some());
    let cert = cert.unwrap();
    assert_eq!(cert.path.as_deref(), Some("c.pem"));
    assert!(cert.chain.is_none());
}
