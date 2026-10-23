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
    /// Find the places in Rust source (a directory, e.g. `src`) that ask for a key the schema does not
    /// define. Exits 1 if there are any. Writes nothing.
    Keys { schema: PathBuf, dir: PathBuf },
    /// Carry a configuration file to the schema's config version (or back, with `--to`). Prints the
    /// migrated text; changes the file only with `--write`, which first saves `<file>.bak`.
    Migrate {
        schema: PathBuf,
        file: PathBuf,
        /// The version of a file that has no `config_version` of its own.
        #[arg(long)]
        from: Option<u32>,
        /// Carry the file back down to this version instead.
        #[arg(long, conflicts_with = "from")]
        to: Option<u32>,
        /// Write the result over the file (after saving `<file>.bak`, which must not exist).
        #[arg(long)]
        write: bool,
    },
}

fn migrate(
    schema: &PathBuf,
    file: &PathBuf,
    from: Option<u32>,
    to: Option<u32>,
    write: bool,
) -> Result<(), String> {
    let s = load_schema(schema)?;
    let text = std::fs::read_to_string(file)
        .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let result = match to {
        Some(to) => s.migrate_back(&text, to),
        None => s.migrate(&text, from),
    }
    .map_err(|e| format!("{}: {e}", file.display()))?;
    for c in &result.changes {
        eprintln!("{c}");
    }
    if !write {
        print!("{}", result.text);
        return Ok(());
    }
    let mut bak = file.clone().into_os_string();
    bak.push(".bak");
    let bak = PathBuf::from(bak);
    if bak.exists() {
        return Err(format!("{} exists; move it away first", bak.display()));
    }
    std::fs::write(&bak, &text).map_err(|e| format!("cannot write {}: {e}", bak.display()))?;
    std::fs::write(file, &result.text)
        .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    eprintln!(
        "{}: config version {} -> {} (original saved as {})",
        file.display(),
        result.from,
        result.to,
        bak.display()
    );
    Ok(())
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
                // This binary has no secret source built in; say which ones the schema names.
                for d in s.values().iter().filter(|d| d.source.is_some()) {
                    println!(
                        "  `{}` names secret source `{}`: not built into this binary, so it comes only from the environment here",
                        d.key,
                        d.source.as_deref().unwrap_or_default()
                    );
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Command::Keys { schema, dir } => {
            let s = match load_schema(&schema) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            match configurant::keycheck::check_dir(&s, &dir) {
                Ok(r) if r.unknown.is_empty() => {
                    println!(
                        "{} key(s) asked for, all in the schema ({} computed name(s) not checked)",
                        r.checked, r.dynamic
                    );
                    ExitCode::SUCCESS
                }
                Ok(r) => {
                    print!("{}", r.render());
                    println!("{} unknown key(s)", r.unknown.len());
                    ExitCode::from(1)
                }
                Err(e) => fail(e),
            }
        }
        Command::Describe { schema } => match load_schema(&schema) {
            Ok(s) => {
                print!("{}", s.help());
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Command::Migrate {
            schema,
            file,
            from,
            to,
            write,
        } => match migrate(&schema, &file, from, to, write) {
            Ok(()) => ExitCode::SUCCESS,
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
