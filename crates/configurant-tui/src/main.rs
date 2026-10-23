// SPDX-License-Identifier: MIT OR Apache-2.0
//! `configurant-tui <schema> --file <config.toml>`: edit a configuration file in the terminal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use configurant::edit::{Confirmed, Session};
use configurant::persistant::{Durability, PersistantModule, parse_endpoint};
use configurant::{Loader, Modules, Needs, Schema};
use configurant_tui::{App, Effect, TransferSetup};
use ratatui::crossterm::event;

#[derive(Clone, Copy, ValueEnum)]
enum MinDurability {
    Volatile,
    Process,
    Machine,
}

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

/// The `--endpoint` answers, checked now so a mistake is reported before the screen is taken over. The
/// answer is never echoed: it may be a URL someone typed by mistake.
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
    let mut modules = Modules::new();
    if let Err(e) = modules.register(Box::new(module.clone())) {
        return fail(e);
    }

    let config = match Loader::new(&schema)
        .modules(&modules)
        .file(&cli.file.display().to_string(), &existing)
        .env(std::env::vars().collect::<BTreeMap<_, _>>())
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    let mut app = App::new(Session::new(&config, Some(&modules))).with_transfer(TransferSetup {
        module,
        needs,
        endpoints,
    });
    let (tx, rx) = mpsc::channel();
    let mut terminal = ratatui::init();
    let outcome = loop {
        if let Err(e) = terminal.draw(|f| app.draw(f)) {
            break Err(e.to_string());
        }
        let mut effect = Effect::None;
        match event::poll(Duration::from_millis(100)) {
            Ok(true) => match event::read() {
                Ok(event::Event::Key(key)) => effect = app.handle_key(key),
                Ok(_) => {}
                Err(e) => break Err(e.to_string()),
            },
            Ok(false) => {}
            Err(e) => break Err(e.to_string()),
        }
        app.tick();
        if let Ok(result) = rx.try_recv() {
            effect = app.transferred(result);
        }
        match effect {
            Effect::None => {}
            Effect::Quit => break Ok(()),
            Effect::Write(confirmed) => {
                let result = write(&cli.file, &existing, &confirmed);
                app.wrote(result, &confirmed);
            }
            Effect::Transfer(job) => {
                // The copy runs here, off the screen's thread, so the spinner keeps turning.
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(job.run());
                });
            }
        }
    };
    ratatui::restore();
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}
