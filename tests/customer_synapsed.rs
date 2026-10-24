// SPDX-License-Identifier: MIT OR Apache-2.0
//! Second customer: the Synapse daemon's settings schema (`examples/synapsed.toml`) loads real settings and refuses bad ones.

use std::collections::BTreeMap;

use configurant::{ConfigError, Loader, Schema};

const SCHEMA: &str = include_str!("../examples/synapsed.toml");

fn load_with(file: &str, env: &[(&str, &str)]) -> Result<configurant::Config, configurant::Errors> {
    let schema = Schema::parse(SCHEMA).expect("the customer schema is valid");
    let env: BTreeMap<String, String> = env
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    Loader::new(&schema)
        .file("synapsed.toml", file)
        .env(env)
        .build()
}

fn refused(file: &str, env: &[(&str, &str)]) -> Vec<ConfigError> {
    load_with(file, env).expect_err("must be refused").0
}

#[test]
fn an_empty_file_loads_with_synapsed_defaults() {
    let c = load_with("", &[]).unwrap();
    assert_eq!(c.get::<String>("storage.id").unwrap(), "redb");
    assert!(c.get::<Vec<String>>("launch.roles").unwrap().is_empty());
    assert!(c.get::<Vec<String>>("alert.roles").unwrap().is_empty());
}

#[test]
fn the_variables_synapsed_reads_today_still_work() {
    let c = load_with(
        "",
        &[
            ("SYNAPSE_ADDR", "127.0.0.1:7878"),
            ("SYNAPSE_HOME", "/srv/synapse"),
            ("SYNAPSE_ALERT_ROLES", "pm,security"),
        ],
    )
    .unwrap();
    assert_eq!(c.get::<String>("listen.addr").unwrap(), "127.0.0.1:7878");
    assert_eq!(
        c.get::<Vec<String>>("alert.roles").unwrap(),
        ["pm", "security"]
    );
}

#[test]
fn the_environment_beats_the_file_and_explain_says_so() {
    let c = load_with(
        "[listen]\naddr = \"0.0.0.0:1\"\n",
        &[("SYNAPSE_ADDR", "127.0.0.1:2")],
    )
    .unwrap();
    assert_eq!(c.get::<String>("listen.addr").unwrap(), "127.0.0.1:2");
    assert!(
        c.explain("listen.addr")
            .unwrap()
            .to_string()
            .contains("file")
    );
}

#[test]
fn a_store_endpoint_can_name_a_variable_for_a_url() {
    let c = load_with("[storage]\nendpoint =\"env:SYNAPSE_DB_URL\"\n", &[]).unwrap();
    assert_eq!(
        c.get::<String>("storage.endpoint").unwrap(),
        "env:SYNAPSE_DB_URL"
    );
}

#[test]
fn a_misspelt_key_is_refused_with_a_suggestion() {
    let e = refused("[storage]\nendpont = \"x\"\n", &[]);
    assert!(format!("{e:?}").contains("endpoint"));
}

#[test]
fn another_store_is_refused_when_the_persistant_module_is_absent() {
    let e = refused("[storage]\nid = \"sqlite\"\n", &[]);
    assert!(format!("{e:?}").contains("persistant"));
}

#[test]
fn no_value_of_this_schema_is_secret() {
    let schema = Schema::parse(SCHEMA).unwrap();
    assert!(schema.values().iter().all(|v| !v.secret));
}
