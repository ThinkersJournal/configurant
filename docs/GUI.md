<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# The graphical front end (`configurant-gui`)

Spec row: front ends are separate crates, each optional; delivery order is schema + library + CLI/env, then TUI,
then GUI (`CONFIGURANT-SPEC.md` section 3). The GUI is the same editor as the TUI in a window.

## Toolkit: egui through eframe 0.37

Chosen because it is MIT OR Apache-2.0, comes entirely from crates.io, needs no code generation and no
external build step, and its immediate-mode shape fits a view over `edit::Session` (the state lives in the
core, so the window keeps almost none). Slint is out (GPL or commercial). eframe is built with
`default-features = false` and `accesskit`, `default_fonts`, `glow`, `wayland`, `x11`: the OpenGL backend
runs on more machines than wgpu and compiles far less code.

## A thin view, no logic of its own

`configurant-gui` holds an `edit::Session` and draws it. Type, range, choices, provider modules, secrets,
review and confirmation are all decided by `Session`; the GUI turns a click or key into one call
(`stage`, `unstage`, `review`, `Review::confirm`) and draws the answer, including the named error. Its states are
the TUI's: list, edit, review, done. A confirmed change leaves as `Effect::Write`; the binary writes
`<file>.bak` and then the file (refusing if the backup exists), and the crate never writes a file or executes a
module's `Plan`. A `DataTransfer` step is listed as NOT DONE, as in the TUI, because the front end cannot open both
endpoints. Tables and maps are file-only and are not shown (`Session::fields` lists scalar values).

## Secrets

- `Session::fields` returns `<redacted>` for a secret, so the GUI never holds the value: nothing exists to
  render unmasked, copy to the clipboard or put in an accessibility label. The only text shown is the marker.
- No text box is ever opened for a secret: the Edit button is disabled for it and an explanation is shown.
  A secret comes only from its environment variable.
- Test: `a_secret_is_not_editable_and_never_drawn` loads a real secret, selects it and searches the whole
  accessibility tree for the value (absent) and the marker (present). It was shown to bite by enabling the button.

## Accessibility and keyboard use

What egui gives, with the `accesskit` feature: a tree of nodes for AccessKit, so Narrator, NVDA, VoiceOver
and Orca can read each widget's role, label, enabled and focused state. Every widget here has a text label;
the edit box is tied to its "New value" label (`labelled_by`); the value rows are focusable selectable
buttons; a message (an error, "nothing to review") is marked as an assertive live region so it is announced when it
appears rather than only when focused. Tab and Shift-Tab move between widgets, Space or Enter presses a button,
Enter in the edit box stages, Escape cancels an edit or leaves the review, and Ctrl+Q (Cmd+Q on macOS) quits.
The test `it_can_be_driven_from_the_keyboard_alone` selects, edits and stages with the keyboard only, and
all the tests find widgets by their accessibility labels, as a screen reader does.

What egui does not give: native platform widgets (it draws its own, so a screen reader sees AccessKit's
description of them, not a native control, and some platform behaviours such as text-field caret
navigation announcements and IME composition are less complete than in a native toolkit); arrow-key navigation
inside the value list (Tab moves through the rows instead); high-contrast and system font scaling follow egui's
theme and `pixels_per_point`, not the operating system's setting. Screen-reader behaviour has been tested here
only at the accessibility-tree level, not with a running screen reader: that is the next thing to check by hand
before calling the window accessible in practice.

## Out of scope for this row

Modules (the binary passes none, as the TUI does), tables and maps, executing a data transfer, scan assist.
