# configurant-gui

Graphical front end for [ConfigurAnt](https://github.com/ThinkersJournal/configurant), built on egui.

    configurant-gui <schema.toml> --file <config.toml>

Click a value (or Tab to it) and press Edit; Enter in the box stages the value, and a bad value stays in the box
with its error. Review and confirm shows every change; "Write the file" saves `<file>.bak` and then the file.
Nothing is written without that confirmation. A value marked `secret` cannot be edited here: it comes only
from its environment variable, and the window never holds or draws it. A data-transfer step in a module's plan
is listed as not done; this front end cannot run one. Keyboard-only use and screen-reader labels are covered by
tests; see `docs/GUI.md` in the repository for what is and is not accessible.

Licensed under MIT OR Apache-2.0.
