//! cabalist-lsp — Language Server Protocol implementation for .cabal files.
//!
//! Provides inline diagnostics, completions, and hover information for Haskell
//! `.cabal` files, powered by the cabalist parser and opinions engine.

mod convert;
mod diagnostics;
mod server;
mod state;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    // Initialize tracing.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // The LSP server communicates over stdio.
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(server::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
