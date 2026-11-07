// SPDX-License-Identifier: MIT OR Apache-2.0
//! Export a ConfigurAnt schema as a JSON Schema (draft 2020-12), so an editor that understands JSON
//! Schema for TOML (taplo and Even Better TOML, VS Code's `evenBetterToml.schema.associations`) can
//! complete keys, check types and show descriptions in the configuration *file* (ROADMAP row 13).
//!
//! What is exported is what the file may hold:
//!
//! - A `secret` value is **left out entirely** (key, description, default and all): a file may not hold one,
//!   the loader refuses it, and an editor must not suggest it. Because every object closes with
//!   `additionalProperties: false`, an editor flags a secret written into the file, as the loader would.
//! - A dotted key becomes nested objects (`server.port` is `server` then `port`).
//! - Tables become arrays of objects, maps become objects whose values share one type, and the loader's own
//!   `config_version` is a positive integer at the top.
//! - Values that come from a module's list (`provider`) are exported as their plain type: the choices are
//!   known only at run time, so none are invented.
//!
//! Nothing here writes a file or needs a dependency: the JSON is written by a small printer of its own,
//! and the tests read it back with a real parser.

use std::collections::BTreeMap;

use crate::schema::{Bound, MapDef, Schema, TableDef, ValueDef};
use crate::value::ValueType;

/// A JSON value: enough to describe a schema.
enum J {
    Bool(bool),
    Int(i64),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn s(text: &str) -> J {
        J::Str(text.to_owned())
    }

    fn obj(items: Vec<(&str, J)>) -> J {
        J::Obj(items.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            J::Int(i) => out.push_str(&i.to_string()),
            // A float always carries a `.` or exponent so it is not read back as an integer, and a value that
            // JSON cannot hold (NaN, infinity) never reaches here: `number` refuses it.
            J::Num(x) => {
                let t = format!("{x:?}");
                out.push_str(&t);
            }
            J::Str(t) => write_string(out, t),
            J::Arr(items) if items.is_empty() => out.push_str("[]"),
            J::Arr(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    indent(out, depth + 1);
                    item.write(out, depth + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                indent(out, depth);
                out.push(']');
            }
            J::Obj(items) if items.is_empty() => out.push_str("{}"),
            J::Obj(items) => {
                out.push_str("{\n");
                for (i, (k, v)) in items.iter().enumerate() {
                    indent(out, depth + 1);
                    write_string(out, k);
                    out.push_str(": ");
                    v.write(out, depth + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                indent(out, depth);
                out.push('}');
            }
        }
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// A JSON string: quotes, backslashes and every control character escaped (so a description with a newline,
/// a tab or a quote stays one valid string).
fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A finite number as JSON, or `None` (a NaN or an infinity is not JSON).
fn number(x: f64) -> Option<J> {
    x.is_finite().then_some(J::Num(x))
}

fn bound(b: Bound) -> Option<J> {
    match b {
        Bound::Int(i) => Some(J::Int(i)),
        Bound::Float(x) => number(x),
    }
}

fn type_name(ty: ValueType) -> &'static str {
    match ty {
        ValueType::Int => "integer",
        ValueType::Float => "number",
        ValueType::Bool => "boolean",
        ValueType::List => "array",
        ValueType::Table => "array",
        ValueType::Map => "object",
        _ => "string",
    }
}

fn default_json(v: &toml::Value) -> Option<J> {
    match v {
        toml::Value::String(s) => Some(J::Str(s.clone())),
        toml::Value::Integer(i) => Some(J::Int(*i)),
        toml::Value::Float(x) => number(*x),
        toml::Value::Boolean(b) => Some(J::Bool(*b)),
        toml::Value::Array(items) => items
            .iter()
            .map(default_json)
            .collect::<Option<Vec<_>>>()
            .map(J::Arr),
        _ => None,
    }
}

/// The schema of one `[[value]]` (or one table field).
fn value_schema(d: &ValueDef) -> J {
    let mut o: Vec<(&str, J)> = vec![("type", J::s(type_name(d.ty)))];
    o.push(("description", J::Str(d.description.clone())));
    match d.ty {
        ValueType::Enum if !d.choices.is_empty() => {
            o.push(("enum", J::Arr(d.choices.iter().map(|c| J::s(c)).collect())));
        }
        ValueType::List => {
            o.push(("items", J::obj(vec![("type", J::s("string"))])));
        }
        ValueType::Int | ValueType::Float => {
            if let Some(m) = d.min.and_then(bound) {
                o.push(("minimum", m));
            }
            if let Some(m) = d.max.and_then(bound) {
                o.push(("maximum", m));
            }
        }
        _ => {}
    }
    if let Some(def) = d.default.as_ref().and_then(default_json) {
        o.push(("default", def));
    }
    J::obj(o)
}

fn object_of(fields: &[ValueDef]) -> J {
    let mut props = Vec::new();
    let mut required = Vec::new();
    for f in fields {
        props.push((f.key.clone(), value_schema(f)));
        if f.required {
            required.push(J::Str(f.key.clone()));
        }
    }
    let mut o: Vec<(&str, J)> = vec![
        ("type", J::s("object")),
        ("properties", J::Obj(props)),
        ("additionalProperties", J::Bool(false)),
    ];
    if !required.is_empty() {
        o.push(("required", J::Arr(required)));
    }
    J::obj(o)
}

fn table_schema(t: &TableDef) -> J {
    J::obj(vec![
        ("type", J::s("array")),
        ("description", J::Str(t.description.clone())),
        ("items", object_of(&t.fields)),
    ])
}

fn map_schema(m: &MapDef) -> J {
    J::obj(vec![
        ("type", J::s("object")),
        ("description", J::Str(m.description.clone())),
        (
            "additionalProperties",
            J::obj(vec![("type", J::s(type_name(m.value_type)))]),
        ),
    ])
}

/// The tree of dotted keys: a leaf schema, or children.
#[derive(Default)]
struct Node {
    leaf: Option<J>,
    required: bool,
    children: BTreeMap<String, Node>,
}

impl Node {
    fn insert(&mut self, key: &str, schema: J, required: bool) {
        let mut node = self;
        for part in key.split('.') {
            node = node.children.entry(part.to_owned()).or_default();
        }
        node.leaf = Some(schema);
        node.required = required;
    }

    /// A node is required if it is, or if anything under it is (an object holding a required key must
    /// itself be present).
    fn needed(&self) -> bool {
        self.required || self.children.values().any(Node::needed)
    }

    fn render(self) -> J {
        let Node { leaf, children, .. } = self;
        if let Some(leaf) = leaf {
            return leaf;
        }
        let mut props = Vec::new();
        let mut required = Vec::new();
        for (name, child) in children {
            if child.needed() {
                required.push(J::Str(name.clone()));
            }
            props.push((name, child.render()));
        }
        let mut o: Vec<(&str, J)> = vec![
            ("type", J::s("object")),
            ("properties", J::Obj(props)),
            ("additionalProperties", J::Bool(false)),
        ];
        if !required.is_empty() {
            o.push(("required", J::Arr(required)));
        }
        J::obj(o)
    }
}

impl Schema {
    /// This schema as a JSON Schema (draft 2020-12) describing the configuration *file*, pretty-printed with a
    /// trailing newline. A `secret` value is left out entirely (see the module notes); nothing is read from
    /// the environment and no value is resolved.
    ///
    /// ```
    /// let s = configurant::Schema::parse(r#"
    /// [project]
    /// name = "demo"
    /// version = "0.1.0"
    /// summary = "demo"
    /// schema_version = 1
    ///
    /// [[value]]
    /// key = "server.port"
    /// type = "int"
    /// default = 8080
    /// min = 1
    /// max = 65535
    /// description = "listen port"
    /// "#).unwrap();
    /// let json = s.json_schema();
    /// assert!(json.contains("\"maximum\": 65535"));
    /// ```
    pub fn json_schema(&self) -> String {
        let mut root = Node::default();
        for d in self.values().iter().filter(|d| !d.secret) {
            root.insert(&d.key, value_schema(d), d.required);
        }
        for t in self.tables() {
            root.insert(&t.key, table_schema(t), t.required);
        }
        for m in self.maps() {
            root.insert(&m.key, map_schema(m), m.required);
        }
        // The loader's own key, never a value of the schema.
        root.children.insert(
            "config_version".to_owned(),
            Node {
                leaf: Some(J::obj(vec![
                    ("type", J::s("integer")),
                    ("minimum", J::Int(1)),
                    (
                        "description",
                        J::s("the version of this file's layout; `configurant migrate` carries it forward"),
                    ),
                ])),
                ..Node::default()
            },
        );
        let J::Obj(mut items) = root.render() else {
            unreachable!("the root is an object")
        };
        let mut head = vec![
            (
                "$schema".to_owned(),
                J::s("https://json-schema.org/draft/2020-12/schema"),
            ),
            (
                "title".to_owned(),
                J::Str(format!("{} {}", self.project.name, self.project.version)),
            ),
            (
                "description".to_owned(),
                J::Str(self.project.summary.clone()),
            ),
        ];
        head.append(&mut items);
        let mut out = String::new();
        J::Obj(head).write(&mut out, 0);
        out.push('\n');
        out
    }
}
