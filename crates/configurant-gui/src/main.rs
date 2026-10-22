// SPDX-License-Identifier: MIT OR Apache-2.0
//! `configurant-gui <schema> --file <config.toml>`: edit a configuration file in a window.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use configurant::edit::{Confirmed, Session};
use configurant::{Config, Loader, Schema};
use configurant_gui::{App, Effect};
use eframe::egui;

#[derive(Parser)]
#[command(
    name = "configurant-gui",
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
    eprintln!("configurant-gui: {msg}");
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

struct Window {
    app: App<'static>,
    file: PathBuf,
    existing: String,
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let effect = egui::CentralPanel::default()
            .show(ui, |ui| self.app.ui(ui))
            .inner;
        match effect {
            Effect::None => {}
            Effect::Quit => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
            Effect::Write(confirmed) => {
                let result = write(&self.file, &self.existing, &confirmed);
                self.app.wrote(result, &confirmed);
            }
        }
    }
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
    // eframe wants a 'static app, and the process exits with the window, so the config is leaked.
    let config: &'static Config = Box::leak(Box::new(config));
    let window = Window {
        app: App::new(Session::new(config, None)),
        file: cli.file,
        existing,
    };
    let result = eframe::run_native(
        "ConfigurAnt",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(window))),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}
