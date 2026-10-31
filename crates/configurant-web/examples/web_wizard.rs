// SPDX-License-Identifier: MIT OR Apache-2.0
//! `web_wizard <schema.toml> --file <config.toml>`: edit a configuration file in your browser.
//!
//! Prints one address. Open it, answer the questions, review, and confirm; the file is written (after saving
//! `<file>.bak` if it exists) only then. The address works once, and the server stops when the file is written
//! or after 30 minutes with no request.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use configurant::{Loader, Needs, Schema};
use configurant_web::{Outcome, Setup, Web};

fn usage() -> ExitCode {
    eprintln!("usage: web_wizard <schema.toml> --file <config.toml>");
    ExitCode::from(2)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [schema_path, flag, file] = args.as_slice() else {
        return usage();
    };
    if flag != "--file" {
        return usage();
    }
    let schema_path = PathBuf::from(schema_path);
    let file = PathBuf::from(file);
    let schema_text = match std::fs::read_to_string(&schema_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {}: {e}", schema_path.display());
            return ExitCode::from(2);
        }
    };
    // Check the schema here so a mistake is a message, not a stopped wizard.
    if let Err(e) = Schema::parse(&schema_text) {
        eprintln!("{}: {e}", schema_path.display());
        return ExitCode::from(2);
    }
    let existing = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            eprintln!("cannot read {}: {e}", file.display());
            return ExitCode::from(2);
        }
    };
    let web = Web::start({
        let file = file.clone();
        move || {
            // Built on the wizard's own thread: the configuration never crosses threads.
            let schema = Schema::parse(&schema_text).expect("checked above");
            let config = Loader::new(&schema)
                .file(&file.display().to_string(), &existing)
                .env(std::env::vars().collect::<BTreeMap<_, _>>())
                .build()
                .unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(2)
                });
            Setup {
                config,
                modules: None,
                needs: Needs::default(),
                file,
            }
        }
    })
    .await;
    let web = match web {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    println!("Open this address in your browser: {}", web.launch_url());
    match web.finished().await {
        Ok(Outcome::Written(path)) => {
            println!("saved {}", path.display());
            ExitCode::SUCCESS
        }
        Ok(Outcome::Failed(why)) => {
            eprintln!("not saved: {why}");
            ExitCode::from(1)
        }
        Ok(_) => {
            eprintln!("stopped with nothing written");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
