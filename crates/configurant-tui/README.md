# configurant-tui

Terminal front end for [ConfigurAnt](https://github.com/ThinkersJournal/configurant).

    configurant-tui <schema.toml> --file <config.toml>

Up/Down select a value, Enter edits it (Tab cycles its choices), `u` unstages, `r` reviews, and `y` in the
review writes the file after saving `<file>.bak`. Nothing is written without that confirmation. A value marked
`secret` cannot be edited here: it comes only from its environment variable and is never drawn. A data-transfer
step in a module's plan is listed as not done; this front end cannot run one.

## Add from scan (optional)

Built with `--features scan`, `configurant-tui <schema> --file <config> --scan <project dir>` adds the key `a`:
it asks rust-analyzer which environment variables the Rust project reads, lists the ones the schema lacks (with
a PARTIAL banner if the server could not resolve every call), and writes the ticked ones into the schema after
a review and a `<schema>.bak`. A secret-looking name is proposed as `secret` with no default. It needs
`rust-analyzer` on the PATH (`--scan-server` names another) and is refused while edits are staged; restart the
tool afterwards to edit the new values.

Licensed under MIT OR Apache-2.0.
