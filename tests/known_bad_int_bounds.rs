// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integer bounds are compared as integers: above 2^53 an f64 cannot tell neighbours apart.

use std::collections::BTreeMap;

use configurant::{ConfigError, Loader, Schema, SchemaError};

fn schema(ty: &str, bounds: &str) -> Result<Schema, SchemaError> {
    Schema::parse(&format!(
        r#"
[project]
name = "demo"
version = "0.1.0"
summary = ""
schema_version = 1

[[value]]
key = "n"
type = "{ty}"
{bounds}
description = "a number"
env = "N"
"#
    ))
}

fn load(s: &Schema, n: &str) -> Result<(), Vec<ConfigError>> {
    let env = BTreeMap::from([("N".to_owned(), n.to_owned())]);
    Loader::new(s).env(env).build().map(|_| ()).map_err(|e| e.0)
}

const P53: i64 = 9_007_199_254_740_992;

#[test]
fn a_min_just_above_2_pow_53_refuses_the_value_below_it() {
    let s = schema("int", &format!("min = {}", P53 + 1)).unwrap();
    assert!(load(&s, &(P53 + 1).to_string()).is_ok());
    let errs = load(&s, &P53.to_string()).unwrap_err();
    assert!(
        matches!(&errs[0], ConfigError::OutOfRange { .. }),
        "{errs:?}"
    );
}

#[test]
fn a_max_of_2_pow_53_refuses_the_next_integer() {
    let s = schema("int", &format!("max = {P53}")).unwrap();
    assert!(load(&s, &P53.to_string()).is_ok());
    let errs = load(&s, &(P53 + 1).to_string()).unwrap_err();
    assert!(
        matches!(&errs[0], ConfigError::OutOfRange { .. }),
        "{errs:?}"
    );
}

#[test]
fn the_extreme_bounds_hold_and_the_error_shows_the_exact_bound() {
    let s = schema("int", &format!("max = {}", i64::MAX - 1)).unwrap();
    assert!(load(&s, &(i64::MAX - 1).to_string()).is_ok());
    let errs = load(&s, &i64::MAX.to_string()).unwrap_err();
    let text = errs[0].to_string();
    assert!(text.contains(&(i64::MAX - 1).to_string()), "{text}");
}

#[test]
fn min_above_max_is_refused_even_when_both_round_to_the_same_float() {
    let r = schema("int", &format!("min = {}\nmax = {P53}", P53 + 1));
    assert!(matches!(r, Err(SchemaError::BadRange(_))), "{r:?}");
}

#[test]
fn a_float_bound_on_an_int_still_works_and_floats_keep_float_bounds() {
    let s = schema("int", "min = 1.5\nmax = 9.5").unwrap();
    assert!(load(&s, "2").is_ok() && load(&s, "9").is_ok());
    assert!(load(&s, "1").is_err() && load(&s, "10").is_err());
    let f = schema("float", "min = 0\nmax = 1.5").unwrap();
    assert!(load(&f, "0.5").is_ok() && load(&f, "1.6").is_err() && load(&f, "-0.1").is_err());
}
