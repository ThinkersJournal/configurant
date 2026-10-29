// SPDX-License-Identifier: MIT OR Apache-2.0
//! The graphical front end's state and drawing. It is a thin view over [`configurant::edit::Session`]:
//! every rule (type, range, choices, secrets, review, confirmation) lives there, and this crate only
//! turns clicks and keys into calls on it and draws what comes back. A confirmed change leaves as
//! [`Effect::Write`], which the caller carries out; this crate never writes a file.
//!
//! A secret is never in this crate's memory: `Session::fields` hands over `<redacted>`, so there is
//! nothing to render unmasked or to put on the clipboard, and no text box is ever opened for one.

use std::collections::BTreeMap;

use configurant::edit::{Confirmed, Field, Review, Session};
use configurant::persistant::{
    EndpointSpec, PersistantModule, TransferEnd, TransferError, TransferJob, TransferOptions,
    TransferReport, parse_endpoint,
};
use configurant::{ActionKind, Needs};
use eframe::egui::{self, Key, RichText};

#[cfg(feature = "scan")]
mod scan_ui;
#[cfg(feature = "scan")]
pub use scan_ui::{ScanJob, ScanSetup};

/// What the caller must do after a frame.
#[derive(Debug)]
pub enum Effect {
    None,
    Quit,
    /// The user confirmed: write `Confirmed::file_text` and report the result with [`App::wrote`].
    Write(Confirmed),
    /// The file must not be written yet: run the job on a worker thread (`TransferJob::run`) and hand the
    /// result to [`App::transferred`], which says whether to write. Keep calling [`App::ui`] meanwhile.
    Transfer(TransferJob),
    /// Run the scan on a worker thread (`ScanJob::run`) and hand the result to [`App::scanned`]. Keep
    /// calling [`App::ui`] meanwhile.
    #[cfg(feature = "scan")]
    Scan(ScanJob),
    /// The user confirmed the schema change: write this text over the schema (saving a backup first) and
    /// report the result with [`App::schema_written`].
    #[cfg(feature = "scan")]
    WriteSchema(String),
}

/// What running a data transfer needs from the caller: the PersistAnt module (a clone of the registered
/// one), the project's needs, and endpoint answers given up front (`id` -> `path` or `env:NAME`).
pub struct TransferSetup {
    pub module: PersistantModule,
    pub needs: Needs,
    pub endpoints: BTreeMap<String, String>,
}

/// The two endpoint questions of one transfer.
struct Ask {
    confirmed: Confirmed,
    ids: [String; 2],
    answers: [String; 2],
    /// Which box takes focus on the next frame.
    focus: Option<usize>,
}

enum Mode {
    List,
    Edit,
    Review(Review),
    /// Asking for the source and target endpoints of a data transfer.
    Endpoints(Box<Ask>),
    /// Asking whether to write the file without copying (the answer is never the default).
    SkipCopy(Box<Ask>),
    /// The copy is running on the caller's worker thread.
    Copying(Confirmed, [String; 2]),
    Done(Vec<String>),
    /// The scan is running on the caller's worker thread.
    #[cfg(feature = "scan")]
    Scanning,
    #[cfg(feature = "scan")]
    Pick(Box<scan_ui::Pick>),
    #[cfg(feature = "scan")]
    SchemaReview(Box<scan_ui::SchemaReview>),
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
    StartCopy,
    CancelTransfer,
    AskSkip,
    SkipYes,
    SkipBack,
    #[cfg(feature = "scan")]
    StartScan,
    /// Leave the proposals (or, from the schema review, go back to them).
    #[cfg(feature = "scan")]
    PickBack,
    #[cfg(feature = "scan")]
    PickReview,
    #[cfg(feature = "scan")]
    WriteSchema,
}

pub struct App<'a> {
    session: Session<'a>,
    fields: Vec<Field>,
    selected: usize,
    input: String,
    mode: Mode,
    message: Option<String>,
    focus_input: bool,
    setup: Option<TransferSetup>,
    report: Option<String>,
    skipped_copy: bool,
    #[cfg(feature = "scan")]
    scan: Option<ScanSetup>,
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
            setup: None,
            report: None,
            skipped_copy: false,
            #[cfg(feature = "scan")]
            scan: None,
        }
    }

    /// Offer "Add from scan": propose schema entries for the environment variables the project reads.
    #[cfg(feature = "scan")]
    #[must_use]
    pub fn with_scan(mut self, setup: ScanSetup) -> Self {
        self.scan = Some(setup);
        self
    }

    /// The scan finished (or failed). A failure is shown and changes nothing.
    #[cfg(feature = "scan")]
    pub fn scanned(&mut self, result: Result<configurant_scan::Proposals, String>) {
        if !matches!(self.mode, Mode::Scanning) {
            return;
        }
        match result {
            Ok(found) => self.mode = Mode::Pick(Box::new(scan_ui::Pick::new(found))),
            Err(e) => {
                self.mode = Mode::List;
                self.message = Some(format!("scan failed, nothing was changed: {e}"));
            }
        }
    }

    /// The caller wrote (or failed to write) the schema for an [`Effect::WriteSchema`].
    #[cfg(feature = "scan")]
    pub fn schema_written(&mut self, result: Result<(), String>) {
        let Mode::SchemaReview(r) = std::mem::replace(&mut self.mode, Mode::List) else {
            return;
        };
        match result {
            Err(e) => {
                self.message = Some(format!("schema not written: {e}"));
                self.mode = Mode::SchemaReview(r);
            }
            Ok(()) => {
                let path = self
                    .scan
                    .as_ref()
                    .map_or_else(String::new, |s| s.schema_path.display().to_string());
                self.message = None;
                self.mode = Mode::Done(vec![
                    format!("{} entries added to {path}: {}.", r.envs.len(), r.envs.join(", ")),
                    "This session still has the old schema: start configurant-gui again to edit the new values.".into(),
                ]);
            }
        }
    }

    /// Let this front end run the data transfer in a plan (without this it only lists it).
    #[must_use]
    pub fn with_transfer(mut self, setup: TransferSetup) -> Self {
        self.setup = Some(setup);
        self
    }

    fn needs(&self) -> Needs {
        self.setup
            .as_ref()
            .map_or_else(Needs::default, |s| s.needs.clone())
    }

    /// The one data transfer in a confirmed plan, if this front end can run it.
    fn transfer_pair(&self, confirmed: &Confirmed) -> Option<[String; 2]> {
        self.setup.as_ref()?;
        let mut pairs = confirmed
            .plan()
            .actions
            .iter()
            .filter_map(|a| match &a.kind {
                ActionKind::DataTransfer { from, to } => Some([from.clone(), to.clone()]),
                _ => None,
            });
        let first = pairs.next()?;
        pairs.next().is_none().then_some(first)
    }

    /// The copy finished (or failed). `Ok` yields the write that was waiting for it; an error writes
    /// nothing, shows why, and leaves the staged edits for another try.
    pub fn transferred(&mut self, result: Result<TransferReport, TransferError>) -> Effect {
        let Mode::Copying(confirmed, _) = std::mem::replace(&mut self.mode, Mode::List) else {
            return Effect::None;
        };
        match result {
            Ok(report) => {
                self.report = Some(report.to_string());
                Effect::Write(confirmed)
            }
            Err(e) => {
                self.message = Some(format!("copy failed, nothing was written: {e}"));
                Effect::None
            }
        }
    }

    /// The caller wrote (or failed to write) the file for an [`Effect::Write`]. A data transfer was run
    /// before this if the app has a [`TransferSetup`]; otherwise the plan is only listed.
    pub fn wrote(&mut self, result: Result<(), String>, confirmed: &Confirmed) {
        match result {
            Err(e) => {
                self.message = Some(format!("not written: {e}"));
                self.mode = Mode::List;
            }
            Ok(()) => {
                let mut lines = vec![format!("{} change(s) written.", confirmed.changes().len())];
                for a in &confirmed.plan().actions {
                    lines.push(match (&a.kind, &self.report) {
                        (ActionKind::DataTransfer { from, to }, Some(report)) => {
                            format!("Copied `{from}` -> `{to}`: {report}")
                        }
                        (ActionKind::DataTransfer { from, to }, None) if self.skipped_copy => format!(
                            "NOT COPIED (you chose to write without copying): the data stays in `{from}`; `{to}` starts empty"
                        ),
                        (ActionKind::DataTransfer { from, to }, None) => format!(
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
            Mode::Endpoints(_) => self.endpoints_ui(ui, escape),
            Mode::SkipCopy(_) => self.skip_ui(ui, escape),
            Mode::Copying(..) => self.copying_ui(ui),
            Mode::Done(_) => self.done_ui(ui),
            #[cfg(feature = "scan")]
            Mode::Scanning => self.scanning_ui(ui),
            #[cfg(feature = "scan")]
            Mode::Pick(_) => self.pick_ui(ui, escape),
            #[cfg(feature = "scan")]
            Mode::SchemaReview(_) => self.schema_review_ui(ui, escape),
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
                            "`{}` is a secret: it comes only from its environment variable{}",
                            f.key,
                            f.source.as_ref().map_or_else(String::new, |s| format!(
                                " or its secret source `{s}`"
                            ))
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
                    let confirmed = review.confirm();
                    return match self.transfer_pair(&confirmed) {
                        Some(ids) => {
                            let prefilled = |id: &String| {
                                self.setup
                                    .as_ref()
                                    .and_then(|s| s.endpoints.get(id).cloned())
                                    .unwrap_or_default()
                            };
                            let answers = [prefilled(&ids[0]), prefilled(&ids[1])];
                            self.message = None;
                            self.mode = Mode::Endpoints(Box::new(Ask {
                                confirmed,
                                ids,
                                answers,
                                focus: Some(0),
                            }));
                            Effect::None
                        }
                        None => Effect::Write(confirmed),
                    };
                }
            }
            Act::StartCopy => return self.start_copy(),
            Act::CancelTransfer => {
                self.mode = Mode::List;
                self.message = Some("transfer cancelled: nothing was copied or written".into());
            }
            Act::AskSkip => {
                if let Mode::Endpoints(ask) = std::mem::replace(&mut self.mode, Mode::List) {
                    self.mode = Mode::SkipCopy(ask);
                }
            }
            Act::SkipBack => {
                if let Mode::SkipCopy(ask) = std::mem::replace(&mut self.mode, Mode::List) {
                    self.mode = Mode::Endpoints(ask);
                }
            }
            Act::SkipYes => {
                if let Mode::SkipCopy(ask) = std::mem::replace(&mut self.mode, Mode::List) {
                    self.skipped_copy = true;
                    return Effect::Write(ask.confirmed);
                }
            }
            Act::Back => self.mode = Mode::List,
            Act::Quit => return Effect::Quit,
            #[cfg(feature = "scan")]
            Act::StartScan => return self.start_scan(),
            #[cfg(feature = "scan")]
            Act::PickBack => match std::mem::replace(&mut self.mode, Mode::List) {
                Mode::SchemaReview(r) => {
                    self.message = None;
                    self.mode = Mode::Pick(Box::new(r.pick));
                }
                _ => self.message = None,
            },
            #[cfg(feature = "scan")]
            Act::PickReview => self.review_picks(),
            #[cfg(feature = "scan")]
            Act::WriteSchema => {
                if let Mode::SchemaReview(r) = &self.mode {
                    return Effect::WriteSchema(r.new_text.clone());
                }
            }
        }
        Effect::None
    }

    #[cfg(feature = "scan")]
    fn start_scan(&mut self) -> Effect {
        let Some(setup) = &self.scan else {
            return Effect::None;
        };
        // The new schema is only read on the next start: staged edits would be lost with it.
        if !self.session.review().is_empty() {
            self.message = Some("review or unstage your changes before adding from a scan".into());
            return Effect::None;
        }
        let job = ScanJob::new(setup);
        self.message = None;
        self.mode = Mode::Scanning;
        Effect::Scan(job)
    }

    /// Turn the ticked proposals into new schema text, held for the user's confirmation.
    #[cfg(feature = "scan")]
    fn review_picks(&mut self) {
        let (Mode::Pick(pick), Some(setup)) = (&self.mode, &self.scan) else {
            return;
        };
        let envs = pick.envs();
        if envs.is_empty() {
            self.message = Some("tick at least one proposal".into());
            return;
        }
        match pick.found.accept(&setup.schema_text, &envs) {
            Ok(new_text) => {
                if let Mode::Pick(pick) = std::mem::replace(&mut self.mode, Mode::List) {
                    self.message = None;
                    self.mode = Mode::SchemaReview(Box::new(scan_ui::SchemaReview {
                        pick: *pick,
                        new_text,
                        envs,
                    }));
                }
            }
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    /// Check both answers; on success hand the caller the job to run.
    fn start_copy(&mut self) -> Effect {
        let (Some(setup), Mode::Endpoints(ask)) = (&self.setup, &mut self.mode) else {
            return Effect::None;
        };
        let labels = ["source", "target"];
        let mut ends = Vec::new();
        #[allow(
            clippy::needless_range_loop,
            reason = "`i` indexes ids, answers and labels together"
        )]
        for i in 0..2 {
            let id = &ask.ids[i];
            let parsed = setup
                .module
                .endpoint_spec(id)
                .ok_or_else(|| format!("backend `{id}` is not offered by this build"))
                .and_then(|spec: EndpointSpec| parse_endpoint(&ask.answers[i], &spec));
            match parsed {
                Ok(endpoint) => ends.push(TransferEnd::new(id, endpoint)),
                Err(e) => {
                    // A typed URL may hold a password: do not leave it in the box.
                    if ask.answers[i].contains("://") {
                        ask.answers[i].clear();
                    }
                    ask.focus = Some(i);
                    self.message = Some(format!("{} endpoint: {e}", labels[i]));
                    return Effect::None;
                }
            }
        }
        let (to, from) = (ends.pop(), ends.pop());
        let (Some(from), Some(to)) = (from, to) else {
            return Effect::None;
        };
        let job = TransferJob::new(
            setup.module.clone(),
            from,
            to,
            setup.needs.clone(),
            TransferOptions::default(),
        );
        let (confirmed, ids) = (ask.confirmed.clone(), ask.ids.clone());
        self.message = None;
        self.mode = Mode::Copying(confirmed, ids);
        Effect::Transfer(job)
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
                    // Focus follows Tab and the arrow keys; the focused row is the selected one.
                    if (r.clicked() || r.has_focus()) && self.selected != i {
                        act = Some(Act::Select(i));
                        // The detail pane below was drawn for the old selection: draw once more.
                        ui.ctx().request_repaint();
                    }
                    if r.double_clicked() {
                        act = Some(Act::BeginEdit);
                    }
                }
            });
        ui.separator();
        if let Some(f) = self.fields.get(self.selected) {
            ui.label(RichText::new(format!("{} ({})", f.key, f.ty.name())).strong());
            ui.label(&f.prompt);
            if let Some(x) = &f.example {
                ui.label(format!("For example: {x}"));
            }
            if let Some(h) = &f.hint {
                ui.label(format!("Hint: {h}"));
            }
            if f.restart {
                ui.label("Changing this needs a restart.");
            }
            if f.secret {
                ui.label("Secret: set only through its environment variable.");
                if let Some(src) = &f.source {
                    ui.label(format!("Secret source: {src}"));
                }
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
            #[cfg(feature = "scan")]
            if self.scan.is_some() && ui.button("Add from scan").clicked() {
                act = Some(Act::StartScan);
            }
            if ui.button("Quit").clicked() {
                act = Some(Act::Quit);
            }
        });
        act
    }

    #[cfg(feature = "scan")]
    fn scanning_ui(&mut self, ui: &mut egui::Ui) -> Option<Act> {
        let root = self
            .scan
            .as_ref()
            .map_or_else(String::new, |s| s.root.display().to_string());
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(format!("Scanning `{root}`"));
        });
        ui.label(
            "The language server loads the project first, which can take a minute. Nothing is changed. Closing the window abandons the scan.",
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        None
    }

    #[cfg(feature = "scan")]
    fn pick_ui(&mut self, ui: &mut egui::Ui, escape: bool) -> Option<Act> {
        let mut act = escape.then_some(Act::PickBack);
        let Mode::Pick(pick) = &mut self.mode else {
            return act;
        };
        ui.heading("Add from scan");
        for l in pick.header() {
            ui.label(l);
        }
        let lines = pick.lines();
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 90.0).max(80.0))
            .show(ui, |ui| {
                for (line, checked) in lines.iter().zip(pick.checked.iter_mut()) {
                    ui.checkbox(checked, line);
                }
            });
        self.message_ui(ui);
        ui.horizontal(|ui| {
            if ui.button("Review the ticked entries").clicked() {
                act = Some(Act::PickReview);
            }
            if ui.button("Cancel").clicked() {
                act = Some(Act::PickBack);
            }
        });
        act
    }

    #[cfg(feature = "scan")]
    fn schema_review_ui(&mut self, ui: &mut egui::Ui, escape: bool) -> Option<Act> {
        let mut act = escape.then_some(Act::PickBack);
        let Mode::SchemaReview(r) = &self.mode else {
            return act;
        };
        ui.heading("Review the schema change");
        ui.label("The schema is not changed until you confirm.");
        let lines = r.pick.lines();
        for (l, c) in lines.iter().zip(&r.pick.checked) {
            if *c {
                ui.label(format!("add {l}"));
            }
        }
        if let Some(s) = &self.scan {
            let p = s.schema_path.display();
            ui.label(format!("{p} is saved to {p}.bak first, then rewritten."));
        }
        self.message_ui(ui);
        ui.horizontal(|ui| {
            if ui.button("Write the schema").clicked() {
                act = Some(Act::WriteSchema);
            }
            if ui.button("Go back").clicked() {
                act = Some(Act::PickBack);
            }
        });
        act
    }

    fn edit_ui(&mut self, ui: &mut egui::Ui, escape: bool) -> Option<Act> {
        let mut act = escape.then_some(Act::CancelEdit);
        let needs = self.needs();
        let Some(f) = self.fields.get(self.selected) else {
            return Some(Act::CancelEdit);
        };
        ui.heading(format!("Edit {}", f.key));
        ui.label(format!("{}: {}", f.ty.name(), f.prompt));
        if let Some(x) = &f.example {
            ui.label(format!("For example: {x}"));
        }
        if let Some(h) = &f.hint {
            ui.label(format!("Hint: {h}"));
        }
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
        let choices = self.session.choices(&f.key, &needs);
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
            ui.label(if self.setup.is_some() {
                "Plan (a data transfer runs first, then the file is written):"
            } else {
                "Plan (shown, not run by this front end):"
            });
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

    fn endpoints_ui(&mut self, ui: &mut egui::Ui, escape: bool) -> Option<Act> {
        let mut act = escape.then_some(Act::CancelTransfer);
        let Mode::Endpoints(ask) = &mut self.mode else {
            return act;
        };
        let spec_of = |id: &str| self.setup.as_ref().and_then(|s| s.module.endpoint_spec(id));
        let hint = |id: &str| match spec_of(id) {
            Some(EndpointSpec::Path) => "a directory or file path".to_owned(),
            Some(EndpointSpec::None) => "nothing: leave it empty".to_owned(),
            Some(EndpointSpec::Url { .. }) => {
                "env:NAME, the variable that holds the URL (a URL is never typed)".to_owned()
            }
            _ => "not offered by this build".to_owned(),
        };
        ui.heading(format!(
            "Move the data: copy `{}` -> `{}`",
            ask.ids[0], ask.ids[1]
        ));
        ui.label("The file is written only after the copy has finished and read back equal.");
        let names = ["Source endpoint of", "Target endpoint of"];
        let mut enter = false;
        for (i, name) in names.iter().enumerate() {
            let label = ui.label(format!("{name} `{}`", ask.ids[i]));
            let r = ui
                .add(egui::TextEdit::singleline(&mut ask.answers[i]).desired_width(f32::INFINITY))
                .labelled_by(label.id);
            if ask.focus == Some(i) {
                r.request_focus();
                ask.focus = None;
            }
            if r.lost_focus() && ui.input(|inp| inp.key_pressed(Key::Enter)) {
                enter = true;
            }
            ui.label(format!("({})", hint(&ask.ids[i])));
        }
        if enter {
            act = Some(Act::StartCopy);
        }
        self.message_ui(ui);
        ui.horizontal(|ui| {
            if ui.button("Copy and write the file").clicked() {
                act = Some(Act::StartCopy);
            }
            if ui.button("Write without copying...").clicked() {
                act = Some(Act::AskSkip);
            }
            if ui.button("Cancel").clicked() {
                act = Some(Act::CancelTransfer);
            }
        });
        act
    }

    fn skip_ui(&mut self, ui: &mut egui::Ui, escape: bool) -> Option<Act> {
        let mut act = escape.then_some(Act::SkipBack);
        let Mode::SkipCopy(ask) = &self.mode else {
            return act;
        };
        ui.heading("Write the file WITHOUT copying?");
        ui.label(format!(
            "The configuration will name `{to}` while all the stored data stays in `{from}`; `{to}` starts empty.",
            from = ask.ids[0],
            to = ask.ids[1]
        ));
        ui.horizontal(|ui| {
            if ui.button("Write without copying").clicked() {
                act = Some(Act::SkipYes);
            }
            if ui.button("Go back").clicked() {
                act = Some(Act::SkipBack);
            }
        });
        act
    }

    fn copying_ui(&mut self, ui: &mut egui::Ui) -> Option<Act> {
        let Mode::Copying(_, ids) = &self.mode else {
            return None;
        };
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(format!("Copying `{}` -> `{}`", ids[0], ids[1]));
        });
        ui.label(
            "Nothing is written yet. Closing the window abandons the copy and leaves the file as it is.",
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        None
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
