// SPDX-License-Identifier: MIT OR Apache-2.0
//! The GUI drives `edit::Session`: nothing is written without the review's confirmation, a bad value
//! stays in the editor with its error, a secret is neither editable nor drawn, and the whole flow can
//! be run from the keyboard. Widgets are found through the accessibility tree, as a screen reader sees them.

use std::collections::BTreeMap;

use configurant::edit::{Confirmed, Session};
use configurant::{Config, Loader, Schema};
use configurant_gui::{App, Effect};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

const SCHEMA: &str = r#"
[project]
name = "demo"
version = "0.1.0"
summary = "a demo"
schema_version = 1

[[value]]
key = "server.port"
type = "int"
default = 8080
min = 1
max = 65535
description = "listen port"

[[value]]
key = "db.password"
type = "string"
secret = true
description = "database password"
env = "DEMO_DB_PASSWORD"

[[value]]
key = "log.level"
type = "enum"
choices = ["info", "debug"]
default = "info"
description = "log level"
"#;

const SECRET: &str = "hunter2-secret";

fn config() -> Config {
    let schema = Schema::parse(SCHEMA).unwrap();
    Loader::new(&schema)
        .env(BTreeMap::from([("DEMO_DB_PASSWORD".into(), SECRET.into())]))
        .build()
        .unwrap()
}

struct State<'a> {
    app: App<'a>,
    written: Option<Confirmed>,
    quit: bool,
}

fn harness(config: &Config) -> Harness<'_, State<'_>> {
    let state = State {
        app: App::new(Session::new(config, None)),
        written: None,
        quit: false,
    };
    Harness::new_ui_state(
        |ui, s: &mut State| match s.app.ui(ui) {
            Effect::None => {}
            Effect::Quit => s.quit = true,
            Effect::Write(c) => s.written = Some(c),
            _ => {}
        },
        state,
    )
}

#[test]
fn the_flow_writes_only_after_the_review_is_confirmed() {
    let config = config();
    let mut h = harness(&config);

    // A bad value stays in the editor and says why; nothing is staged.
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("New value").type_text("99999");
    h.run();
    h.get_by_label("Stage").click();
    h.run();
    assert!(h.query_by_label_contains("outside").is_some());
    assert!(h.query_by_label("Stage").is_some(), "still in the editor");
    h.get_by_label("Cancel").click();
    h.run();
    h.get_by_label("Review and confirm").click();
    h.run();
    assert!(h.query_by_label_contains("nothing to review").is_some());

    // A good value is staged; review does not write; going back does not write; confirming does.
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("New value").type_text("9001");
    h.run();
    h.get_by_label("Stage").click();
    h.run();
    assert!(h.query_by_label_contains("8080 -> 9001").is_some());
    h.get_by_label("Review and confirm").click();
    h.run();
    assert!(
        h.query_by_label_contains("Nothing is written until you confirm")
            .is_some()
    );
    h.get_by_label("Go back").click();
    h.run();
    assert!(h.state().written.is_none());
    h.get_by_label("Review and confirm").click();
    h.run();
    assert!(h.state().written.is_none(), "reviewing must not write");
    h.get_by_label("Write the file").click();
    h.run();
    let confirmed = h
        .state_mut()
        .written
        .take()
        .expect("confirming must yield the write");
    assert!(confirmed.file_text("").unwrap().contains("port = 9001"));
    h.state_mut().app.wrote(Ok(()), &confirmed);
    h.run();
    assert!(h.query_by_label_contains("1 change(s) written").is_some());
    h.get_by_label("Close").click();
    h.run();
    assert!(h.state().quit);
}

#[test]
fn a_secret_is_not_editable_and_never_drawn() {
    let config = config();
    let mut h = harness(&config);
    h.get_by_label_contains("db.password").click();
    h.run();
    assert!(h.query_by_label_contains("<redacted>").is_some());
    assert!(h.query_by_label_contains(SECRET).is_none());
    // The Edit button is disabled for a secret, so no text box can open for it.
    assert!(h.get_by_label("Edit").accesskit_node().is_disabled());
    assert!(h.query_by_label("New value").is_none());
    assert!(
        h.query_by_label_contains("Secret: set only through")
            .is_some()
    );
}

#[test]
fn a_provider_free_enum_offers_its_choices_and_stages_one() {
    let config = config();
    let mut h = harness(&config);
    h.get_by_label_contains("log.level").click();
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("debug").click();
    h.run();
    h.get_by_label("Stage").click();
    h.run();
    assert!(h.query_by_label_contains("\"info\" -> \"debug\"").is_some());
}

#[test]
fn it_can_be_driven_from_the_keyboard_alone() {
    let config = config();
    let mut h = harness(&config);
    // Focus the first row with Tab, open the editor from the button by keyboard, type, stage with Enter.
    h.key_press(eframe::egui::Key::Tab);
    h.run();
    assert!(
        h.get_by_label("server.port   8080").is_focused(),
        "the first value takes focus on Tab"
    );
    h.get_by_label("Edit").focus();
    h.run();
    h.key_press(eframe::egui::Key::Enter);
    h.run();
    h.get_by_label("New value").type_text("9002");
    h.run();
    h.key_press(eframe::egui::Key::Enter);
    h.run();
    assert!(h.query_by_label_contains("8080 -> 9002").is_some());
}
