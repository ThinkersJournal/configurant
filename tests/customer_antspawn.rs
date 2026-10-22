// SPDX-License-Identifier: MIT OR Apache-2.0
//! First customer: AntSpawn's job-spec schema (`examples/antspawn.toml`) loads real specs and refuses bad ones.

use std::collections::BTreeMap;

use configurant::{ConfigError, Loader, Schema};

const SCHEMA: &str = include_str!("../examples/antspawn.toml");

fn load(spec: &str) -> Result<configurant::Config, configurant::Errors> {
    let schema = Schema::parse(SCHEMA).expect("the customer schema is valid");
    Loader::new(&schema)
        .file("job.toml", spec)
        .env(BTreeMap::new())
        .build()
}

#[test]
fn a_minimal_spec_loads_with_antspawn_defaults() {
    let c = load("[job]\nname = \"slangc\"\ncmd = [\"slangc\", \"--version\"]\n").unwrap();
    assert_eq!(c.get::<String>("needs.network").unwrap(), "off");
    assert_eq!(c.get::<String>("needs.gpu").unwrap(), "none");
    assert_eq!(c.get::<Vec<String>>("needs.require").unwrap().len(), 4);
    assert_eq!(
        c.get::<Vec<String>>("job.cmd").unwrap(),
        ["slangc", "--version"]
    );
}

#[test]
fn a_full_flat_spec_loads() {
    let c = load(
        "[job]\nname = \"b\"\ncmd = [\"cargo\", \"test\"]\ncwd = \"/work\"\n\
         [limits]\ncpu = 4\nram_mb = 8192\ntime_s = 600\n\
         [needs]\nnetwork = \"on\"\ngpu = \"cuda\"\ntools = [\"cargo\"]\nprefer = [\"jit-runner\"]\n",
    )
    .unwrap();
    assert_eq!(c.get::<i64>("limits.ram_mb").unwrap(), 8192);
    assert_eq!(c.get::<String>("needs.gpu").unwrap(), "cuda");
}

fn refused(spec: &str) -> Vec<ConfigError> {
    load(spec).expect_err("the spec must be refused").0
}

#[test]
fn a_misspelt_key_is_refused_with_a_suggestion() {
    let e = refused("[job]\nname = \"x\"\ncmd = [\"a\"]\n[limits]\nram_mbb = 1\n");
    assert!(format!("{e:?}").contains("ram_mb"), "{e:?}");
}

#[test]
fn missing_name_or_cmd_is_refused() {
    assert!(!refused("[job]\ncmd = [\"a\"]\n").is_empty());
    assert!(!refused("[job]\nname = \"x\"\n").is_empty());
}

#[test]
fn bad_enum_and_out_of_range_are_refused() {
    let base = "[job]\nname = \"x\"\ncmd = [\"a\"]\n";
    assert!(!refused(&format!("{base}[needs]\ngpu = \"rocm\"\n")).is_empty());
    assert!(!refused(&format!("{base}[limits]\ncpu = 0\n")).is_empty());
}

#[test]
fn mounts_outputs_and_env_load() {
    let c = load(
        "[job]
name = \"x\"
cmd = [\"a\"]
[job.env]
RUST_LOG = \"info\"
         [[mount]]
host = \"C:/src\"
guest = \"src\"
mode = \"ro\"
         [[mount]]
guest = \"tmp\"
mode = \"scratch\"
         [[output]]
guest = \"out/r.json\"
name = \"result\"
",
    )
    .unwrap();
    assert_eq!(c.rows("mount").unwrap().len(), 2);
    assert_eq!(c.rows("output").unwrap()[0]["name"].shown(), "\"result\"");
    assert_eq!(c.map("job.env").unwrap().unwrap().len(), 1);
    assert!(
        load(
            "[job]
name = \"x\"
cmd = [\"a\"]
"
        )
        .unwrap()
        .rows("mount")
        .unwrap()
        .is_empty()
    );
}

#[test]
fn bad_mounts_outputs_and_env_are_refused() {
    let base = "[job]
name = \"x\"
cmd = [\"a\"]
";
    // Unknown key in a row, with a suggestion; missing required field; bad mode; wrong env type.
    let e = refused(&format!(
        "{base}[[mount]]
gest = \"/s\"
mode = \"ro\"
"
    ));
    assert!(format!("{e:?}").contains("guest"), "{e:?}");
    assert!(
        !refused(&format!(
            "{base}[[mount]]
guest = \"s\"
"
        ))
        .is_empty()
    );
    assert!(
        !refused(&format!(
            "{base}[[mount]]
guest = \"s\"
mode = \"bind\"
"
        ))
        .is_empty()
    );
    assert!(
        !refused(&format!(
            "{base}[[output]]
guest = \"o\"
"
        ))
        .is_empty()
    );
    assert!(
        !refused(&format!(
            "{base}[job.env]
N = 1
"
        ))
        .is_empty()
    );
}
