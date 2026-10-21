// SPDX-License-Identifier: MIT OR Apache-2.0
//! `configurant`: check a schema, print its help, or explain where a value comes from.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use configurant::{Loader, Schema};

#[derive(Parser)]
#[command(name = "configurant", version, about = "Schema-first configuration")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse a schema and check it against itself.
    Check { schema: PathBuf },
    /// Print the help text a schema describes (secrets never shown). Named `describe` because clap owns `help`.
    Describe { schema: PathBuf },
    /// Load the configuration and explain where one value came from.
    Explain {
        schema: PathBuf,
        key: String,
        /// A configuration file to layer over the defaults.
        #[arg(long)]
        file: Option<PathBuf>,
        /// The application's own arguments, after `--`.
        #[arg(last = true)]
        app_args: Vec<String>,
    },
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("configurant: {msg}");
    ExitCode::from(2)
}

fn load_schema(path: &PathBuf) -> Result<Schema, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Schema::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { schema } => match load_schema(&schema) {
            Ok(s) => {
                println!(
                    "schema `{}` {} is valid: {} values",
                    s.project.name,
                    s.project.version,
                    s.values().len()
                );
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Command::Describe { schema } => match load_schema(&schema) {
            Ok(s) => {
                print!("{}", s.help());
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Command::Explain {
            schema,
            key,
            file,
            app_args,
        } => {
            let s = match load_schema(&schema) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let mut loader = Loader::new(&s)
                .env(std::env::vars().collect::<BTreeMap<_, _>>())
                .cli(app_args);
            if let Some(path) = file {
                match std::fs::read_to_string(&path) {
                    Ok(text) => loader = loader.file(&path.display().to_string(), &text),
                    Err(e) => return fail(format!("cannot read {}: {e}", path.display())),
                }
            }
            let cfg = match loader.build() {
                Ok(c) => c,
                Err(e) => return fail(e),
            };
            match cfg.explain(&key) {
                Ok(x) => {
                    println!("{x}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
    }
}
