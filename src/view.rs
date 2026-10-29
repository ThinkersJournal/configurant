// SPDX-License-Identifier: MIT OR Apache-2.0
//! A serialisable view-model of an editing [`Session`], for renderers on any platform.
//!
//! A [`Driver`] owns a session. A renderer (terminal, desktop, browser, phone) asks for a [`View`] (plain
//! data: every field with its question, example, hint and choices, and what a confirmation would do),
//! sends [`Event`]s back, and gets a [`Reply`] with the new view. Everything here derives `Serialize`
//! (or `Deserialize` for events), so the renderer can live in another process or language. The rules are
//! the session's own: a secret is `<redacted>` and cannot be set, a bad value is refused with a named
//! error, and nothing is written or executed. The host takes the [`Confirmed`] from
//! [`Driver::take_confirmed`] after a `confirm` event and does the writing and the plan itself.
//!
//! A form draws `sections` (a title and the keys under it) and looks each key up in `fields`. A section's title
//! is the key's first segment (`server` for `server.port`) unless the schema gives the value `section = "Network"`;
//! a schema author marks a rarely-changed value with `advanced = true`, and the renderer decides how to fold it
//! (the view only says which fields are advanced). `file_only` lists tables and maps, which are edited in the file.
//!
//! In short: a renderer shows `view()`, turns each click or keystroke into an [`Event`] (JSON works), and
//! calls `apply`; the host writes the file once `take_confirmed` returns something.
//!
//! ```
//! use configurant::{Loader, Needs, Schema, edit::Session, view::{Driver, Event}};
//! let toml = r#"
//! project = { name = "app", version = "1", summary = "demo", schema_version = 1 }
//! value = [{ key = "server.port", type = "int", default = 8080, prompt = "Which port?", description = "port" }]
//! "#;
//! let schema = Schema::parse(toml).unwrap();
//! let config = Loader::new(&schema).build().unwrap();
//! // `Needs` ranks a provider module's choices (none here); `None` is the module registry (none here).
//! let mut driver = Driver::new(Session::new(&config, None), Needs::default());
//! println!("{}", driver.view().fields[0].prompt); // Which port?
//! println!("{}", driver.view().sections[0].title); // server: fields are grouped for a form
//! let event: Event = serde_json::from_str(r#"{"event":"set","key":"server.port","text":"9000"}"#).unwrap();
//! assert!(driver.apply(event).refused.is_none());
//! assert!(driver.apply(Event::Confirm).confirmed);
//! // `file_text` takes the config file's current text (empty for a new file) and keeps its comments.
//! let text = driver.take_confirmed().unwrap().file_text("").unwrap();
//! assert!(text.contains("port = 9000")); // the host writes this text to the config file
//! ```

use serde::{Deserialize, Serialize};

use crate::edit::{Confirmed, Session};
use crate::module::{ActionKind, Needs};

/// One selectable value and why it is (or is not) a good fit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChoiceView {
    pub id: String,
    pub reason: String,
}

/// One value, as a form shows it. A secret's `current` is `<redacted>` and it is not editable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldView {
    pub key: String,
    /// The type's name: `string`, `int`, `float`, `bool`, `path`, `enum` or `list`.
    pub ty: &'static str,
    pub description: String,
    /// The question to ask: the schema's `prompt`, else the description.
    pub prompt: String,
    pub example: Option<String>,
    pub hint: Option<String>,
    /// The section it is listed under (one of the view's `sections`).
    pub section: String,
    /// A form may fold it away until asked.
    pub advanced: bool,
    pub secret: bool,
    pub source: Option<String>,
    pub restart: bool,
    pub editable: bool,
    /// The allowed values (an enum's, or the provider module's list for the session's needs).
    pub choices: Vec<ChoiceView>,
    pub provider: Option<String>,
    pub current: Option<String>,
    pub staged: Option<String>,
}

/// One staged change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeView {
    pub key: String,
    pub from: Option<String>,
    pub to: String,
}

/// One step of a module's plan, for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionView {
    pub description: String,
    /// `note` or `data-transfer`.
    pub kind: &'static str,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// What a confirmation would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewView {
    pub changes: Vec<ChangeView>,
    pub restart: Vec<String>,
    pub actions: Vec<ActionView>,
}

/// A group of fields in a form. It names keys only; the values are in `fields`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionView {
    pub title: String,
    pub keys: Vec<String>,
}

/// A table or map, which only the configuration file can set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileOnlyView {
    pub key: String,
    /// `table` or `map`.
    pub kind: &'static str,
    pub description: String,
}

/// Everything a renderer draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    pub fields: Vec<FieldView>,
    /// The fields grouped for a form: sections in order of first appearance, keys in schema order.
    pub sections: Vec<SectionView>,
    /// Tables and maps: show where they are edited, not an editor.
    pub file_only: Vec<FileOnlyView>,
    pub review: ReviewView,
}

/// What a renderer can ask for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// Stage `text` as the new value of `key`.
    Set { key: String, text: String },
    /// Forget a staged change.
    Unset { key: String },
    /// The user said yes to the review.
    Confirm,
}

/// Why an event changed nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    /// The key the event named (empty when it named none).
    pub key: String,
    pub error: String,
}

/// The answer to one event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reply {
    pub view: View,
    pub refused: Option<Refusal>,
    /// True when this event confirmed the review; the host then calls [`Driver::take_confirmed`].
    pub confirmed: bool,
}

/// An editing session driven by events.
pub struct Driver<'a> {
    session: Session<'a>,
    needs: Needs,
    confirmed: Option<Confirmed>,
}

impl<'a> Driver<'a> {
    /// `needs` is what provider modules rank their choices for.
    pub fn new(session: Session<'a>, needs: Needs) -> Self {
        Driver {
            session,
            needs,
            confirmed: None,
        }
    }

    /// The current state, as data.
    pub fn view(&self) -> View {
        let fields: Vec<FieldView> = self
            .session
            .fields()
            .into_iter()
            .map(|f| FieldView {
                choices: self
                    .session
                    .choices(&f.key, &self.needs)
                    .into_iter()
                    .map(|c| ChoiceView {
                        id: c.id,
                        reason: c.reason,
                    })
                    .collect(),
                ty: f.ty.name(),
                key: f.key,
                description: f.description,
                prompt: f.prompt,
                example: f.example,
                hint: f.hint,
                section: f.section,
                advanced: f.advanced,
                secret: f.secret,
                source: f.source,
                restart: f.restart,
                editable: f.editable,
                provider: f.provider,
                current: f.current,
                staged: f.staged,
            })
            .collect();
        let mut sections: Vec<SectionView> = Vec::new();
        for f in &fields {
            match sections.iter_mut().find(|s| s.title == f.section) {
                Some(s) => s.keys.push(f.key.clone()),
                None => sections.push(SectionView {
                    title: f.section.clone(),
                    keys: vec![f.key.clone()],
                }),
            }
        }
        let file_only = self
            .session
            .file_only()
            .into_iter()
            .map(|t| FileOnlyView {
                key: t.key,
                kind: t.kind,
                description: t.description,
            })
            .collect();
        let review = self.session.review();
        let actions = review
            .plan()
            .actions
            .iter()
            .map(|a| {
                let (kind, from, to) = match &a.kind {
                    ActionKind::Note => ("note", None, None),
                    ActionKind::DataTransfer { from, to } => {
                        ("data-transfer", Some(from.clone()), Some(to.clone()))
                    }
                };
                ActionView {
                    description: a.description.clone(),
                    kind,
                    from,
                    to,
                }
            })
            .collect();
        View {
            fields,
            sections,
            file_only,
            review: ReviewView {
                changes: review
                    .changes()
                    .iter()
                    .map(|c| ChangeView {
                        key: c.key.clone(),
                        from: c.from.clone(),
                        to: c.to.clone(),
                    })
                    .collect(),
                restart: review.restart_needed().to_vec(),
                actions,
            },
        }
    }

    /// Handle one event. A refused event changes nothing and the reply says why.
    pub fn apply(&mut self, event: Event) -> Reply {
        let mut refused = None;
        let mut confirmed = false;
        match event {
            Event::Set { key, text } => {
                self.confirmed = None;
                if let Err(e) = self.session.stage(&key, &text) {
                    refused = Some(Refusal {
                        key,
                        error: e.to_string(),
                    });
                }
            }
            Event::Unset { key } => {
                self.confirmed = None;
                self.session.unstage(&key);
            }
            Event::Confirm => {
                let review = self.session.review();
                if review.is_empty() {
                    refused = Some(Refusal {
                        key: String::new(),
                        error: "there is nothing to confirm: no value has been changed".into(),
                    });
                } else {
                    self.confirmed = Some(review.confirm());
                    confirmed = true;
                }
            }
        }
        Reply {
            view: self.view(),
            refused,
            confirmed,
        }
    }

    /// The confirmed changes, once, after a `confirm` event. The host writes the file and runs the plan.
    pub fn take_confirmed(&mut self) -> Option<Confirmed> {
        self.confirmed.take()
    }
}
