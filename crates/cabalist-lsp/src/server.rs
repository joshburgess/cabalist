//! LSP server implementation using `tower-lsp`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{OnceCell, RwLock};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::completions;
use crate::diagnostics;
use crate::hover;
use crate::state::DocumentState;

/// The LSP server backend. Holds shared state across all handler methods.
pub struct Backend {
    /// The tower-lsp client handle for sending notifications.
    pub client: Client,
    /// Per-document state, keyed by document URI.
    pub documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
    /// Lazily loaded Hackage package index (from cache).
    pub hackage_index: OnceCell<Option<cabalist_hackage::HackageIndex>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            hackage_index: OnceCell::new(),
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

/// Load the Hackage index from the default cache location.
fn load_hackage_index() -> Option<cabalist_hackage::HackageIndex> {
    let dirs = directories::ProjectDirs::from("", "", "cabalist")?;
    let cache_path = dirs.cache_dir().join("index.json");
    cabalist_hackage::HackageIndex::load_from_cache(&cache_path).ok()
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
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![":".into(), " ".into(), "-".into(), ",".into()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: crate::semantic_tokens::legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            ..Default::default()
                        },
                    ),
                ),
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
        // Eagerly load the hackage index in the background.
        let _ = self
            .hackage_index
            .get_or_init(|| std::future::ready(load_hackage_index()))
            .await;
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

        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        if let Some(change) = params.content_changes.into_iter().next() {
            let mut docs = self.documents.write().await;
            if let Some(doc) = docs.get_mut(&uri) {
                doc.update(change.text, version);
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        if let Some(text) = params.text {
            let mut docs = self.documents.write().await;
            if let Some(doc) = docs.get_mut(&uri) {
                doc.update(text, doc.version);
            }
        }

        self.publish_diagnostics(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let mut docs = self.documents.write().await;
        docs.remove(&uri);

        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let hackage = self.hackage_index.get().and_then(|opt| opt.as_ref());
        let items = completions::completions(doc, position, hackage);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let hackage = self.hackage_index.get().and_then(|opt| opt.as_ref());
        Ok(hover::hover(doc, position, hackage))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        Ok(crate::rename::prepare_rename(
            &doc.source,
            &doc.line_index,
            position,
        ))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        Ok(crate::rename::rename(
            &doc.source,
            &doc.line_index,
            &uri,
            position,
            &params.new_name,
        ))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let result = crate::definition::goto_definition(&doc.source, &doc.line_index, position);
        match result {
            Some(mut loc) => {
                loc.uri = uri; // Replace placeholder with actual document URI.
                Ok(Some(GotoDefinitionResponse::Scalar(loc)))
            }
            None => Ok(None),
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let project_root = Self::project_root(&uri);
        let actions =
            crate::actions::code_actions(doc, &uri, &project_root, &params.range, &params.context);
        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let symbols = crate::symbols::document_symbols(&doc.source, &doc.line_index);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let hackage = self.hackage_index.get().and_then(|opt| opt.as_ref());
        let hints =
            crate::inlay_hints::inlay_hints(&doc.source, &doc.line_index, &params.range, hackage);
        if hints.is_empty() {
            Ok(None)
        } else {
            Ok(Some(hints))
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let tokens = crate::semantic_tokens::semantic_tokens(&doc.source, &doc.line_index);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let project_root = Self::project_root(&uri);
        let edits = crate::formatting::format_document(&doc.source, &doc.line_index, &project_root);
        if edits.is_empty() {
            Ok(None)
        } else {
            Ok(Some(edits))
        }
    }
}
