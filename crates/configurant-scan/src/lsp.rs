// SPDX-License-Identifier: MIT OR Apache-2.0
//! A [`Resolver`](crate::Resolver) backed by a language server through `lsp-bridge`. The scan API is
//! synchronous; this owns a Tokio runtime of its own.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lsp_bridge::{LspBridge, LspBridgeError, LspError, LspServerConfig};
use lsp_types::Position;
use url::Url;

use crate::{Resolver, ScanError};

/// A running language server.
pub struct LspResolver {
    rt: tokio::runtime::Runtime,
    bridge: LspBridge,
    id: String,
    command: String,
    opened: HashSet<PathBuf>,
    ready: bool,
    wait: Duration,
}

impl LspResolver {
    /// Start `command args...` rooted at `root`. `wait` bounds how long the first answer may take (a server
    /// loads the project before it can resolve anything).
    pub fn start(
        command: &str,
        args: &[String],
        root: &Path,
        wait: Duration,
    ) -> Result<LspResolver, ScanError> {
        let err = |reason: String| ScanError::Server {
            command: command.to_owned(),
            reason,
        };
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| err(e.to_string()))?;
        let root = plain(root.canonicalize().map_err(|e| err(format!("root: {e}")))?);
        let cfg = LspServerConfig::new()
            .command(command)
            .args(args.iter().cloned())
            .startup_timeout(wait)
            .request_timeout(wait)
            .root_path(&root)
            .workspace_folder(&root);
        let mut bridge = LspBridge::new();
        let id = "scan".to_owned();
        rt.block_on(async {
            bridge.register_server(id.clone(), cfg).await?;
            bridge.start_server(&id).await
        })
        .map_err(|e| err(e.to_string()))?;
        Ok(LspResolver {
            rt,
            bridge,
            id,
            command: command.to_owned(),
            opened: HashSet::new(),
            ready: false,
            wait,
        })
    }
}

impl Resolver for LspResolver {
    fn definition(
        &mut self,
        file: &Path,
        text: &str,
        line: u32,
        col: u32,
    ) -> Result<Option<String>, ScanError> {
        let err = |reason: String| ScanError::Server {
            command: self.command.clone(),
            reason,
        };
        let abs = plain(
            file.canonicalize()
                .map_err(|e| err(format!("{}: {e}", file.display())))?,
        );
        let uri = Url::from_file_path(&abs)
            .map_err(|()| err(format!("not a file path: {}", abs.display())))?
            .to_string();
        if self.opened.insert(abs) {
            self.rt
                .block_on(self.bridge.open_document(&self.id, &uri, text))
                .map_err(|e| err(e.to_string()))?;
        }
        let deadline = Instant::now() + self.wait;
        loop {
            let asked = self.rt.block_on(self.bridge.go_to_definition(
                &self.id,
                &uri,
                Position::new(line, col),
            ));
            // ContentModified is the server saying it reloaded the project under the request: ask again.
            let got = match asked {
                Ok(got) => got,
                Err(LspBridgeError::Lsp(LspError::ContentModified { .. })) => None,
                Err(e) => return Err(err(e.to_string())),
            };
            if let Some(loc) = got {
                self.ready = true;
                let raw = loc.uri.as_str();
                let path = match Url::parse(raw).ok().and_then(|u| u.to_file_path().ok()) {
                    Some(p) => p.to_string_lossy().replace('\\', "/"),
                    None => raw.to_owned(),
                };
                return Ok(Some(path));
            }
            if self.ready || Instant::now() >= deadline {
                return Ok(None);
            }
            self.rt
                .block_on(async { tokio::time::sleep(Duration::from_secs(1)).await });
        }
    }
}

impl Drop for LspResolver {
    fn drop(&mut self) {
        let _ = self.rt.block_on(self.bridge.shutdown());
    }
}

/// A canonical path without the Windows verbatim prefix (`\\?\C:\x` -> `C:\x`), which is no URI path.
fn plain(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix("\\\\?\\")) {
        Some(rest) => PathBuf::from(rest),
        None => p,
    }
}
