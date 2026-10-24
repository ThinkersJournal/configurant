// SPDX-License-Identifier: MIT OR Apache-2.0
//! `configurant-gui <schema> --file <config.toml>`: edit a configuration file in a window.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::sync::mpsc;

use clap::{Parser, ValueEnum};
use configurant::edit::{Confirmed, Session};
use configurant::persistant::{
    Durability, PersistantModule, TransferError, TransferReport, parse_endpoint,
};
use configurant::{Config, Loader, Modules, Needs, Schema};
#[cfg(feature = "scan")]
use configurant_gui::ScanSetup;
use configurant_gui::{App, Effect, TransferSetup};
use eframe::egui;

#[derive(Clone, Copy, ValueEnum)]
enum MinDurability {
    Volatile,
    Process,
    Machine,
}

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
    /// A capability the chosen PersistAnt backend must have, e.g. `kv.ttl` (repeatable).
    #[arg(long = "require", value_name = "CAPABILITY")]
    require: Vec<String>,
    /// A capability that matters, most important first (repeatable); it orders the choices.
    #[arg(long = "prefer", value_name = "CAPABILITY")]
    prefer: Vec<String>,
    /// The lowest durability a listed backend may have.
    #[arg(long, value_enum, default_value = "process")]
    min_durability: MinDurability,
    /// Answer a transfer's endpoint question up front: `<backend id>=<path>` or `<backend id>=env:NAME`
    /// (the variable that holds a connection URL; a URL itself is refused). Repeatable.
    #[arg(long = "endpoint", value_name = "ID=ANSWER")]
    endpoint: Vec<String>,
    /// Offer "Add from scan": the Rust project whose environment reads become schema proposals.
    #[cfg(feature = "scan")]
    #[arg(long, value_name = "DIR")]
    scan: Option<PathBuf>,
    /// The language server command for `--scan`.
    #[cfg(feature = "scan")]
    #[arg(long, default_value = "rust-analyzer")]
    scan_server: String,
    /// How long the server may take to load the project and answer, in seconds.
    #[cfg(feature = "scan")]
    #[arg(long, default_value_t = 120)]
    scan_wait: u64,
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("configurant-gui: {msg}");
    ExitCode::from(2)
}

/// Write the confirmed file text, saving `<file>.bak` first when the file exists (refused if the backup does).
fn write(file: &Path, existing: &str, confirmed: &Confirmed) -> Result<(), String> {
    let text = confirmed.file_text(existing).map_err(|e| e.to_string())?;
    save(file, existing, &text)
}

/// Write `text` over `file`, saving `existing` to `<file>.bak` first when the file exists (refused if the backup does).
fn save(file: &Path, existing: &str, text: &str) -> Result<(), String> {
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

/// The `--endpoint` answers, checked now so a mistake is reported before the window opens. The answer is
/// never echoed: it may be a URL someone typed by mistake.
fn endpoint_answers(
    module: &PersistantModule,
    given: &[String],
) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for item in given {
        let Some((id, answer)) = item.split_once('=') else {
            return Err("--endpoint wants ID=ANSWER".to_owned());
        };
        let Some(spec) = module.endpoint_spec(id) else {
            return Err(format!(
                "--endpoint: backend `{id}` is not offered by this build"
            ));
        };
        if let Err(e) = parse_endpoint(answer, &spec) {
            return Err(format!("--endpoint for `{id}`: {e}"));
        }
        out.insert(id.to_owned(), answer.to_owned());
    }
    Ok(out)
}

type CopyResult = Result<TransferReport, TransferError>;

#[cfg(feature = "scan")]
type ScanResult = Result<configurant_scan::Proposals, String>;

struct Window {
    app: App<'static>,
    #[cfg(feature = "scan")]
    schema: PathBuf,
    #[cfg(feature = "scan")]
    schema_text: String,
    #[cfg(feature = "scan")]
    scan_tx: mpsc::Sender<ScanResult>,
    #[cfg(feature = "scan")]
    scan_rx: mpsc::Receiver<ScanResult>,
    file: PathBuf,
    existing: String,
    tx: mpsc::Sender<CopyResult>,
    rx: mpsc::Receiver<CopyResult>,
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut effect = egui::CentralPanel::default()
            .show(ui, |ui| self.app.ui(ui))
            .inner;
        if let Ok(result) = self.rx.try_recv() {
            effect = self.app.transferred(result);
        }
        #[cfg(feature = "scan")]
        if let Ok(result) = self.scan_rx.try_recv() {
            self.app.scanned(result);
        }
        match effect {
            Effect::None => {}
            Effect::Quit => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
            Effect::Transfer(job) => {
                // The copy runs off the frame loop, so the window keeps drawing.
                let (tx, ctx) = (self.tx.clone(), ui.ctx().clone());
                std::thread::spawn(move || {
                    let _ = tx.send(job.run());
                    ctx.request_repaint();
                });
            }
            #[cfg(feature = "scan")]
            Effect::Scan(job) => {
                // The server loads the project here, off the frame loop, so the window keeps drawing.
                let (tx, ctx) = (self.scan_tx.clone(), ui.ctx().clone());
                std::thread::spawn(move || {
                    let _ = tx.send(job.run());
                    ctx.request_repaint();
                });
            }
            #[cfg(feature = "scan")]
            Effect::WriteSchema(text) => {
                let result = save(&self.schema, &self.schema_text, &text);
                self.app.schema_written(result);
            }
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
    let needs = Needs {
        required: cli.require.clone(),
        order: cli.prefer.clone(),
    };
    let mut module = PersistantModule::new().min_durability(match cli.min_durability {
        MinDurability::Volatile => Durability::Volatile,
        MinDurability::Process => Durability::Process,
        MinDurability::Machine => Durability::Machine,
    });
    if !needs.required.is_empty() || !needs.order.is_empty() {
        module = module.needs(needs.clone());
    }
    let endpoints = match endpoint_answers(&module, &cli.endpoint) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    // eframe wants a 'static app, and the process exits with the window, so these are leaked.
    let mut modules = Modules::new();
    if let Err(e) = modules.register(Box::new(module.clone())) {
        return fail(e);
    }
    let modules: &'static Modules = Box::leak(Box::new(modules));
    let config = match Loader::new(&schema)
        .modules(modules)
        .file(&cli.file.display().to_string(), &existing)
        .env(std::env::vars().collect::<BTreeMap<_, _>>())
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    let config: &'static Config = Box::leak(Box::new(config));
    let (tx, rx) = mpsc::channel();
    #[allow(unused_mut, reason = "only `--features scan` adds the scan setup")]
    let mut app = App::new(Session::new(config, Some(modules))).with_transfer(TransferSetup {
        module,
        needs,
        endpoints,
    });
    #[cfg(feature = "scan")]
    if let Some(root) = &cli.scan {
        app = app.with_scan(ScanSetup {
            schema_path: cli.schema.clone(),
            schema_text: schema_text.clone(),
            root: root.clone(),
            server: cli.scan_server.clone(),
            wait: std::time::Duration::from_secs(cli.scan_wait),
        });
    }
    #[cfg(feature = "scan")]
    let (scan_tx, scan_rx) = mpsc::channel();
    let window = Window {
        app,
        #[cfg(feature = "scan")]
        schema: cli.schema,
        #[cfg(feature = "scan")]
        schema_text,
        #[cfg(feature = "scan")]
        scan_tx,
        #[cfg(feature = "scan")]
        scan_rx,
        file: cli.file,
        existing,
        tx,
        rx,
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
