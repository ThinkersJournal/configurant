# configurant-tui

Terminal front end for [ConfigurAnt](https://github.com/ThinkersJournal/configurant).

    configurant-tui <schema.toml> --file <config.toml>

Up/Down select a value, Enter edits it (Tab cycles its choices), `u` unstages, `r` reviews, and `y` in the
review writes the file after saving `<file>.bak`. Nothing is written without that confirmation. A value marked
`secret` cannot be edited here: it comes only from its environment variable and is never drawn. A data-transfer
step in a module's plan is listed as not done; this front end cannot run one.

Licensed under MIT OR Apache-2.0.
