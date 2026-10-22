// SPDX-License-Identifier: MIT OR Apache-2.0
//! The graphical front end's state and drawing. It is a thin view over [`configurant::edit::Session`]:
//! every rule (type, range, choices, secrets, review, confirmation) lives there, and this crate only
//! turns clicks and keys into calls on it and draws what comes back. A confirmed change leaves as
//! [`Effect::Write`], which the caller carries out; this crate never writes a file.
//!
//! A secret is never in this crate's memory: `Session::fields` hands over `<redacted>`, so there is
//! nothing to render unmasked or to put on the clipboard, and no text box is ever opened for one.

use configurant::edit::{Confirmed, Field, Review, Session};
use configurant::{ActionKind, Needs};
use eframe::egui::{self, Key, RichText};

/// What the caller must do after a frame.
#[derive(Debug)]
pub enum Effect {
    None,
    Quit,
    /// The user confirmed: write `Confirmed::file_text` and report the result with [`App::wrote`].
    Write(Confirmed),
}

enum Mode {
    List,
    Edit,
    Review(Review),
    Done(Vec<String>),
}

/// What the user asked for this frame; applied after drawing so no widget borrows the state it changes.
enum Act {
    Select(usize),
    BeginEdit,
    Stage,
    CancelEdit,
    Choose(String),
    Unstage,
    OpenReview,
    Confirm,
    Back,
    Quit,
}

pub struct App<'a> {
    session: Session<'a>,
    fields: Vec<Field>,
    selected: usize,
    input: String,
    mode: Mode,
    message: Option<String>,
    focus_input: bool,
}

impl<'a> App<'a> {
    pub fn new(session: Session<'a>) -> Self {
        let fields = session.fields();
        App {
            session,
            fields,
            selected: 0,
            input: String::new(),
            mode: Mode::List,
            message: None,
            focus_input: false,
        }
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
                self.mode = Mode::Done(lines);
            }
        }
    }

    /// Draw one frame and say what the caller must do.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Effect {
        let (escape, quit_key) = ui.input(|i| {
            (
                i.key_pressed(Key::Escape),
                i.modifiers.command && i.key_pressed(Key::Q),
            )
        });
        let mut act = match self.mode {
            Mode::List => self.list_ui(ui),
            Mode::Edit => self.edit_ui(ui, escape),
            Mode::Review(_) => self.review_ui(ui, escape),
            Mode::Done(_) => self.done_ui(ui),
        };
        if quit_key {
            act = Some(Act::Quit);
        }
        match act {
            Some(act) => self.apply(act),
            None => Effect::None,
        }
    }

    fn refresh(&mut self) {
        self.fields = self.session.fields();
    }

    fn apply(&mut self, act: Act) -> Effect {
        match act {
            Act::Select(i) => {
                self.selected = i.min(self.fields.len().saturating_sub(1));
                self.message = None;
            }
            Act::BeginEdit => {
                self.message = None;
                match self.fields.get(self.selected) {
                    Some(f) if !f.editable => {
                        self.message = Some(format!(
                            "`{}` is a secret: it comes only from its environment variable",
                            f.key
                        ));
                    }
                    Some(_) => {
                        self.input.clear();
                        self.focus_input = true;
                        self.mode = Mode::Edit;
                    }
                    None => {}
                }
            }
            Act::Choose(id) => {
                self.input = id;
                self.focus_input = true;
            }
            Act::Stage => {
                if let Some(key) = self.fields.get(self.selected).map(|f| f.key.clone()) {
                    match self.session.stage(&key, &self.input) {
                        Ok(()) => {
                            self.mode = Mode::List;
                            self.message = None;
                            self.refresh();
                        }
                        // Stay in the editor so the text can be fixed.
                        Err(e) => {
                            self.message = Some(e.to_string());
                            self.focus_input = true;
                        }
                    }
                }
            }
            Act::CancelEdit => {
                self.mode = Mode::List;
                self.message = None;
            }
            Act::Unstage => {
                if let Some(key) = self.fields.get(self.selected).map(|f| f.key.clone()) {
                    self.session.unstage(&key);
                    self.refresh();
                }
            }
            Act::OpenReview => {
                let review = self.session.review();
                if review.is_empty() {
                    self.message = Some("nothing to review: no changes staged".into());
                } else {
                    self.message = None;
                    self.mode = Mode::Review(review);
                }
            }
            Act::Confirm => {
                if let Mode::Review(review) = std::mem::replace(&mut self.mode, Mode::List) {
                    return Effect::Write(review.confirm());
                }
            }
            Act::Back => self.mode = Mode::List,
            Act::Quit => return Effect::Quit,
        }
        Effect::None
    }

    fn message_ui(&self, ui: &mut egui::Ui) {
        if let Some(m) = &self.message {
            let r = ui.label(RichText::new(m).color(ui.visuals().warn_fg_color));
            // Announced by a screen reader when it appears, not only when focused.
            ui.ctx().accesskit_node_builder(r.id, |n| {
                n.set_live(egui::accesskit::Live::Assertive);
            });
        }
    }

    fn list_ui(&mut self, ui: &mut egui::Ui) -> Option<Act> {
        let mut act = None;
        ui.heading("Values");
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 170.0).max(80.0))
            .show(ui, |ui| {
                for (i, f) in self.fields.iter().enumerate() {
                    let now = f.current.as_deref().unwrap_or("<unset>");
                    let line = match &f.staged {
                        Some(s) => format!("{}   {now} -> {s}", f.key),
                        None => format!("{}   {now}", f.key),
                    };
                    let r = ui.selectable_label(self.selected == i, line);
                    if r.clicked() || r.gained_focus() {
                        act = Some(Act::Select(i));
                    }
                    if r.double_clicked() {
                        act = Some(Act::BeginEdit);
                    }
                }
            });
        ui.separator();
        if let Some(f) = self.fields.get(self.selected) {
            ui.label(RichText::new(format!("{} ({})", f.key, f.ty.name())).strong());
            ui.label(&f.description);
            if f.restart {
                ui.label("Changing this needs a restart.");
            }
            if f.secret {
                ui.label("Secret: set only through its environment variable.");
            }
            if !f.choices.is_empty() {
                ui.label(format!("Choices: {}", f.choices.join(", ")));
            }
        }
        self.message_ui(ui);
        ui.horizontal(|ui| {
            let editable = self.fields.get(self.selected).is_some_and(|f| f.editable);
            if ui
                .add_enabled(editable, egui::Button::new("Edit"))
                .clicked()
            {
                act = Some(Act::BeginEdit);
            }
            if ui.button("Unstage").clicked() {
                act = Some(Act::Unstage);
            }
            if ui.button("Review and confirm").clicked() {
                act = Some(Act::OpenReview);
            }
            if ui.button("Quit").clicked() {
                act = Some(Act::Quit);
            }
        });
        act
    }

    fn edit_ui(&mut self, ui: &mut egui::Ui, escape: bool) -> Option<Act> {
        let mut act = escape.then_some(Act::CancelEdit);
        let Some(f) = self.fields.get(self.selected) else {
            return Some(Act::CancelEdit);
        };
        ui.heading(format!("Edit {}", f.key));
        ui.label(format!("{}: {}", f.ty.name(), f.description));
        let label = ui.label("New value");
        let r = ui
            .add(egui::TextEdit::singleline(&mut self.input).desired_width(f32::INFINITY))
            .labelled_by(label.id);
        if self.focus_input {
            r.request_focus();
            self.focus_input = false;
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            act = Some(Act::Stage);
        }
        let choices = self.session.choices(&f.key, &Needs::default());
        if !choices.is_empty() {
            ui.label("Choices");
            for c in choices {
                let text = if c.reason.is_empty() {
                    c.id.clone()
                } else {
                    format!("{}: {}", c.id, c.reason)
                };
                if ui.button(text).clicked() {
                    act = Some(Act::Choose(c.id));
                }
            }
        }
        self.message_ui(ui);
        ui.horizontal(|ui| {
            if ui.button("Stage").clicked() {
                act = Some(Act::Stage);
            }
            if ui.button("Cancel").clicked() {
                act = Some(Act::CancelEdit);
            }
        });
        act
    }

    fn review_ui(&mut self, ui: &mut egui::Ui, escape: bool) -> Option<Act> {
        let mut act = escape.then_some(Act::Back);
        let Mode::Review(r) = &self.mode else {
            return act;
        };
        ui.heading("Review");
        ui.label("Nothing is written until you confirm.");
        for c in r.changes() {
            ui.label(format!(
                "{}: {} -> {}",
                c.key,
                c.from.as_deref().unwrap_or("<unset>"),
                c.to
            ));
        }
        if !r.restart_needed().is_empty() {
            ui.label(format!(
                "Restart needed for: {}",
                r.restart_needed().join(", ")
            ));
        }
        if !r.plan().actions.is_empty() {
            ui.label("Plan (shown, not run by this front end):");
            for a in &r.plan().actions {
                ui.label(format!("- {}", a.description));
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Write the file").clicked() {
                act = Some(Act::Confirm);
            }
            if ui.button("Go back").clicked() {
                act = Some(Act::Back);
            }
        });
        act
    }

    fn done_ui(&mut self, ui: &mut egui::Ui) -> Option<Act> {
        let Mode::Done(lines) = &self.mode else {
            return None;
        };
        ui.heading("Done");
        for l in lines {
            ui.label(l);
        }
        ui.button("Close").clicked().then_some(Act::Quit)
    }
}
