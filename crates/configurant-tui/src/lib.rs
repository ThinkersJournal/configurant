// SPDX-License-Identifier: MIT OR Apache-2.0
//! The terminal front end's state machine and drawing. It holds an [`configurant::edit::Session`],
//! turns key presses into staged edits, shows a review, and asks for a confirmation; it never writes
//! a file itself. A confirmed change leaves as [`Effect::Write`], which the caller carries out.

use configurant::edit::{Confirmed, Field, Review, Session};
use configurant::{ActionKind, Needs};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};

/// What the caller must do after a key press.
#[derive(Debug)]
pub enum Effect {
    None,
    Quit,
    /// The user confirmed: write `Confirmed::file_text` and report the result with [`App::wrote`].
    Write(Confirmed),
}

enum Mode {
    List,
    Edit(String),
    Review(Review),
    Done(Vec<String>),
}

pub struct App<'a> {
    session: Session<'a>,
    fields: Vec<Field>,
    selected: usize,
    mode: Mode,
    message: Option<String>,
}

impl<'a> App<'a> {
    pub fn new(session: Session<'a>) -> Self {
        let fields = session.fields();
        App {
            session,
            fields,
            selected: 0,
            mode: Mode::List,
            message: None,
        }
    }

    fn refresh(&mut self) {
        self.fields = self.session.fields();
    }

    /// The caller wrote (or failed to write) the file for an [`Effect::Write`]. The plan is listed, not
    /// executed: this front end has no way to open both endpoints of a data transfer.
    pub fn wrote(&mut self, result: Result<(), String>, confirmed: &Confirmed) {
        match result {
            Err(e) => {
                self.message = Some(format!("not written: {e}"));
                self.mode = Mode::List;
            }
            Ok(()) => {
                let mut lines = vec![format!("{} change(s) written.", confirmed.changes().len())];
                for a in &confirmed.plan().actions {
                    lines.push(match &a.kind {
                        ActionKind::DataTransfer { from, to } => format!(
                            "NOT DONE: {} (copy `{from}` -> `{to}`; this front end cannot run data transfers)",
                            a.description
                        ),
                        _ => a.description.clone(),
                    });
                }
                lines.push("Press any key to leave.".into());
                self.mode = Mode::Done(lines);
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Effect {
        if key.kind != KeyEventKind::Press {
            return Effect::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Effect::Quit;
        }
        match &mut self.mode {
            Mode::List => self.list_key(key.code),
            Mode::Edit(_) => {
                self.edit_key(key.code);
                Effect::None
            }
            Mode::Review(_) => self.review_key(key.code),
            Mode::Done(_) => Effect::Quit,
        }
    }

    fn list_key(&mut self, code: KeyCode) -> Effect {
        self.message = None;
        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.fields.len().saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char('e') => match self.fields.get(self.selected) {
                Some(f) if !f.editable => {
                    self.message = Some(format!(
                        "`{}` is a secret: it comes only from its environment variable",
                        f.key
                    ));
                }
                Some(_) => self.mode = Mode::Edit(String::new()),
                None => {}
            },
            KeyCode::Char('u') => {
                if let Some(f) = self.fields.get(self.selected) {
                    self.session.unstage(&f.key.clone());
                    self.refresh();
                }
            }
            KeyCode::Char('r') => {
                let review = self.session.review();
                if review.is_empty() {
                    self.message = Some("nothing to review: no changes staged".into());
                } else {
                    self.mode = Mode::Review(review);
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => return Effect::Quit,
            _ => {}
        }
        Effect::None
    }

    fn edit_key(&mut self, code: KeyCode) {
        let Mode::Edit(input) = &mut self.mode else {
            return;
        };
        match code {
            KeyCode::Esc => {
                self.mode = Mode::List;
                self.message = None;
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => input.push(c),
            KeyCode::Tab => {
                // Cycle through the key's choices (the schema's, or its provider module's).
                if let Some(f) = self.fields.get(self.selected) {
                    let ids: Vec<String> = self
                        .session
                        .choices(&f.key, &Needs::default())
                        .into_iter()
                        .map(|c| c.id)
                        .collect();
                    if !ids.is_empty() {
                        let next = ids.iter().position(|i| i == input).map_or(0, |p| p + 1);
                        *input = ids[next % ids.len()].clone();
                    }
                }
            }
            KeyCode::Enter => {
                let text = input.clone();
                let key = self.fields[self.selected].key.clone();
                match self.session.stage(&key, &text) {
                    Ok(()) => {
                        self.mode = Mode::List;
                        self.message = None;
                        self.refresh();
                    }
                    // Stay in the editor so the text can be fixed.
                    Err(e) => self.message = Some(e.to_string()),
                }
            }
            _ => {}
        }
    }

    fn review_key(&mut self, code: KeyCode) -> Effect {
        match code {
            KeyCode::Char('y') => {
                let Mode::Review(review) = std::mem::replace(&mut self.mode, Mode::List) else {
                    return Effect::None;
                };
                Effect::Write(review.confirm())
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::List;
                Effect::None
            }
            _ => Effect::None,
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let [body, detail, footer] = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .areas(frame.area());
        match &self.mode {
            Mode::Review(r) => frame.render_widget(review_text(r), frame.area()),
            Mode::Done(lines) => frame.render_widget(
                Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false }),
                frame.area(),
            ),
            _ => {
                let items: Vec<ListItem> = self
                    .fields
                    .iter()
                    .map(|f| {
                        let now = f.current.as_deref().unwrap_or("<unset>");
                        let line = match &f.staged {
                            Some(s) => format!("{:<28} {now} -> {s}", f.key),
                            None => format!("{:<28} {now}", f.key),
                        };
                        ListItem::new(line)
                    })
                    .collect();
                let list = List::new(items)
                    .block(Block::bordered().title("values"))
                    .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
                let mut state = ListState::default().with_selected(Some(self.selected));
                frame.render_stateful_widget(list, body, &mut state);
                frame.render_widget(self.detail(), detail);
                frame.render_widget(Paragraph::new(self.hint()), footer);
            }
        }
    }

    fn detail(&self) -> Paragraph<'_> {
        let mut text = String::new();
        if let Some(f) = self.fields.get(self.selected) {
            text += &format!("{} ({}): {}", f.key, f.ty.name(), f.description);
            if f.restart {
                text += " [restart needed]";
            }
            if f.secret {
                text += " [secret]";
            }
            if !f.choices.is_empty() {
                text += &format!("\nchoices: {}", f.choices.join(", "));
            }
        }
        if let Mode::Edit(input) = &self.mode {
            text += &format!("\nnew value> {input}_");
        }
        if let Some(m) = &self.message {
            text += &format!("\n{m}");
        }
        Paragraph::new(text)
            .block(Block::bordered())
            .wrap(Wrap { trim: false })
    }

    fn hint(&self) -> &'static str {
        match self.mode {
            Mode::Edit(_) => "Enter stage   Tab next choice   Esc cancel",
            _ => "Up/Down select   Enter edit   u unstage   r review & confirm   q quit",
        }
    }
}

fn review_text(r: &Review) -> Paragraph<'static> {
    let mut lines = vec![
        "Review: nothing is written until you confirm.".to_owned(),
        String::new(),
    ];
    for c in r.changes() {
        lines.push(format!(
            "  {}: {} -> {}",
            c.key,
            c.from.as_deref().unwrap_or("<unset>"),
            c.to
        ));
    }
    if !r.restart_needed().is_empty() {
        lines.push(format!(
            "\nRestart needed for: {}",
            r.restart_needed().join(", ")
        ));
    }
    if !r.plan().actions.is_empty() {
        lines.push("\nPlan (shown, not run by this front end):".into());
        for a in &r.plan().actions {
            lines.push(format!("  - {}", a.description));
        }
    }
    lines.push("\ny write the file   n / Esc go back".into());
    Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false })
}
