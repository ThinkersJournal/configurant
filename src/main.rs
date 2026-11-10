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
    /// Print the resolved non-secret values as a host's settings block: `--format env`, `wrangler` (a
    /// `[vars]` table) or `compose` (an `environment:` mapping). A secret is named, never printed: only its
    /// variable and its source reference. Reads the schema defaults and `--file`; add `--from-env` to layer
    /// this process's environment too. Writes nothing.
    Project {
        schema: PathBuf,
        #[arg(long)]
        format: String,
        /// A configuration file to layer over the defaults.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Also layer this process's environment (a required secret must then be set in it).
        #[arg(long)]
        from_env: bool,
    },
    /// Print the schema as a JSON Schema for the configuration file, so an editor can complete keys and check
    /// types (secrets are left out). Writes nothing else.
    JsonSchema { schema: PathBuf },
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
        /// A library's schema fragment mounted into the schema, as `NAME=FILE` (repeatable), so its options
        /// are carried to the fragment's own config version too.
        #[arg(long = "mount", value_name = "NAME=FILE")]
        mounts: Vec<String>,
    },
    /// Copy the data of one PersistAnt backend into another and read the copy back (the same copy the TUI
    /// and GUI run). Changes neither the source nor any configuration file: edit the file yourself after
    /// it succeeds. Only the backends this build offers (`memory`, `fs`) are available.
    #[cfg(feature = "persistant-blocking")]
    Transfer {
        /// The backend id to copy from.
        #[arg(long)]
        from: String,
        /// The backend id to copy into.
        #[arg(long)]
        to: String,
        /// Where a backend opens from: `<backend id>=<path>` or `<backend id>=env:NAME` (the variable that
        /// holds a connection URL; a URL itself is refused). Repeatable; `memory` needs none.
        #[arg(long = "endpoint", value_name = "ID=ANSWER")]
        endpoint: Vec<String>,
        /// A capability the target backend must have, e.g. `kv.ttl` (repeatable).
        #[arg(long = "require", value_name = "CAPABILITY")]
        require: Vec<String>,
        /// A capability that matters, most important first (repeatable).
        #[arg(long = "prefer", value_name = "CAPABILITY")]
        prefer: Vec<String>,
        /// The lowest durability the target backend may have.
        #[arg(long, value_enum, default_value = "process")]
        min_durability: MinDurability,
    },
}

#[cfg(feature = "persistant-blocking")]
#[derive(Clone, Copy, clap::ValueEnum)]
enum MinDurability {
    Volatile,
    Process,
    Machine,
}

/// `configurant transfer`: the endpoints are checked before anything opens, and no answer is echoed (it may
/// be a URL typed by mistake).
#[cfg(feature = "persistant-blocking")]
fn transfer(
    from: &str,
    to: &str,
    endpoint: &[String],
    needs: configurant::Needs,
    min: MinDurability,
) -> Result<String, String> {
    use configurant::persistant::{
        Durability, PersistantModule, TransferEnd, TransferOptions, TransferPhase, parse_endpoint,
    };

    let module = PersistantModule::new().min_durability(match min {
        MinDurability::Volatile => Durability::Volatile,
        MinDurability::Process => Durability::Process,
        MinDurability::Machine => Durability::Machine,
    });
    let mut answers = BTreeMap::new();
    for item in endpoint {
        let Some((id, answer)) = item.split_once('=') else {
            return Err("--endpoint wants ID=ANSWER".to_owned());
        };
        if id != from && id != to {
            return Err(format!(
                "--endpoint: backend `{id}` is neither the source nor the target"
            ));
        }
        answers.insert(id.to_owned(), answer.to_owned());
    }
    let mut end = |id: &str| -> Result<TransferEnd, String> {
        let spec = module
            .endpoint_spec(id)
            .ok_or_else(|| format!("backend `{id}` is not offered by this build"))?;
        let answer = answers.remove(id).unwrap_or_default();
        let endpoint = parse_endpoint(&answer, &spec)
            .map_err(|e| format!("endpoint for `{id}` (--endpoint {id}=...): {e}"))?;
        Ok(TransferEnd::new(id, endpoint))
    };
    let (src, dst) = (end(from)?, end(to)?);
    let mut phase = None;
    let report = module
        .transfer_blocking_with_progress(
            &src,
            &dst,
            &needs,
            &TransferOptions::default().count_first(true),
            &mut |p| {
                if phase != Some(p.phase) {
                    phase = Some(p.phase);
                    match p.phase {
                        TransferPhase::Copy => eprintln!("copying..."),
                        TransferPhase::Verify => eprintln!("checking the copy..."),
                        _ => {}
                    }
                }
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "{report}\nthe source and every configuration file are unchanged; point the configuration at `{to}` yourself"
    ))
}

fn migrate(
    schema: &PathBuf,
    file: &PathBuf,
    from: Option<u32>,
    to: Option<u32>,
    write: bool,
    mounts: &[String],
) -> Result<(), String> {
    let s = load_schema_with(schema, mounts)?;
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
    load_schema_with(path, &[])
}

/// The schema with each `NAME=FILE` fragment mounted under NAME.
fn load_schema_with(path: &PathBuf, mounts: &[String]) -> Result<Schema, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut parsed = Vec::new();
    for m in mounts {
        let (name, file) = m
            .split_once('=')
            .filter(|(n, f)| !n.is_empty() && !f.is_empty())
            .ok_or_else(|| format!("--mount `{m}` must be NAME=FILE"))?;
        let fragment =
            std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
        parsed.push(configurant::Mount::new(name, &fragment));
    }
    Schema::parse_with(&text, &parsed).map_err(|e| format!("{}: {e}", path.display()))
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
        Command::JsonSchema { schema } => match load_schema(&schema) {
            Ok(s) => {
                print!("{}", s.json_schema());
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Command::Project {
            schema,
            format,
            file,
            from_env,
        } => {
            let Some(fmt) = configurant::HostFormat::parse(&format) else {
                return fail(format!(
                    "unknown format `{format}`: expected env, wrangler or compose"
                ));
            };
            let s = match load_schema(&schema) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let mut loader = Loader::new(&s);
            if from_env {
                loader = loader.env(std::env::vars().collect::<BTreeMap<_, _>>());
            }
            if let Some(path) = file {
                match std::fs::read_to_string(&path) {
                    Ok(text) => loader = loader.file(&path.display().to_string(), &text),
                    Err(e) => return fail(format!("cannot read {}: {e}", path.display())),
                }
            }
            match loader.build() {
                Ok(c) => {
                    print!("{}", c.project().render(fmt));
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
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
            mounts,
        } => match migrate(&schema, &file, from, to, write, &mounts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(e),
        },
        #[cfg(feature = "persistant-blocking")]
        Command::Transfer {
            from,
            to,
            endpoint,
            require,
            prefer,
            min_durability,
        } => {
            let needs = configurant::Needs {
                required: require,
                order: prefer,
            };
            match transfer(&from, &to, &endpoint, needs, min_durability) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
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
