// SPDX-License-Identifier: MIT OR Apache-2.0
use configurant::{Loader, Schema};
fn main() {
    let toml = r#"
project = { name = "app", version = "1", summary = "demo", schema_version = 1 }
value = [{ key = "server.port", type = "int", default = 8080, description = "port" }]
"#;
    let schema = Schema::parse(toml).unwrap();
    let c = Loader::new(&schema).build().unwrap(); // default < file < env < command line
    assert_eq!(c.get::<i64>("server.port").unwrap(), 8080);
}
