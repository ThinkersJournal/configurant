// SPDX-License-Identifier: MIT OR Apache-2.0
//! The pages: a pure function from what the wizard says (`Snapshot`) to HTML. No script, no outside resource;
//! one stylesheet served by the same server. Every string from a schema or a value goes through `esc`.

use std::fmt::Write;

use configurant::view::{FieldView, View};

use crate::actor::Snapshot;
use crate::guard::esc;

/// The stylesheet at `/style.css` (the page policy allows styles from this server only).
pub(crate) const CSS: &str = "\
body{font:16px/1.5 system-ui,sans-serif;max-width:40rem;margin:2rem auto;padding:0 1rem;color:#1b1b1b;background:#fff}\
h1{font-size:1.5rem}nav ol{display:flex;flex-wrap:wrap;gap:.25rem 1rem;list-style:none;padding:0;margin:0 0 1.5rem}\
nav li{color:#555}nav li.current{font-weight:700;color:#000}nav li.hidden{display:none}\
.field{margin:1.25rem 0}label{display:block;font-weight:600}small{display:block;color:#444}\
input[type=text],select{font:inherit;padding:.4rem;width:100%;box-sizing:border-box}\
button{font:inherit;padding:.5rem 1rem;margin:.25rem .5rem .25rem 0}\
.error{border-left:.4rem solid #b00020;padding:.5rem .75rem;background:#fdecee}\
.note{border-left:.4rem solid #666;padding:.5rem .75rem;background:#f1f1f1}\
table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #ccc;padding:.4rem;text-align:left}\
@media (prefers-color-scheme:dark){body{background:#161616;color:#eee}nav li{color:#aaa}nav li.current{color:#fff}small{color:#bbb}.error{background:#3a1519}.note{background:#262626}td,th{border-color:#444}}";

pub(crate) fn render(snap: &Snapshot) -> String {
    let view = &snap.view;
    let mut out = String::new();
    let title = view
        .wizard
        .as_ref()
        .map(|w| {
            if w.at_review {
                "Review".to_owned()
            } else {
                w.title.clone()
            }
        })
        .unwrap_or_default();
    let _ = write!(
        out,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{}</title><link rel=\"stylesheet\" href=\"/style.css\"></head><body><main>",
        esc(&title)
    );
    if let Some(Ok(path)) = &snap.finished {
        let _ = write!(
            out,
            "<h1>Saved</h1><p>The changes were written to <code>{}</code>. You can close this tab.</p>",
            esc(&path.display().to_string())
        );
    } else if let Some(Err(why)) = &snap.finished {
        let _ = write!(
            out,
            "<h1>Not saved</h1><p role=\"alert\" class=\"error\">{}</p>",
            esc(why)
        );
    } else {
        steps(&mut out, view);
        let _ = write!(out, "<h1>{}</h1>", esc(&title));
        if let Some(r) = &snap.refused {
            let _ = write!(
                out,
                "<p role=\"alert\" class=\"error\">{}</p>",
                esc(&r.error)
            );
        }
        if let Some(note) = view.wizard.as_ref().and_then(|w| w.note.as_ref()) {
            let _ = write!(out, "<p class=\"note\">{}</p>", esc(note));
        }
        out.push_str("<form method=\"post\" action=\"/event\" autocomplete=\"off\">");
        if view.wizard.as_ref().is_some_and(|w| w.at_review) {
            review(&mut out, view);
        } else {
            step(&mut out, snap);
        }
        buttons(&mut out, view);
        out.push_str("</form>");
    }
    out.push_str("</main></body></html>");
    out
}

fn steps(out: &mut String, view: &View) {
    let Some(w) = &view.wizard else { return };
    out.push_str("<nav aria-label=\"Progress\"><ol>");
    for s in &w.steps {
        let _ = write!(
            out,
            "<li class=\"{}\"{}>{}</li>",
            esc(s.state),
            if s.state == "current" {
                " aria-current=\"step\""
            } else {
                ""
            },
            esc(&s.title)
        );
    }
    out.push_str("</ol></nav>");
}

fn step(out: &mut String, snap: &Snapshot) {
    let view = &snap.view;
    let Some(w) = &view.wizard else { return };
    if !w.intro.is_empty() {
        let _ = write!(out, "<p>{}</p>", esc(&w.intro));
    }
    for (n, key) in w.keys.iter().enumerate() {
        let Some(field) = view.fields.iter().find(|f| &f.key == key) else {
            continue;
        };
        field_html(out, n, field, snap.plain.get(key).map(String::as_str));
    }
}

fn field_html(out: &mut String, n: usize, f: &FieldView, plain: Option<&str>) {
    let id = format!("f{n}");
    let _ = write!(
        out,
        "<div class=\"field\"><label for=\"{id}\">{}</label>",
        esc(&f.prompt)
    );
    if f.secret || !f.editable {
        // No control for a secret: it comes only from its environment variable or secret source.
        let _ = write!(
            out,
            "<p id=\"{id}\" class=\"note\">Set by the environment or a secret source; it cannot be typed here.</p></div>"
        );
        return;
    }
    let value = plain.unwrap_or("");
    if !f.choices.is_empty() || f.ty == "bool" {
        let _ = write!(out, "<select id=\"{id}\" name=\"v.{}\"", esc(&f.key));
        out.push('>');
        if f.ty == "bool" && f.choices.is_empty() {
            for id in ["true", "false"] {
                option(out, id, id, value);
            }
        } else {
            for c in &f.choices {
                let label = if c.reason.is_empty() {
                    c.id.clone()
                } else {
                    format!("{} - {}", c.id, c.reason)
                };
                option(out, &c.id, &label, value);
            }
        }
        out.push_str("</select>");
    } else {
        let mut hint_ids = String::new();
        if f.hint.is_some() {
            hint_ids.push_str(&format!("{id}h "));
        }
        if f.example.is_some() {
            hint_ids.push_str(&format!("{id}e"));
        }
        let describedby = if hint_ids.trim().is_empty() {
            String::new()
        } else {
            format!(" aria-describedby=\"{}\"", esc(hint_ids.trim()))
        };
        let _ = write!(
            out,
            "<input type=\"text\" id=\"{id}\" name=\"v.{}\" value=\"{}\"{describedby}>",
            esc(&f.key),
            esc(value)
        );
    }
    if let Some(h) = &f.hint {
        let _ = write!(out, "<small id=\"{id}h\">{}</small>", esc(h));
    }
    if let Some(e) = &f.example {
        let _ = write!(out, "<small id=\"{id}e\">For example: {}</small>", esc(e));
    }
    out.push_str("</div>");
}

fn option(out: &mut String, value: &str, label: &str, current: &str) {
    let _ = write!(
        out,
        "<option value=\"{}\"{}>{}</option>",
        esc(value),
        if value == current { " selected" } else { "" },
        esc(label)
    );
}

fn review(out: &mut String, view: &View) {
    let r = &view.review;
    if r.changes.is_empty() {
        out.push_str(
            "<p class=\"note\">Nothing has been changed, so there is nothing to save.</p>",
        );
        return;
    }
    out.push_str(
        "<table><thead><tr><th>Setting</th><th>Was</th><th>Becomes</th></tr></thead><tbody>",
    );
    for c in &r.changes {
        let _ = write!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&c.key),
            esc(c.from.as_deref().unwrap_or("(not set)")),
            esc(&c.to)
        );
    }
    out.push_str("</tbody></table>");
    if !r.restart.is_empty() {
        let _ = write!(
            out,
            "<p class=\"note\">These need a restart to take effect: {}.</p>",
            esc(&r.restart.join(", "))
        );
    }
    if !r.actions.is_empty() {
        out.push_str("<p class=\"note\">These follow-up steps are listed but <strong>not done</strong> here:</p><ul>");
        for a in &r.actions {
            let _ = write!(out, "<li>{}</li>", esc(&a.description));
        }
        out.push_str("</ul>");
    }
}

fn buttons(out: &mut String, view: &View) {
    let Some(w) = &view.wizard else { return };
    if w.can_back {
        out.push_str("<button type=\"submit\" name=\"action\" value=\"back\">Back</button>");
    }
    if w.can_next {
        out.push_str("<button type=\"submit\" name=\"action\" value=\"next\">Next</button>");
    }
    if w.at_review && !view.review.changes.is_empty() {
        out.push_str(
            "<button type=\"submit\" name=\"action\" value=\"confirm\">Save these changes</button>",
        );
    }
}
