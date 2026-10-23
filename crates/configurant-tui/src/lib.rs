// SPDX-License-Identifier: MIT OR Apache-2.0
//! The terminal front end's state machine and drawing. It holds an [`configurant::edit::Session`],
//! turns key presses into staged edits, shows a review, and asks for a confirmation; it never writes
//! a file itself. A confirmed change leaves as [`Effect::Write`], which the caller carries out.

use std::collections::BTreeMap;

use configurant::edit::{Confirmed, Field, Review, Session};
use configurant::persistant::{
    EndpointSpec, PersistantModule, TransferEnd, TransferError, TransferJob, TransferOptions,
    TransferReport, parse_endpoint,
};
use configurant::{ActionKind, Needs};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};

#[cfg(feature = "scan")]
mod scan_ui;
#[cfg(feature = "scan")]
pub use scan_ui::{ScanJob, ScanSetup};

/// What the caller must do after a key press.
#[derive(Debug)]
pub enum Effect {
    None,
    Quit,
    /// The user confirmed: write `Confirmed::file_text` and report the result with [`App::wrote`].
    Write(Confirmed),
    /// The file must not be written yet: run the job on a worker thread (`TransferJob::run`) and hand the
    /// result to [`App::transferred`], which says whether to write. Keep drawing and calling
    /// [`App::tick`] meanwhile.
    Transfer(TransferJob),
    /// Run the scan on a worker thread (`ScanJob::run`) and hand the result to [`App::scanned`]. Keep
    /// drawing and calling [`App::tick`] meanwhile.
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
    active: usize,
}

enum Mode {
    List,
    Edit(String),
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

pub struct App<'a> {
    session: Session<'a>,
    fields: Vec<Field>,
    selected: usize,
    mode: Mode,
    message: Option<String>,
    setup: Option<TransferSetup>,
    report: Option<String>,
    skipped_copy: bool,
    ticks: usize,
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
            mode: Mode::List,
            message: None,
            setup: None,
            report: None,
            skipped_copy: false,
            ticks: 0,
            #[cfg(feature = "scan")]
            scan: None,
        }
    }

    /// Offer "Add from scan" (key `a`): propose schema entries for the environment variables the project reads.
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
                self.mode = Mode::Done(vec![
                    format!("{} entries added to {path}: {}.", r.envs.len(), r.envs.join(", ")),
                    "This session still has the old schema: start configurant-tui again to edit the new values.".into(),
                    "Press any key to leave.".into(),
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

    /// Advance the spinner shown while a copy runs; call it on every loop turn.
    pub fn tick(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);
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

    fn refresh(&mut self) {
        self.fields = self.session.fields();
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
            Mode::Endpoints(_) => self.endpoints_key(key),
            Mode::SkipCopy(_) => self.skip_key(key.code),
            // Nothing to answer while the copy runs; Ctrl-C (above) still leaves.
            Mode::Copying(..) => Effect::None,
            Mode::Done(_) => Effect::Quit,
            #[cfg(feature = "scan")]
            Mode::Scanning => Effect::None,
            #[cfg(feature = "scan")]
            Mode::Pick(_) => self.pick_key(key.code),
            #[cfg(feature = "scan")]
            Mode::SchemaReview(_) => self.schema_review_key(key.code),
        }
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
        self.mode = Mode::Scanning;
        Effect::Scan(job)
    }

    #[cfg(feature = "scan")]
    fn pick_key(&mut self, code: KeyCode) -> Effect {
        let Mode::Pick(pick) = &mut self.mode else {
            return Effect::None;
        };
        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                pick.cursor = (pick.cursor + 1).min(pick.checked.len().saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => pick.cursor = pick.cursor.saturating_sub(1),
            KeyCode::Char(' ') => {
                self.message = None;
                if let Some(c) = pick.checked.get_mut(pick.cursor) {
                    *c = !*c;
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::List;
                self.message = None;
            }
            KeyCode::Enter => {
                let envs = pick.envs();
                if envs.is_empty() {
                    self.message = Some("tick at least one proposal with Space".into());
                    return Effect::None;
                }
                let Some(setup) = &self.scan else {
                    return Effect::None;
                };
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
            _ => {}
        }
        Effect::None
    }

    #[cfg(feature = "scan")]
    fn schema_review_key(&mut self, code: KeyCode) -> Effect {
        match code {
            KeyCode::Char('y') => match &self.mode {
                Mode::SchemaReview(r) => Effect::WriteSchema(r.new_text.clone()),
                _ => Effect::None,
            },
            KeyCode::Char('n') | KeyCode::Esc => {
                if let Mode::SchemaReview(r) = std::mem::replace(&mut self.mode, Mode::List) {
                    self.message = None;
                    self.mode = Mode::Pick(Box::new(r.pick));
                }
                Effect::None
            }
            _ => Effect::None,
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
            #[cfg(feature = "scan")]
            KeyCode::Char('a') => return self.start_scan(),
            KeyCode::Char('q') | KeyCode::Esc => return Effect::Quit,
            _ => {}
        }
        Effect::None
    }

    fn edit_key(&mut self, code: KeyCode) {
        let needs = self.needs();
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
                        .choices(&f.key, &needs)
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
                let confirmed = review.confirm();
                match self.transfer_pair(&confirmed) {
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
                            active: 0,
                        }));
                        Effect::None
                    }
                    None => Effect::Write(confirmed),
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::List;
                Effect::None
            }
            _ => Effect::None,
        }
    }

    fn endpoints_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Endpoints(ask) = &mut self.mode else {
            return Effect::None;
        };
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('w')
                && let Mode::Endpoints(ask) = std::mem::replace(&mut self.mode, Mode::List)
            {
                self.mode = Mode::SkipCopy(ask);
            }
            return Effect::None;
        }
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::List;
                self.message = Some("transfer cancelled: nothing was copied or written".into());
            }
            KeyCode::Backspace => {
                ask.answers[ask.active].pop();
            }
            KeyCode::Char(c) => ask.answers[ask.active].push(c),
            KeyCode::Tab => ask.active = 1 - ask.active,
            KeyCode::Enter if ask.active == 0 => ask.active = 1,
            KeyCode::Enter => return self.start_copy(),
            _ => {}
        }
        Effect::None
    }

    /// Check both answers; on success hand the caller the job to run.
    fn start_copy(&mut self) -> Effect {
        let (Some(setup), Mode::Endpoints(ask)) = (&self.setup, &mut self.mode) else {
            return Effect::None;
        };
        let labels = ["source", "target"];
        let mut ends = Vec::new();
        #[allow(clippy::needless_range_loop)] // `i` indexes ids, answers and labels together
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
                    // A typed URL may hold a password: do not leave it on screen.
                    if ask.answers[i].contains("://") {
                        ask.answers[i].clear();
                    }
                    ask.active = i;
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

    fn skip_key(&mut self, code: KeyCode) -> Effect {
        match code {
            KeyCode::Char('y') => {
                let Mode::SkipCopy(ask) = std::mem::replace(&mut self.mode, Mode::List) else {
                    return Effect::None;
                };
                self.skipped_copy = true;
                Effect::Write(ask.confirmed)
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                if let Mode::SkipCopy(ask) = std::mem::replace(&mut self.mode, Mode::List) {
                    self.mode = Mode::Endpoints(ask);
                }
                Effect::None
            }
            _ => Effect::None,
        }
    }

    fn endpoints_text(&self, ask: &Ask) -> String {
        let hint = |id: &str| match self.setup.as_ref().and_then(|s| s.module.endpoint_spec(id)) {
            Some(EndpointSpec::Path) => "a directory or file path".to_owned(),
            Some(EndpointSpec::None) => "nothing: leave it empty".to_owned(),
            Some(EndpointSpec::Url { .. }) => {
                "env:NAME, the variable that holds the URL (a URL is never typed)".to_owned()
            }
            _ => "not offered by this build".to_owned(),
        };
        let line = |i: usize, label: &str| {
            let cursor = if ask.active == i { "_" } else { "" };
            format!(
                "{} {label} `{}`: {}{cursor}\n    ({})",
                if ask.active == i { ">" } else { " " },
                ask.ids[i],
                ask.answers[i],
                hint(&ask.ids[i])
            )
        };
        let mut text = format!(
            "Move the data: copy `{}` -> `{}`.\nThe file is written only after the copy has finished and read back equal.\n\n{}\n{}\n",
            ask.ids[0],
            ask.ids[1],
            line(0, "source endpoint of"),
            line(1, "target endpoint of"),
        );
        if let Some(m) = &self.message {
            text += &format!("\n{m}\n");
        }
        text + "\nEnter next / copy   Tab switch   Ctrl-W write without copying   Esc cancel"
    }

    pub fn draw(&self, frame: &mut Frame) {
        let [body, detail, footer] = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .areas(frame.area());
        match &self.mode {
            Mode::Review(r) => {
                frame.render_widget(review_text(r, self.setup.is_some()), frame.area());
            }
            Mode::Endpoints(ask) => frame.render_widget(
                Paragraph::new(self.endpoints_text(ask)).wrap(Wrap { trim: false }),
                frame.area(),
            ),
            Mode::SkipCopy(ask) => frame.render_widget(
                Paragraph::new(format!(
                    "Write the file WITHOUT copying?\n\nThe configuration will name `{to}` while all the stored data stays in `{from}`; `{to}` starts empty.\n\ny write without copying   n / Esc go back",
                    from = ask.ids[0],
                    to = ask.ids[1]
                ))
                .wrap(Wrap { trim: false }),
                frame.area(),
            ),
            Mode::Copying(_, ids) => {
                let spin = ["|", "/", "-", "\\"][self.ticks % 4];
                frame.render_widget(
                    Paragraph::new(format!(
                        "Copying `{}` -> `{}` {spin}\n\nNothing is written yet. Ctrl-C abandons the copy and leaves the file as it is.",
                        ids[0], ids[1]
                    ))
                    .wrap(Wrap { trim: false }),
                    frame.area(),
                );
            }
            Mode::Done(lines) => frame.render_widget(
                Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false }),
                frame.area(),
            ),
            #[cfg(feature = "scan")]
            Mode::Scanning => {
                let spin = ["|", "/", "-", "\\"][self.ticks % 4];
                let root = self
                    .scan
                    .as_ref()
                    .map_or_else(String::new, |s| s.root.display().to_string());
                frame.render_widget(
                    Paragraph::new(format!(
                        "Scanning `{root}` {spin}\n\nThe language server loads the project first, which can take a minute. Nothing is changed. Ctrl-C abandons the scan."
                    ))
                    .wrap(Wrap { trim: false }),
                    frame.area(),
                );
            }
            #[cfg(feature = "scan")]
            Mode::Pick(pick) => self.draw_pick(frame, pick),
            #[cfg(feature = "scan")]
            Mode::SchemaReview(r) => {
                let mut lines = vec![
                    "Review: the schema is not changed until you confirm.".to_owned(),
                    String::new(),
                ];
                let ticked = r.pick.lines();
                for (l, c) in ticked.iter().zip(&r.pick.checked) {
                    if *c {
                        lines.push(format!("  add {}", l.trim_start_matches("[x] ")));
                    }
                }
                if let Some(s) = &self.scan {
                    lines.push(format!(
                        "\n{} is saved to {}.bak first, then rewritten.",
                        s.schema_path.display(),
                        s.schema_path.display()
                    ));
                }
                if let Some(m) = &self.message {
                    lines.push(format!("\n{m}"));
                }
                lines.push("\ny write the schema   n / Esc go back".into());
                frame.render_widget(
                    Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false }),
                    frame.area(),
                );
            }
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

    #[cfg(feature = "scan")]
    fn draw_pick(&self, frame: &mut Frame, pick: &scan_ui::Pick) {
        let header = pick.header();
        let [head, body, footer] = Layout::vertical([
            Constraint::Length(u16::try_from(header.len() + 1).unwrap_or(5)),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new(header.join("\n")).wrap(Wrap { trim: false }),
            head,
        );
        let items: Vec<ListItem> = pick.lines().into_iter().map(ListItem::new).collect();
        let list = List::new(items)
            .block(Block::bordered().title("proposed entries"))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default().with_selected(Some(pick.cursor));
        frame.render_stateful_widget(list, body, &mut state);
        let msg = self.message.as_deref().unwrap_or("");
        frame.render_widget(
            Paragraph::new(format!(
                "{msg}\nUp/Down select   Space tick   Enter review   Esc back"
            )),
            footer,
        );
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
            #[cfg(feature = "scan")]
            _ if self.scan.is_some() => {
                "Up/Down select   Enter edit   u unstage   r review & confirm   a add from scan   q quit"
            }
            _ => "Up/Down select   Enter edit   u unstage   r review & confirm   q quit",
        }
    }
}

fn review_text(r: &Review, runs_transfer: bool) -> Paragraph<'static> {
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
        lines.push(if runs_transfer {
            "\nPlan (a data transfer runs first, then the file is written):".into()
        } else {
            "\nPlan (shown, not run by this front end):".into()
        });
        for a in &r.plan().actions {
            lines.push(format!("  - {}", a.description));
        }
    }
    lines.push("\ny write the file   n / Esc go back".into());
    Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false })
}
