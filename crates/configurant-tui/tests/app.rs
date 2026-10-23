// SPDX-License-Identifier: MIT OR Apache-2.0
//! The TUI drives `edit::Session`: nothing is written without the review's `y`, a bad value stays in
//! the editor with its error, and a secret is neither editable nor drawn.

use std::collections::BTreeMap;

use configurant::edit::Session;
use configurant::{Loader, Schema};
use configurant_tui::{App, Effect};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

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
"#;

fn press(app: &mut App, code: KeyCode) -> Effect {
    app.handle_key(KeyEvent::from(code))
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

fn screen(app: &App) -> String {
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| app.draw(f)).unwrap();
    t.backend().to_string()
}

#[test]
fn the_flow_writes_only_after_the_review_is_confirmed() {
    let schema = Schema::parse(SCHEMA).unwrap();
    let config = Loader::new(&schema)
        .env(BTreeMap::from([(
            "DEMO_DB_PASSWORD".into(),
            "hunter2-secret".into(),
        )]))
        .build()
        .unwrap();
    let mut app = App::new(Session::new(&config, None));

    // A bad value stays in the editor and says why; nothing is staged.
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "99999");
    assert!(matches!(press(&mut app, KeyCode::Enter), Effect::None));
    assert!(screen(&app).contains("outside"), "{}", screen(&app));
    assert!(screen(&app).contains("new value> 99999"));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('r'));
    assert!(screen(&app).contains("nothing to review"));

    // A good value is staged; review does not write; `n` goes back; `y` yields the write.
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "9001");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("8080 -> 9001"));
    press(&mut app, KeyCode::Char('r'));
    assert!(screen(&app).contains("nothing is written until you confirm"));
    assert!(matches!(press(&mut app, KeyCode::Char('x')), Effect::None));
    assert!(matches!(press(&mut app, KeyCode::Char('n')), Effect::None));
    press(&mut app, KeyCode::Char('r'));
    let Effect::Write(confirmed) = press(&mut app, KeyCode::Char('y')) else {
        panic!("y in the review must confirm");
    };
    assert!(confirmed.file_text("").unwrap().contains("port = 9001"));
    app.wrote(Ok(()), &confirmed);
    assert!(screen(&app).contains("1 change(s) written"));
    assert!(matches!(press(&mut app, KeyCode::Char('z')), Effect::Quit));
}

#[test]
fn a_secret_is_not_editable_and_never_drawn() {
    let schema = Schema::parse(SCHEMA).unwrap();
    let config = Loader::new(&schema)
        .env(BTreeMap::from([(
            "DEMO_DB_PASSWORD".into(),
            "hunter2-secret".into(),
        )]))
        .build()
        .unwrap();
    let mut app = App::new(Session::new(&config, None));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let s = screen(&app);
    assert!(s.contains("is a secret"), "{s}");
    assert!(s.contains("<redacted>"));
    assert!(!s.contains("hunter2-secret"));
    assert!(
        !s.contains("new value>"),
        "a secret must not open the editor"
    );
}

#[test]
fn a_secret_with_a_source_shows_its_reference_and_still_is_not_editable() {
    let text = SCHEMA.replace(
        "env = \"DEMO_DB_PASSWORD\"",
        "env = \"DEMO_DB_PASSWORD\"\nsource = \"vault:DB_ENTRY\"",
    );
    let schema = Schema::parse(&text).unwrap();
    let config = Loader::new(&schema)
        .resolved(BTreeMap::from([(
            "db.password".into(),
            "hunter2-secret".into(),
        )]))
        .build()
        .unwrap();
    let mut app = App::new(Session::new(&config, None));
    press(&mut app, KeyCode::Down);
    assert!(screen(&app).contains("[source: vault:DB_ENTRY]"));
    press(&mut app, KeyCode::Enter);
    let s = screen(&app);
    assert!(s.contains("secret source `vault:DB_ENTRY`"), "{s}");
    assert!(!s.contains("hunter2-secret") && !s.contains("new value>"));
}
