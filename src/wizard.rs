// SPDX-License-Identifier: MIT OR Apache-2.0
//! The step engine: a [`Wizard`] walks a schema's steps (`[[step]]`, or one per form section) over a
//! [`Driver`], so a terminal, a window and a browser page ask the same questions in the same order.
//!
//! It takes the driver's events plus `next`, `back` and `goto`, and its [`View`] carries a `wizard` part:
//! the step list (`done`, `current`, `todo` or `hidden`), the current step's title, intro and keys, and what
//! is possible next. A step with a `when` that does not hold is skipped both ways; the review is the last
//! stop and the only place `confirm` works. The saved position, [`WizardState`], is step ids only, never a
//! value, so a host can store it across a restart or a page reload and [`Wizard::resume`] from it. The staged
//! (not yet confirmed) values are not in it: the host keeps its own edits, or asks again.
//!
//! From a web page the events are JSON with a lowercase `event` tag and the other fields beside it:
//! `{"event":"set","key":"server.port","text":"9000"}`, `{"event":"next"}`, `{"event":"back"}`,
//! `{"event":"goto","step":"net"}`, `{"event":"confirm"}`. After a confirm, `take_confirmed()` gives the
//! changes once and `file_text(existing_text)` gives the new text of the config file, which the host writes.
//!
//! ```
//! use configurant::{Loader, Needs, Schema, edit::Session, view::{Driver, Event}, wizard::Wizard};
//! let schema = Schema::parse(r#"
//! project = { name = "app", version = "1", summary = "demo", schema_version = 1 }
//! value = [
//!   { key = "mode", type = "enum", choices = ["server", "client"], default = "client", description = "what to run" },
//!   { key = "server.port", type = "int", default = 8080, description = "listen port" },
//! ]
//! step = [
//!   { id = "mode", title = "What do you want to run?", keys = ["mode"] },
//!   { id = "net", title = "Where should it listen?", keys = ["server.port"], when = { key = "mode", equals = "server" } },
//! ]
//! "#).unwrap();
//! let config = Loader::new(&schema).build().unwrap();
//! let mut wizard = Wizard::new(Driver::new(Session::new(&config, None), Needs::default()));
//! let at = |w: &Wizard| w.view().wizard.unwrap().current; // None means the review
//! assert_eq!(at(&wizard).as_deref(), Some("mode"));
//! // The mode is "client", so the "net" step is hidden and `next` goes straight to the review.
//! wizard.apply(Event::Next);
//! assert_eq!(at(&wizard), None);
//! assert!(wizard.apply(Event::Confirm).refused.is_some()); // nothing was changed, so nothing to confirm
//!
//! // Choose server mode and the "net" step appears; set the port, reach the review, confirm.
//! wizard.apply(Event::Back);
//! wizard.apply(Event::Set { key: "mode".into(), text: "server".into() });
//! wizard.apply(Event::Next);
//! assert_eq!(at(&wizard).as_deref(), Some("net"));
//! wizard.apply(Event::Set { key: "server.port".into(), text: "9000".into() });
//! wizard.apply(Event::Next);
//! assert!(wizard.apply(Event::Confirm).confirmed);
//! // The new config file text (the argument is the file's current text, empty for a new file).
//! let text = wizard.take_confirmed().unwrap().file_text("").unwrap();
//! assert!(text.contains("port = 9000"));
//! ```

use serde::{Deserialize, Serialize};

use crate::error::suggestion;
use crate::schema::Step;
use crate::view::{Driver, Event, Refusal, Reply, View};

/// One step in the list a renderer draws as progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepView {
    pub id: String,
    pub title: String,
    /// `done`, `current`, `todo` or `hidden` (its `when` does not hold now).
    pub state: &'static str,
}

/// Where the person is, and what they can do next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WizardView {
    pub steps: Vec<StepView>,
    /// The current step's id; `None` at the review.
    pub current: Option<String>,
    pub at_review: bool,
    /// The current step's title, plain intro and keys (empty at the review).
    pub title: String,
    pub intro: String,
    pub keys: Vec<String>,
    pub can_back: bool,
    pub can_next: bool,
    /// Why the position is not the one saved (a stale step id on resume), once.
    pub note: Option<String>,
}

/// A saved position: step ids only, never a value.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WizardState {
    /// The current step; `None` is the review.
    #[serde(default)]
    pub current: Option<String>,
    /// Steps already reached, which `goto` may return to.
    #[serde(default)]
    pub reached: Vec<String>,
}

/// A [`Driver`] walked through steps.
pub struct Wizard<'a> {
    driver: Driver<'a>,
    state: WizardState,
    note: Option<String>,
}

impl<'a> Wizard<'a> {
    /// Start at the first step that is shown (the review if none is).
    pub fn new(driver: Driver<'a>) -> Self {
        let mut w = Wizard {
            driver,
            state: WizardState::default(),
            note: None,
        };
        let first = w.visible_steps().into_iter().next();
        w.go(first.map(|s| s.id));
        w
    }

    /// Continue from a saved position. A step id the schema no longer has starts at the first step, with a
    /// note in the view.
    pub fn resume(driver: Driver<'a>, state: WizardState) -> Self {
        let mut w = Wizard::new(driver);
        let all = w.all_steps();
        let known = |id: &str| all.iter().any(|s| s.id == id);
        match &state.current {
            Some(id) if !known(id) => {
                w.note = Some(format!(
                    "the saved step `{id}` no longer exists; starting at the first step"
                ));
            }
            Some(id) => {
                w.go(Some(id.clone()));
                w.settle();
            }
            None => w.state.current = None,
        }
        for id in state.reached {
            if known(&id) && !w.state.reached.contains(&id) {
                w.state.reached.push(id);
            }
        }
        w
    }

    /// The position to save.
    pub fn state(&self) -> WizardState {
        self.state.clone()
    }

    /// The driver's view with the `wizard` part filled in.
    pub fn view(&self) -> View {
        let mut view = self.driver.view();
        view.wizard = Some(self.wizard_view());
        view
    }

    /// The confirmed changes, once, after a `confirm` at the review.
    pub fn take_confirmed(&mut self) -> Option<crate::edit::Confirmed> {
        self.driver.take_confirmed()
    }

    /// Handle one event. A refused event changes nothing and the reply says why.
    pub fn apply(&mut self, event: Event) -> Reply {
        self.note = None;
        match event {
            Event::Set { .. } | Event::Unset { .. } => {
                let r = self.driver.apply(event);
                self.settle();
                self.reply(r.refused, false)
            }
            Event::Confirm => {
                if self.state.current.is_some() {
                    return self.refuse("", "confirm is for the review: use next to get there");
                }
                let r = self.driver.apply(Event::Confirm);
                self.reply(r.refused, r.confirmed)
            }
            Event::Next => {
                let Some(id) = self.state.current.clone() else {
                    return self.refuse("", "this is the review: there is no next step");
                };
                let next = self.visible_after(&id);
                self.go(next.map(|s| s.id));
                self.reply(None, false)
            }
            Event::Back => {
                let before = match &self.state.current {
                    Some(id) => self.visible_before(id),
                    None => self.visible_steps().pop(),
                };
                match before {
                    Some(s) => {
                        self.go(Some(s.id));
                        self.reply(None, false)
                    }
                    None => self.refuse("", "this is the first step: there is nothing before it"),
                }
            }
            Event::Goto { step } => {
                let all = self.all_steps();
                let Some(target) = all.iter().find(|s| s.id == step) else {
                    let hint = suggestion(&step, all.iter().map(|s| s.id.as_str()))
                        .map(|s| format!("; did you mean `{s}`?"))
                        .unwrap_or_default();
                    return self.refuse(&step, &format!("there is no step `{step}`{hint}"));
                };
                if !self.state.reached.contains(&target.id) {
                    return self.refuse(&step, &format!("step `{step}` has not been reached yet"));
                }
                if !self.shown(target) {
                    return self.refuse(&step, &format!("step `{step}` is hidden right now"));
                }
                let id = target.id.clone();
                self.go(Some(id));
                self.reply(None, false)
            }
        }
    }

    fn reply(&self, refused: Option<Refusal>, confirmed: bool) -> Reply {
        Reply {
            view: self.view(),
            refused,
            confirmed,
        }
    }

    fn refuse(&self, key: &str, error: &str) -> Reply {
        self.reply(
            Some(Refusal {
                key: key.to_owned(),
                error: error.to_owned(),
            }),
            false,
        )
    }

    fn go(&mut self, id: Option<String>) {
        if let Some(id) = &id
            && !self.state.reached.contains(id)
        {
            self.state.reached.push(id.clone());
        }
        self.state.current = id;
    }

    /// The steps in order, shown or not.
    fn all_steps(&self) -> Vec<Step> {
        self.driver.schema().effective_steps()
    }

    /// Is `step` shown, given the values now (staged if any, else current)?
    fn shown(&self, step: &Step) -> bool {
        step.visible(&|key: &str| self.driver.plain(key))
    }

    fn visible_steps(&self) -> Vec<Step> {
        self.all_steps()
            .into_iter()
            .filter(|s| self.shown(s))
            .collect()
    }

    fn visible_after(&self, id: &str) -> Option<Step> {
        let all = self.all_steps();
        let at = all.iter().position(|s| s.id == id)?;
        all.into_iter().skip(at + 1).find(|s| self.shown(s))
    }

    fn visible_before(&self, id: &str) -> Option<Step> {
        let all = self.all_steps();
        let at = all.iter().position(|s| s.id == id)?;
        all.into_iter().take(at).rev().find(|s| self.shown(s))
    }

    /// After an edit the current step may no longer be shown: move on to the next one that is.
    fn settle(&mut self) {
        let Some(id) = self.state.current.clone() else {
            return;
        };
        let all = self.all_steps();
        if all
            .iter()
            .find(|s| s.id == id)
            .is_some_and(|s| self.shown(s))
        {
            return;
        }
        let next = self.visible_after(&id);
        self.go(next.map(|s| s.id));
    }

    fn wizard_view(&self) -> WizardView {
        let all = self.all_steps();
        let cur = self
            .state
            .current
            .as_ref()
            .and_then(|id| all.iter().position(|s| &s.id == id));
        let steps = all
            .iter()
            .enumerate()
            .map(|(i, s)| StepView {
                id: s.id.clone(),
                title: s.title.clone(),
                state: if !self.shown(s) {
                    "hidden"
                } else if Some(i) == cur {
                    "current"
                } else if self.state.reached.contains(&s.id) && cur.is_none_or(|c| i < c) {
                    "done"
                } else {
                    "todo"
                },
            })
            .collect();
        let step = cur.map(|i| &all[i]);
        let can_back = match step {
            Some(s) => self.visible_before(&s.id).is_some(),
            None => !self.visible_steps().is_empty(),
        };
        WizardView {
            steps,
            current: self.state.current.clone(),
            at_review: self.state.current.is_none(),
            title: step.map(|s| s.title.clone()).unwrap_or_default(),
            intro: step.map(|s| s.intro.clone()).unwrap_or_default(),
            keys: step.map(|s| s.keys.clone()).unwrap_or_default(),
            can_back,
            can_next: step.is_some(),
            note: self.note.clone(),
        }
    }
}
