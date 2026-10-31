// SPDX-License-Identifier: MIT OR Apache-2.0
//! The one thread that owns the wizard. A `Driver` borrows its `Config`, so it cannot sit in state shared by
//! requests; instead this thread builds everything inside itself and the handlers send it plain data.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use configurant::module::{Modules, Needs};
use configurant::view::{Driver, Event, Refusal, View};
use configurant::wizard::Wizard;
use configurant::{Config, edit::Session};
use tokio::sync::oneshot;

use crate::Outcome;

/// What the application gives the wizard to ask about. Built on the wizard's own thread (see [`crate::Web::start`]),
/// so nothing in it has to be `Send`.
pub struct Setup {
    /// The loaded configuration to edit.
    pub config: Config,
    /// The registered provider modules, if any.
    pub modules: Option<Modules>,
    /// What provider modules rank their choices for.
    pub needs: Needs,
    /// The configuration file the confirmed changes are written to (created if it does not exist).
    pub file: PathBuf,
}

/// What a page is drawn from.
pub(crate) struct Snapshot {
    pub view: View,
    /// The text each field starts with (`<redacted>` for a secret).
    pub plain: BTreeMap<String, String>,
    pub refused: Option<Refusal>,
    /// Set once the wizard is over: where the file went, or why it could not be written.
    pub finished: Option<Result<PathBuf, String>>,
}

/// What a form post asks for.
pub(crate) struct Submit {
    /// `(key, text)` of each text box, in page order.
    pub fields: Vec<(String, String)>,
    pub action: Action,
}

pub(crate) enum Action {
    Next,
    Back,
    Goto(String),
    Confirm,
}

pub(crate) enum Msg {
    Page(oneshot::Sender<Snapshot>),
    Submit(Submit, oneshot::Sender<Snapshot>),
}

/// Run until every sender is gone. `done` gets the outcome once, when the file is written or cannot be.
pub(crate) fn run(
    build: impl FnOnce() -> Setup,
    rx: mpsc::Receiver<Msg>,
    done: oneshot::Sender<Outcome>,
) {
    let setup = build();
    let session = Session::new(&setup.config, setup.modules.as_ref());
    let mut wizard = Wizard::new(Driver::new(session, setup.needs));
    let mut done = Some(done);
    let mut finished: Option<Result<PathBuf, String>> = None;
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Page(reply) => {
                let _ = reply.send(snapshot(&wizard, None, finished.clone()));
            }
            Msg::Submit(submit, reply) => {
                let refused = if finished.is_some() {
                    Some(over())
                } else {
                    apply(&mut wizard, submit, &setup.file, &mut finished)
                };
                if let Some(result) = &finished
                    && let Some(done) = done.take()
                {
                    let _ = done.send(match result {
                        Ok(path) => Outcome::Written(path.clone()),
                        Err(why) => Outcome::Failed(why.clone()),
                    });
                }
                let _ = reply.send(snapshot(&wizard, refused, finished.clone()));
            }
        }
    }
}

fn over() -> Refusal {
    Refusal {
        key: String::new(),
        error: "this wizard has finished; start it again to change more".into(),
    }
}

fn snapshot(
    wizard: &Wizard<'_>,
    refused: Option<Refusal>,
    finished: Option<Result<PathBuf, String>>,
) -> Snapshot {
    let view = wizard.view();
    let plain = view
        .fields
        .iter()
        .filter_map(|f| wizard.plain(&f.key).map(|p| (f.key.clone(), p)))
        .collect();
    Snapshot {
        view,
        plain,
        refused,
        finished,
    }
}

/// Stage each changed text box, then take the action. The first refusal stops everything after it.
fn apply(
    wizard: &mut Wizard<'_>,
    submit: Submit,
    file: &Path,
    finished: &mut Option<Result<PathBuf, String>>,
) -> Option<Refusal> {
    for (key, text) in submit.fields {
        // A box left as it was is not a change (and a secret is never a box).
        if wizard.plain(&key).as_deref() == Some(text.as_str()) {
            continue;
        }
        let reply = wizard.apply(Event::Set { key, text });
        if reply.refused.is_some() {
            return reply.refused;
        }
    }
    let event = match submit.action {
        Action::Next => Event::Next,
        Action::Back => Event::Back,
        Action::Goto(step) => Event::Goto { step },
        Action::Confirm => Event::Confirm,
    };
    let reply = wizard.apply(event);
    if reply.confirmed {
        *finished = Some(match wizard.take_confirmed() {
            Some(confirmed) => write(file, &confirmed),
            None => Err("the confirmed changes were already taken".into()),
        });
    }
    reply.refused
}

/// Write the confirmed text with the save function the terminal and window front ends use.
fn write(file: &Path, confirmed: &configurant::edit::Confirmed) -> Result<PathBuf, String> {
    let existing = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", file.display())),
    };
    let text = confirmed.file_text(&existing).map_err(|e| e.to_string())?;
    configurant::edit::save_with_backup(file, &existing, &text)?;
    Ok(file.to_path_buf())
}
