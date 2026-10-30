// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `ports` module (feature `ports`, docs/MODULES.md section 2, rules P1 to P4). Known-bad first: every row
//! of the design is a value that must fail to load, with the accepted control beside it.

#![cfg(feature = "ports")]

use std::collections::BTreeMap;

use configurant::modules::ports::Ports;
use configurant::{ConfigError, Errors, Loader, Modules, Schema, SchemaError};

fn schema_with(ty: &str, extra: &str, default: &str) -> String {
    format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "listen.addr"
type = "{ty}"
{default}
description = "where to listen"
env = "LISTEN"
provider = "ports"
{extra}
"#
    )
}

fn ports(p: Ports) -> Modules {
    let mut m = Modules::new();
    m.register(Box::new(p)).unwrap();
    m
}

fn load(schema: &str, p: Ports, env: &[(&str, &str)]) -> Result<configurant::Config, Errors> {
    let s = Schema::parse(schema).unwrap();
    let modules = ports(p);
    let env: BTreeMap<String, String> = env
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    Loader::new(&s).modules(&modules).env(env).build()
}

/// The text of the module's refusal of `value` given as the environment string, or `None` if it loaded.
fn refusal(schema: &str, p: Ports, value: &str) -> Option<String> {
    match load(schema, p, &[("LISTEN", value)]) {
        Ok(_) => None,
        Err(e) => {
            assert_eq!(e.0.len(), 1, "{e:?}");
            match &e.0[0] {
                ConfigError::ModuleRejected { key, module, .. } => {
                    assert_eq!(key, "listen.addr");
                    assert_eq!(module, "ports");
                }
                // A text that is not an int never reaches the module for an `int` key.
                ConfigError::WrongType { .. } => {}
                other => panic!("unexpected {other:?}"),
            }
            Some(e.0[0].to_string())
        }
    }
}

fn int_schema() -> String {
    schema_with("int", "", "default = 8080")
}

fn addr_schema() -> String {
    schema_with("string", "", "default = \"127.0.0.1:7920\"")
}

fn privileged_schema(ty: &str, default: &str) -> String {
    schema_with(
        ty,
        "privileged = \"public web port, runs as a service\"",
        default,
    )
}

// ---- P1: an int is 1 to 65535 ----

#[test]
fn p1_an_int_port_out_of_range_is_refused() {
    for bad in ["0", "65536", "-1", "99999999999"] {
        let why = refusal(&int_schema(), Ports::new(), bad);
        assert!(why.is_some(), "{bad} loaded");
    }
}

#[test]
fn p1_the_edges_load() {
    for ok in ["1024", "65535", "8080"] {
        assert!(refusal(&int_schema(), Ports::new(), ok).is_none(), "{ok}");
    }
}

#[test]
fn p1_zero_is_accepted_only_with_allow_zero() {
    assert!(refusal(&int_schema(), Ports::new(), "0").is_some());
    assert!(refusal(&int_schema(), Ports::new().allow_zero(), "0").is_none());
    // allow_zero does not open the other edges.
    assert!(refusal(&int_schema(), Ports::new().allow_zero(), "65536").is_some());
}

#[test]
fn p1_a_non_numeric_int_is_refused() {
    assert!(refusal(&int_schema(), Ports::new(), "http").is_some());
    assert!(refusal(&int_schema(), Ports::new(), "80x").is_some());
}

// ---- P2: a string is host:port ----

#[test]
fn p2_a_bad_address_is_refused() {
    for bad in [
        "7878",
        "localhost:",
        ":8080",
        "::1:8080",
        "a b:8080",
        "-x.test:8080",
        "x-.test:8080",
        "[::1]:99999",
        "[::1]:0",
        "127.0.0.1:http",
        "127.0.0.1:+8080",
        "127.0.0.1:-1",
        "999.1.1.1:8080",
        "host..test:8080",
        "[::1:8080",
        "user:pw@h:8080",
    ] {
        assert!(
            refusal(&addr_schema(), Ports::new(), bad).is_some(),
            "{bad} loaded"
        );
    }
}

#[test]
fn p2_a_good_address_loads() {
    for ok in [
        "127.0.0.1:7920",
        "localhost:8080",
        "[::1]:8080",
        "[2001:db8::1]:65535",
        "my-host.example.test:1024",
        "0.0.0.0:9000",
    ] {
        assert!(refusal(&addr_schema(), Ports::new(), ok).is_none(), "{ok}");
    }
}

#[test]
fn p2_a_label_over_63_or_a_name_over_253_is_refused() {
    let label = "a".repeat(64);
    assert!(refusal(&addr_schema(), Ports::new(), &format!("{label}.test:8080")).is_some());
    let ok_label = "a".repeat(63);
    assert!(
        refusal(
            &addr_schema(),
            Ports::new(),
            &format!("{ok_label}.test:8080")
        )
        .is_none()
    );
    let long = format!("{0}.{0}.{0}.{0}.test:8080", "a".repeat(63));
    assert!(refusal(&addr_schema(), Ports::new(), &long).is_some());
}

#[test]
fn p2_a_non_string_non_int_type_is_refused() {
    let schema = schema_with("bool", "", "default = true");
    let s = Schema::parse(&schema).unwrap();
    let modules = ports(Ports::new());
    let errs = Loader::new(&s).modules(&modules).build().unwrap_err();
    assert!(
        errs.0
            .iter()
            .any(|e| matches!(e, ConfigError::ModuleRejected { .. })),
        "{errs:?}"
    );
}

// ---- P3: below 1024 only with a declared reason ----

#[test]
fn p3_a_privileged_port_is_refused_without_the_declaration_naming_how() {
    let why = refusal(&addr_schema(), Ports::new(), "127.0.0.1:80").expect("refused");
    assert!(why.contains("listen.addr"), "{why}");
    assert!(why.contains("1024"), "{why}");
    assert!(why.contains("privileged"), "{why}");
    assert!(refusal(&int_schema(), Ports::new(), "443").is_some());
}

#[test]
fn p3_the_same_value_is_accepted_with_the_declaration() {
    assert!(
        refusal(
            &privileged_schema("string", "default = \"127.0.0.1:7920\""),
            Ports::new(),
            "127.0.0.1:80"
        )
        .is_none()
    );
    assert!(
        refusal(
            &privileged_schema("int", "default = 8080"),
            Ports::new(),
            "443"
        )
        .is_none()
    );
}

#[test]
fn p3_the_declaration_does_not_open_zero_or_out_of_range() {
    assert!(
        refusal(
            &privileged_schema("int", "default = 8080"),
            Ports::new(),
            "0"
        )
        .is_some()
    );
    assert!(
        refusal(
            &privileged_schema("int", "default = 8080"),
            Ports::new(),
            "65536"
        )
        .is_some()
    );
}

#[test]
fn p3_a_privileged_default_is_checked_too() {
    let schema = schema_with("string", "", "default = \"127.0.0.1:80\"");
    let s = Schema::parse(&schema).unwrap();
    let modules = ports(Ports::new());
    let errs = Loader::new(&s).modules(&modules).build().unwrap_err();
    assert!(
        errs.0
            .iter()
            .any(|e| matches!(e, ConfigError::ModuleRejected { .. })),
        "{errs:?}"
    );
    let schema = privileged_schema("string", "default = \"127.0.0.1:80\"");
    let s = Schema::parse(&schema).unwrap();
    assert!(Loader::new(&s).modules(&modules).build().is_ok());
}

#[test]
fn p3_an_empty_reason_is_a_schema_error() {
    let schema = schema_with(
        "string",
        "privileged = \"\"",
        "default = \"127.0.0.1:7920\"",
    );
    assert!(
        matches!(Schema::parse(&schema), Err(SchemaError::EmptyPrivileged(k)) if k == "listen.addr")
    );
    let schema = schema_with(
        "string",
        "privileged = \"   \"",
        "default = \"127.0.0.1:7920\"",
    );
    assert!(matches!(
        Schema::parse(&schema),
        Err(SchemaError::EmptyPrivileged(_))
    ));
}

#[test]
fn p3_privileged_is_valid_only_with_provider_ports() {
    let other = privileged_schema("string", "default = \"127.0.0.1:7920\"")
        .replace("provider = \"ports\"", "provider = \"storage\"");
    assert!(
        matches!(Schema::parse(&other), Err(SchemaError::PrivilegedWithoutPorts(k)) if k == "listen.addr")
    );
    let none = privileged_schema("string", "default = \"127.0.0.1:7920\"")
        .replace("provider = \"ports\"\n", "");
    assert!(matches!(
        Schema::parse(&none),
        Err(SchemaError::PrivilegedWithoutPorts(_))
    ));
}

#[test]
fn p3_explain_and_help_show_the_reason() {
    let schema = privileged_schema("string", "default = \"127.0.0.1:80\"");
    let s = Schema::parse(&schema).unwrap();
    let help = s.help();
    assert!(
        help.contains("public web port, runs as a service"),
        "{help}"
    );
    let modules = ports(Ports::new());
    let cfg = Loader::new(&s).modules(&modules).build().unwrap();
    let text = cfg.explain("listen.addr").unwrap().to_string();
    assert!(
        text.contains("public web port, runs as a service"),
        "{text}"
    );
    // A key without the declaration shows nothing of the kind.
    let plain = Schema::parse(&addr_schema()).unwrap();
    assert!(!plain.help().contains("privileged"));
}

// ---- P4: the error names the key and rule and leaks nothing of a secret ----

#[test]
fn p4_a_refusal_names_the_key_and_the_rule() {
    let why = refusal(&addr_schema(), Ports::new(), "localhost:").expect("refused");
    assert!(why.contains("listen.addr"), "{why}");
    assert!(why.contains("P2"), "{why}");
    let why = refusal(&int_schema(), Ports::new(), "65536").expect("refused");
    assert!(why.contains("P1"), "{why}");
    let why = refusal(&addr_schema(), Ports::new(), "127.0.0.1:80").expect("refused");
    assert!(why.contains("P3"), "{why}");
}

#[test]
fn p4_a_secret_address_leaks_nothing_anywhere() {
    let schema = schema_with("string", "secret = true", "");
    let s = Schema::parse(&schema).unwrap();
    let modules = ports(Ports::new());
    let env: BTreeMap<String, String> =
        BTreeMap::from([("LISTEN".to_owned(), "user:hunter2@h:80".to_owned())]);
    let errs = Loader::new(&s)
        .modules(&modules)
        .env(env)
        .build()
        .unwrap_err();
    let shown = format!("{errs} {errs:?}");
    assert!(!shown.contains("hunter2"), "{shown}");
    assert!(shown.contains("<redacted>"), "{shown}");
}

#[test]
fn p4_a_non_secret_refusal_does_not_echo_the_host_text() {
    // An address string can hold a credential by mistake; the refusal says what rule failed, not the text.
    let why = refusal(&addr_schema(), Ports::new(), "user:hunter2@h:8080").expect("refused");
    assert!(!why.contains("hunter2"), "{why}");
}

// ---- options and the rest of the interface ----

#[test]
fn list_choices_is_empty_unless_the_project_suggests() {
    use configurant::{Module, Needs};
    assert!(Ports::new().list_choices(&Needs::default()).is_empty());
    let ids: Vec<String> = Ports::new()
        .suggest(&[8080, 8443])
        .list_choices(&Needs::default())
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids, ["8080", "8443"]);
}

#[test]
fn the_module_is_named_ports_and_speaks_the_current_interface() {
    use configurant::{INTERFACE_VERSION, Module};
    let p = Ports::new();
    assert_eq!(p.name(), "ports");
    assert_eq!(p.interface_version(), INTERFACE_VERSION);
}

#[test]
fn without_the_module_registered_a_non_default_value_is_still_refused_naming_it() {
    let s = Schema::parse(&addr_schema()).unwrap();
    let env: BTreeMap<String, String> =
        BTreeMap::from([("LISTEN".to_owned(), "127.0.0.1:9".to_owned())]);
    let errs = Loader::new(&s).env(env).build().unwrap_err();
    assert!(
        errs.0
            .iter()
            .any(|e| matches!(e, ConfigError::ModuleMissing { .. })),
        "{errs:?}"
    );
}

#[test]
fn p2_an_unbracketed_ipv6_host_is_told_to_use_brackets() {
    let why = refusal(&addr_schema(), Ports::new(), "::1:8080").expect("refused");
    assert!(why.contains("[...]"), "{why}");
}

// ---- P5 and interface version 2: two ports in one schema must not collide ----

fn two_keys(secret_b: bool, a: &str, b: &str) -> String {
    // A secret key may not have a default, so its value comes from the environment (see `ADMIN`).
    let b_default = if secret_b {
        String::new()
    } else {
        format!("default = \"{b}\"")
    };
    format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "listen.addr"
type = "string"
default = "{a}"
description = "where to listen"
env = "LISTEN"
provider = "ports"

[[value]]
key = "admin.addr"
type = "string"
{b_default}
description = "admin listener"
env = "ADMIN"
provider = "ports"
secret = {secret_b}
"#
    )
}

/// Every error of the load, as text, or an empty list if it loaded.
fn all_errors(schema: &str, env: &[(&str, &str)]) -> Vec<String> {
    match load(schema, Ports::new(), env) {
        Ok(_) => Vec::new(),
        Err(e) => e.0.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn p5_two_defaults_on_one_port_are_refused_naming_both_keys() {
    let errs = all_errors(&two_keys(false, "127.0.0.1:7878", "127.0.0.1:7878"), &[]);
    // One error per key at fault, each naming the other key.
    assert_eq!(errs.len(), 2, "{errs:?}");
    assert!(errs.iter().all(|e| e.contains("P5")), "{errs:?}");
    assert!(
        errs.iter()
            .any(|e| e.contains("`listen.addr` from") && e.contains("`admin.addr`")),
        "{errs:?}"
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("`admin.addr` from") && e.contains("`listen.addr`")),
        "{errs:?}"
    );
}

#[test]
fn p5_a_collision_made_by_an_override_is_refused() {
    let schema = two_keys(false, "127.0.0.1:7878", "127.0.0.1:7879");
    assert!(all_errors(&schema, &[]).is_empty());
    let errs = all_errors(&schema, &[("ADMIN", "127.0.0.1:7878")]);
    assert_eq!(errs.len(), 2, "{errs:?}");
    assert!(errs.iter().all(|e| e.contains("P5")), "{errs:?}");
    assert!(
        errs.iter().any(|e| e.contains("ADMIN")),
        "names the origin: {errs:?}"
    );
}

#[test]
fn p5_the_same_port_on_different_hosts_loads() {
    let schema = two_keys(false, "127.0.0.1:7878", "192.168.1.5:7878");
    assert!(all_errors(&schema, &[]).is_empty());
}

#[test]
fn p5_a_wildcard_host_collides_with_a_specific_one() {
    for (a, b) in [
        ("0.0.0.0:7878", "127.0.0.1:7878"),
        ("[::]:7878", "[::1]:7878"),
        ("localhost:7878", "0.0.0.0:7878"),
    ] {
        let errs = all_errors(&two_keys(false, a, b), &[]);
        assert_eq!(errs.len(), 2, "{a} vs {b}: {errs:?}");
    }
}

#[test]
fn p5_host_names_compare_without_case() {
    let errs = all_errors(
        &two_keys(false, "Example.TEST:7878", "example.test:7878"),
        &[],
    );
    assert_eq!(errs.len(), 2, "{errs:?}");
}

#[test]
fn p5_port_zero_never_collides() {
    let mut m = Modules::new();
    m.register(Box::new(Ports::new().allow_zero())).unwrap();
    let s = Schema::parse(&two_keys(false, "127.0.0.1:0", "127.0.0.1:0")).unwrap();
    assert!(Loader::new(&s).modules(&m).build().is_ok());
}

#[test]
fn p5_a_secret_key_in_a_collision_leaks_nothing() {
    let env = [("ADMIN", "127.0.0.1:7878")];
    let errs = all_errors(&two_keys(true, "127.0.0.1:7878", ""), &env);
    assert_eq!(errs.len(), 2, "{errs:?}");
    assert!(errs.iter().all(|e| !e.contains("7878")), "{errs:?}");
    let s = Schema::parse(&two_keys(true, "127.0.0.1:7878", "")).unwrap();
    let e = Loader::new(&s)
        .modules(&ports(Ports::new()))
        .env(BTreeMap::from([(
            "ADMIN".to_owned(),
            "127.0.0.1:7878".to_owned(),
        )]))
        .build()
        .unwrap_err();
    assert!(!format!("{e:?}").contains("7878"));
}

#[test]
fn p5_an_int_port_and_an_address_with_the_same_port_collide() {
    let schema = r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "a.port"
type = "int"
default = 7878
description = "a"
provider = "ports"

[[value]]
key = "b.addr"
type = "string"
default = "10.0.0.1:7878"
description = "b"
provider = "ports"
"#;
    let errs = all_errors(schema, &[]);
    assert_eq!(errs.len(), 2, "{errs:?}");
}

#[test]
fn p5_a_key_that_already_failed_is_not_reported_twice() {
    let errs = all_errors(
        &two_keys(false, "127.0.0.1:7878", "127.0.0.1:7879"),
        &[("ADMIN", "not an address")],
    );
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("P2"), "{errs:?}");
}

#[test]
fn interface_one_modules_still_register_and_two_is_the_current_version() {
    use configurant::{INTERFACE_VERSION, MIN_INTERFACE_VERSION, Module, Needs, Value};
    struct Old;
    impl Module for Old {
        fn name(&self) -> &str {
            "old"
        }
        fn interface_version(&self) -> u32 {
            1
        }
        fn list_choices(&self, _: &Needs) -> Vec<configurant::Choice> {
            Vec::new()
        }
        fn validate(&self, _: &Value) -> Result<(), String> {
            Ok(())
        }
    }
    struct Future;
    impl Module for Future {
        fn name(&self) -> &str {
            "future"
        }
        fn interface_version(&self) -> u32 {
            INTERFACE_VERSION + 1
        }
        fn list_choices(&self, _: &Needs) -> Vec<configurant::Choice> {
            Vec::new()
        }
        fn validate(&self, _: &Value) -> Result<(), String> {
            Ok(())
        }
    }
    assert_eq!((MIN_INTERFACE_VERSION, INTERFACE_VERSION), (1, 2));
    let mut m = Modules::new();
    assert!(m.register(Box::new(Old)).is_ok());
    assert!(m.register(Box::new(Future)).is_err());
}
