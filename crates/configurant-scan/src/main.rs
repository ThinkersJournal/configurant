// SPDX-License-Identifier: MIT OR Apache-2.0
//! `configurant-scan <schema> --root <dir>`: list the environment variables a Rust project reads that its
//! schema does not declare. Read-only unless `--accept` is given with `--write`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use configurant::Schema;
use configurant_scan::lsp::LspResolver;
use configurant_scan::scan;

#[derive(Parser)]
#[command(
    name = "configurant-scan",
    version,
    about = "Propose schema entries for the environment variables a Rust project reads"
)]
struct Cli {
    /// The project's schema (`configurant.toml`).
    schema: PathBuf,
    /// The project to scan.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// The language server command.
    #[arg(long, default_value = "rust-analyzer")]
    server: String,
    /// An argument for the language server command (repeatable).
    #[arg(long = "server-arg", value_name = "ARG", allow_hyphen_values = true)]
    server_arg: Vec<String>,
    /// How long the server may take to load the project and answer, in seconds.
    #[arg(long, default_value_t = 120)]
    wait: u64,
    /// Add the proposal for this environment variable to the schema text (repeatable). Prints the new
    /// schema; changes the file only with `--write`.
    #[arg(long = "accept", value_name = "ENV")]
    accept: Vec<String>,
    /// Write the `--accept` result over the schema (after saving `<schema>.bak`, which must not exist).
    #[arg(long, requires = "accept")]
    write: bool,
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("configurant-scan: {msg}");
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let text = match std::fs::read_to_string(&cli.schema) {
        Ok(t) => t,
        Err(e) => return fail(format!("cannot read {}: {e}", cli.schema.display())),
    };
    let schema = match Schema::parse(&text) {
        Ok(s) => s,
        Err(e) => return fail(format!("{}: {e}", cli.schema.display())),
    };
    let mut server = match LspResolver::start(
        &cli.server,
        &cli.server_arg,
        &cli.root,
        Duration::from_secs(cli.wait),
    ) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let found = match scan(&cli.root, &schema, &mut server) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    drop(server);
    if cli.accept.is_empty() {
        print!("{}", found.report());
        // 2: the list may be incomplete, so a script must not take it as "nothing to add".
        return ExitCode::from(if found.is_partial() { 2 } else { 0 });
    }
    let new_text = match found.accept(&text, &cli.accept) {
        Ok(t) => t,
        Err(e) => return fail(e),
    };
    if !cli.write {
        print!("{new_text}");
        return ExitCode::SUCCESS;
    }
    let mut bak = cli.schema.clone().into_os_string();
    bak.push(".bak");
    let bak = PathBuf::from(bak);
    if bak.exists() {
        return fail(format!("{} exists; move it away first", bak.display()));
    }
    if let Err(e) = std::fs::write(&bak, &text) {
        return fail(format!("cannot write {}: {e}", bak.display()));
    }
    if let Err(e) = std::fs::write(&cli.schema, &new_text) {
        return fail(format!("cannot write {}: {e}", cli.schema.display()));
    }
    eprintln!(
        "wrote {} (previous text in {})",
        cli.schema.display(),
        bak.display()
    );
    ExitCode::SUCCESS
}
