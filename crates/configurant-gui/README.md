# configurant-gui

Graphical front end for [ConfigurAnt](https://github.com/ThinkersJournal/configurant), built on egui.

    configurant-gui <schema.toml> --file <config.toml>

Click a value (or Tab to it) and press Edit; Enter in the box stages the value, and a bad value stays in the box
with its error. Review and confirm shows every change; "Write the file" saves `<file>.bak` and then the file.
Nothing is written without that confirmation. A value marked `secret` cannot be edited here: it comes only
from its environment variable, and the window never holds or draws it. When a change moves a PersistAnt
backend, the window asks for the two endpoints (a path, or `env:NAME` for a URL; a URL is never typed), copies
the data, and writes the file only if the copy read back equal. Flags: `--require`, `--prefer`,
`--min-durability`, `--endpoint ID=ANSWER`. Keyboard-only use and screen-reader labels are covered by
tests; see `docs/GUI.md` in the repository for what is and is not accessible.

Licensed under MIT OR Apache-2.0.
