//! LSP server implementation using `tower-lsp`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::diagnostics;
use crate::state::DocumentState;

/// The LSP server backend. Holds shared state across all handler methods.
pub struct Backend {
    /// The tower-lsp client handle for sending notifications.
    pub client: Client,
    /// Per-document state, keyed by document URI.
    pub documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Derive the project root from a document URI.
    fn project_root(uri: &Url) -> PathBuf {
        uri.to_file_path()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Run the full diagnostic pipeline and publish results.
    async fn publish_diagnostics(&self, uri: Url) {
        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return;
        };

        let project_root = Self::project_root(&uri);
        let lsp_diags =
            diagnostics::compute_diagnostics(&doc.source, &doc.line_index, &project_root);

        self.client
            .publish_diagnostics(uri, lsp_diags, Some(doc.version))
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                // Completions and hover will be added in later phases.
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "cabalist-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("cabalist-lsp initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("cabalist-lsp shutting down");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let source = params.text_document.text;
        let version = params.text_document.version;

        {
            let mut docs = self.documents.write().await;
            docs.insert(uri.clone(), DocumentState::new(source, version));
        }

        // Publish diagnostics on open.
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // Full sync: the first (and only) change contains the entire document.
        if let Some(change) = params.content_changes.into_iter().next() {
            let mut docs = self.documents.write().await;
            if let Some(doc) = docs.get_mut(&uri) {
                doc.update(change.text, version);
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        // If save includes text, update our copy.
        if let Some(text) = params.text {
            let mut docs = self.documents.write().await;
            if let Some(doc) = docs.get_mut(&uri) {
                doc.update(text, doc.version);
            }
        }

        // Publish diagnostics on save.
        self.publish_diagnostics(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let mut docs = self.documents.write().await;
        docs.remove(&uri);

        // Clear diagnostics for closed files.
        self.client
            .publish_diagnostics(uri, Vec::new(), None)
            .await;
    }
}
