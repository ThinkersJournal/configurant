// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known-bad inputs for arrays of tables (`[[table]]`) and maps (`[[map]]`): each rule starts here as a
//! failing test.

use std::collections::BTreeMap;

use configurant::edit::Session;
use configurant::{Config, ConfigError, Errors, Loader, Schema, SchemaError, Value};

const SCHEMA: &str = r#"
[project]
name = "t"
version = "1.0.0"
schema_version = 1

[[value]]
key = "job.name"
type = "string"
description = "name"

[[table]]
key = "mount"
description = "directories shared with the job"

[[table.field]]
key = "guest"
type = "path"
required = true
description = "path inside the job"

[[table.field]]
key = "mode"
type = "enum"
choices = ["ro", "rw", "scratch"]
default = "ro"
description = "access"

[[table.field]]
key = "size_mb"
type = "int"
min = 1
description = "size"

[[map]]
key = "job.env"
value_type = "string"
description = "environment passed to the job"
"#;

fn schema() -> Schema {
    Schema::parse(SCHEMA).expect("the test schema is valid")
}

fn load(file: &str) -> Result<Config, Errors> {
    let s = schema();
    Loader::new(&s).file("f.toml", file).build()
}

fn refused(file: &str) -> Vec<ConfigError> {
    load(file).expect_err("must be refused").0
}

fn bad_schema(extra: &str) -> SchemaError {
    Schema::parse(&format!("{SCHEMA}{extra}")).expect_err("the schema must be refused")
}

#[test]
fn rows_and_a_map_load_with_field_defaults() {
    let c = load(
        "[[mount]]\nguest = \"/s\"\nmode = \"scratch\"\n[[mount]]\nguest = \"/d\"\n\
         [job.env]\nA = \"1\"\nB = \"two\"\n",
    )
    .unwrap();
    let rows = c.rows("mount").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["mode"], Value::Enum("scratch".into()));
    assert_eq!(rows[1]["mode"], Value::Enum("ro".into()), "default filled");
    assert!(!rows[1].contains_key("size_mb"), "no default, not required");
    let env = c.map("job.env").unwrap().unwrap();
    assert_eq!(env["B"], Value::String("two".into()));
    assert!(load("").unwrap().rows("mount").unwrap().is_empty());
    assert!(load("").unwrap().map("job.env").unwrap().is_none());
}

#[test]
fn an_unknown_key_inside_a_row_is_refused_with_a_suggestion_and_the_row_index() {
    let e = refused("[[mount]]\nguest = \"/a\"\n[[mount]]\nguest = \"/b\"\nmod = \"rw\"\n");
    assert_eq!(e.len(), 1, "{e:?}");
    match &e[0] {
        ConfigError::UnknownKey {
            key, suggestion, ..
        } => {
            assert_eq!(key, "mount[1].mod");
            assert_eq!(suggestion.as_deref(), Some("mode"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_wrong_type_in_one_row_names_that_row_and_never_echoes_the_value() {
    let e = refused("[[mount]]\nguest = \"/a\"\n[[mount]]\nguest = 31337\n");
    assert_eq!(e.len(), 1, "{e:?}");
    let text = e[0].to_string();
    assert!(text.contains("mount[1].guest"), "{text}");
    assert!(text.contains("integer"), "{text}");
    assert!(!text.contains("31337"), "{text}");
}

#[test]
fn a_row_value_outside_its_range_or_choices_is_refused() {
    let e = refused("[[mount]]\nguest = \"/a\"\nsize_mb = 0\nmode = \"wx\"\n");
    assert_eq!(e.len(), 2, "{e:?}");
    assert!(e.iter().all(|e| e.to_string().contains("mount[0].")));
}

#[test]
fn a_missing_required_field_names_the_row() {
    let e = refused("[[mount]]\nguest = \"/a\"\n[[mount]]\nmode = \"rw\"\n");
    assert_eq!(
        e,
        [ConfigError::MissingRequired {
            key: "mount[1].guest".into()
        }]
    );
}

#[test]
fn a_duplicate_map_key_is_refused_by_the_file_parser() {
    let e = refused("[job.env]\nA = \"1\"\nA = \"2\"\n");
    assert!(matches!(e[0], ConfigError::FileParse { .. }), "{e:?}");
}

#[test]
fn a_map_value_of_the_wrong_type_or_an_empty_key_is_refused_without_echo() {
    let e = refused("[job.env]\nAPI = 31337\n\"\" = \"x\"\n[job.env.sub]\nq = \"r\"\n");
    assert_eq!(e.len(), 3, "{e:?}");
    let text = Errors(e).to_string();
    assert!(text.contains("job.env.API"), "{text}");
    assert!(text.contains("empty key"), "{text}");
    assert!(text.contains("job.env.sub"), "{text}");
    assert!(!text.contains("31337"), "{text}");
}

#[test]
fn the_wrong_shape_for_a_table_or_a_map_is_refused() {
    let e = refused("[mount]\nguest = \"/a\"\n");
    assert!(
        e[0].to_string()
            .contains("expected array of tables, found table"),
        "{e:?}"
    );
    let e = refused("mount = [1]\n");
    assert!(e[0].to_string().contains("found integer"), "{e:?}");
    let e = refused("job.env = \"A=1\"\n");
    assert!(
        e[0].to_string().contains("expected map, found string"),
        "{e:?}"
    );
}

#[test]
fn nothing_is_kept_of_an_array_with_a_bad_row() {
    // The loader reports and fails; it never half-loads an array.
    assert!(load("[[mount]]\nguest = \"/a\"\n[[mount]]\nmode = \"rw\"\n").is_err());
}

#[test]
fn a_required_table_or_map_must_be_set() {
    let s = Schema::parse(&SCHEMA.replace(
        "key = \"mount\"\ndescription",
        "key = \"mount\"\nrequired = true\ndescription",
    ))
    .unwrap();
    let e = Loader::new(&s).file("f.toml", "").build().unwrap_err().0;
    assert_eq!(
        e,
        [ConfigError::MissingRequired {
            key: "mount".into()
        }]
    );
    // An empty array is set.
    assert!(
        Loader::new(&s)
            .file("f.toml", "mount = []\n")
            .build()
            .is_ok()
    );
}

#[test]
fn a_secret_row_field_is_refused_by_the_schema() {
    let e = Schema::parse(&SCHEMA.replace(
        "default = \"ro\"\ndescription = \"access\"",
        "default = \"ro\"\ndescription = \"access\"\n\n[[table.field]]\nkey = \"token\"\ntype = \"string\"\nsecret = true\ndescription = \"t\"",
    ))
    .unwrap_err();
    assert_eq!(
        e,
        SchemaError::RowFieldOption {
            table: "mount".into(),
            field: "token".into(),
            option: "secret"
        }
    );
}

#[test]
fn a_secret_map_is_refused_by_the_schema() {
    let e = Schema::parse(&SCHEMA.replace(
        "value_type = \"string\"",
        "value_type = \"string\"\nsecret = true",
    ))
    .unwrap_err();
    assert!(matches!(e, SchemaError::Parse(_)), "{e:?}");
}

#[test]
fn a_row_field_cannot_have_env_flag_or_provider() {
    for (opt, line) in [
        ("env", "env = \"X\""),
        ("flag", "flag = \"--x\""),
        ("provider", "provider = \"m\""),
    ] {
        let e = Schema::parse(
            &SCHEMA.replace("default = \"ro\"", &format!("default = \"ro\"\n{line}")),
        )
        .unwrap_err();
        assert!(
            matches!(&e, SchemaError::RowFieldOption { option, .. } if *option == opt),
            "{opt}: {e:?}"
        );
    }
}

#[test]
fn schema_rules_for_tables_and_maps() {
    assert!(matches!(
        bad_schema("\n[[table]]\nkey = \"mount\"\ndescription = \"d\"\n"),
        SchemaError::DuplicateKey(k) if k == "mount"
    ));
    assert!(matches!(
        bad_schema("\n[[map]]\nkey = \"job.env\"\nvalue_type = \"string\"\ndescription = \"d\"\n"),
        SchemaError::DuplicateKey(_)
    ));
    assert!(matches!(
        bad_schema("\n[[map]]\nkey = \"job\"\nvalue_type = \"string\"\ndescription = \"d\"\n"),
        SchemaError::KeyConflict { .. }
    ));
    assert!(matches!(
        bad_schema("\n[[map]]\nkey = \"m\"\nvalue_type = \"list\"\ndescription = \"d\"\n"),
        SchemaError::BadMapValueType(_)
    ));
    assert!(matches!(
        bad_schema("\n[[value]]\nkey = \"x\"\ntype = \"table\"\ndescription = \"d\"\n"),
        SchemaError::CompositeValue(_)
    ));
    assert!(matches!(
        bad_schema(
            "\n[[map]]\nkey = \"config_version\"\nvalue_type = \"int\"\ndescription = \"d\"\n"
        ),
        SchemaError::ReservedKey(_)
    ));
    let dup = SCHEMA.replace("key = \"mode\"", "key = \"guest\"");
    assert!(matches!(
        Schema::parse(&dup).unwrap_err(),
        SchemaError::DuplicateField { .. }
    ));
    let dotted = SCHEMA.replace("key = \"mode\"", "key = \"a.b\"");
    assert!(matches!(
        Schema::parse(&dotted).unwrap_err(),
        SchemaError::BadFieldName { .. }
    ));
    let bad_default = SCHEMA.replace("default = \"ro\"", "default = \"zz\"");
    assert!(matches!(
        Schema::parse(&bad_default).unwrap_err(),
        SchemaError::BadDefault { .. }
    ));
}

#[test]
fn environment_and_command_line_cannot_reach_a_table_or_a_map() {
    let s = schema();
    let mut env = BTreeMap::new();
    env.insert("MOUNT".to_owned(), "x".to_owned());
    assert!(
        Loader::new(&s).env(env).build().is_ok(),
        "no prefix: ignored"
    );
    let e = Loader::new(&s)
        .cli(vec!["--mount".into(), "x".into()])
        .build()
        .unwrap_err();
    assert!(matches!(e.0[0], ConfigError::UnknownKey { .. }), "{e:?}");
}

#[test]
fn explain_and_debug_cover_tables_and_maps() {
    let c = load("[[mount]]\nguest = \"/s\"\n[job.env]\nA = \"1\"\n").unwrap();
    let text = c.explain("mount").unwrap().to_string();
    assert!(text.contains("mount (table)"), "{text}");
    assert!(text.contains("guest: \"/s\""), "{text}");
    let text = c.explain("job.env").unwrap().to_string();
    assert!(
        text.contains("job.env (map)") && text.contains("A: \"1\""),
        "{text}"
    );
    assert!(c.explain("mounts").is_err());
    let dbg = format!("{c:?}");
    assert!(dbg.contains("mount"), "{dbg}");
    assert!(schema().help().contains("[[mount]]"));
    assert!(schema().help().contains("[job.env]"));
}

#[test]
fn rows_and_maps_deserialize_into_the_callers_struct() {
    #[derive(serde::Deserialize)]
    struct Mount {
        guest: String,
        mode: String,
    }
    #[derive(serde::Deserialize)]
    struct Job {
        env: BTreeMap<String, String>,
    }
    #[derive(serde::Deserialize)]
    struct Spec {
        mount: Vec<Mount>,
        job: Job,
    }
    let c = load("[[mount]]\nguest = \"/s\"\n[job.env]\nA = \"1\"\n").unwrap();
    let spec: Spec = c.deserialize().unwrap();
    assert_eq!(spec.mount[0].guest, "/s");
    assert_eq!(spec.mount[0].mode, "ro");
    assert_eq!(spec.job.env["A"], "1");
}

#[test]
fn an_edit_session_refuses_a_table_or_map_and_keeps_them_in_the_file() {
    let file = "# keep\n[[mount]]\nguest = \"/s\"\n[job.env]\nA = \"1\"\n";
    let c = load(file).unwrap();
    let mut s = Session::new(&c, None);
    assert!(matches!(
        s.stage("mount", "x").unwrap_err(),
        ConfigError::FileOnly { .. }
    ));
    assert!(matches!(
        s.stage("job.env", "x").unwrap_err(),
        ConfigError::FileOnly { .. }
    ));
    s.stage("job.name", "n").unwrap();
    let text = s.review().confirm().file_text(file).unwrap();
    assert!(text.contains("# keep") && text.contains("[[mount]]") && text.contains("A = \"1\""));
    assert!(text.contains("name = \"n\""));
}
