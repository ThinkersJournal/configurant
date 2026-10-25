// SPDX-License-Identifier: MIT OR Apache-2.0
//! List items in text (environment, command line): `,` separates, `\,` is a literal comma, `\` a literal
//! backslash. Any other backslash stays as written, so a Windows path in a list is not changed.

use configurant::{Value, ValueType};

fn list(text: &str) -> Vec<String> {
    match Value::parse_text(ValueType::List, text) {
        Some(Value::List(items)) => items,
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn an_escaped_comma_is_one_item() {
    assert_eq!(list(r"a\,b,c"), vec!["a,b", "c"]);
}

#[test]
fn an_escaped_backslash_before_a_comma_is_a_backslash_then_a_split() {
    assert_eq!(list(r"a\\,b"), vec![r"a\", "b"]);
}

#[test]
fn an_escaped_backslash_then_an_escaped_comma_is_both_literal() {
    assert_eq!(list(r"a\\\,b"), vec![r"a\,b"]);
}

#[test]
fn any_other_backslash_stays_literal() {
    assert_eq!(list(r"C:\data,D:\more\x"), vec![r"C:\data", r"D:\more\x"]);
    assert_eq!(list(r"tail\"), vec![r"tail\"]);
}

#[test]
fn plain_lists_and_the_empty_list_are_unchanged() {
    assert_eq!(list("a, b ,c"), vec!["a", "b", "c"]);
    assert!(list("  ").is_empty());
}
