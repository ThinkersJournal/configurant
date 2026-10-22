// SPDX-License-Identifier: MIT OR Apache-2.0
//! `configurant-tui <schema> --file <config.toml>`: edit a configuration file in the terminal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use configurant::edit::{Confirmed, Session};
use configurant::{Loader, Schema};
use configurant_tui::{App, Effect};
use ratatui::crossterm::event;

#[derive(Parser)]
#[command(
    name = "configurant-tui",
    version,
    about = "Edit a configuration file against its schema"
)]
struct Cli {
    schema: PathBuf,
    /// The configuration file to edit (created if absent).
    #[arg(long)]
    file: PathBuf,
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("configurant-tui: {msg}");
    ExitCode::from(2)
}

/// Write the confirmed file text, saving `<file>.bak` first when the file exists (refused if the backup does).
fn write(file: &Path, existing: &str, confirmed: &Confirmed) -> Result<(), String> {
    let text = confirmed.file_text(existing).map_err(|e| e.to_string())?;
    if file.exists() {
        let mut bak = file.to_path_buf().into_os_string();
        bak.push(".bak");
        let bak = PathBuf::from(bak);
        if bak.exists() {
            return Err(format!("{} exists; move it away first", bak.display()));
        }
        std::fs::write(&bak, existing)
            .map_err(|e| format!("cannot write {}: {e}", bak.display()))?;
    }
    std::fs::write(file, text).map_err(|e| format!("cannot write {}: {e}", file.display()))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let schema_text = match std::fs::read_to_string(&cli.schema) {
        Ok(t) => t,
        Err(e) => return fail(format!("cannot read {}: {e}", cli.schema.display())),
    };
    let schema = match Schema::parse(&schema_text) {
        Ok(s) => s,
        Err(e) => return fail(format!("{}: {e}", cli.schema.display())),
    };
    let existing = match std::fs::read_to_string(&cli.file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return fail(format!("cannot read {}: {e}", cli.file.display())),
    };
    let config = match Loader::new(&schema)
        .file(&cli.file.display().to_string(), &existing)
        .env(std::env::vars().collect::<BTreeMap<_, _>>())
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    let mut app = App::new(Session::new(&config, None));
    let mut terminal = ratatui::init();
    let outcome = loop {
        if let Err(e) = terminal.draw(|f| app.draw(f)) {
            break Err(e.to_string());
        }
        match event::read() {
            Ok(event::Event::Key(key)) => match app.handle_key(key) {
                Effect::None => {}
                Effect::Quit => break Ok(()),
                Effect::Write(confirmed) => {
                    let result = write(&cli.file, &existing, &confirmed);
                    app.wrote(result, &confirmed);
                }
            },
            Ok(_) => {}
            Err(e) => break Err(e.to_string()),
        }
    };
    ratatui::restore();
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}
