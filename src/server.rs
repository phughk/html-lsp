use dashmap::DashMap;
use serde_json::{to_value, Value};
use tower_lsp::jsonrpc::{ErrorCode, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use crate::custom_capabilities::CustomCapabilities;
use crate::language_facts::data_manager::DataManager;
use crate::parser::html_document::{self, HtmlDocument};
use crate::services::{
    completion, document_highlight, document_links, document_symbols, folding, formatting, hover,
    linked_editing, rename, xpath,
};

pub struct HtmlLanguageServer {
    client: Client,
    documents: DashMap<Url, String>,
    data_manager: DataManager,
}

impl HtmlLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
            data_manager: DataManager::new(),
        }
    }

    fn parse_document(&self, text: &str) -> HtmlDocument {
        let void_elements = self.data_manager.get_void_elements();
        html_document::parse(text, &void_elements)
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for HtmlLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "<".to_string(),
                        "/".to_string(),
                        "\"".to_string(),
                        "'".to_string(),
                        "=".to_string(),
                        " ".to_string(),
                        ".".to_string(),
                        "&".to_string(),
                    ]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                rename_provider: Some(OneOf::Left(true)),
                linked_editing_range_provider: Some(
                    LinkedEditingRangeServerCapabilities::Simple(true),
                ),
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: Default::default(),
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["xpath_from_regex".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                experimental: Some(to_value(CustomCapabilities{ xpath_from_regex: true }).unwrap()),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "html-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "HTML LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.documents
            .insert(params.text_document.uri, params.text_document.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.documents
                .insert(params.text_document.uri, change.text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(text) = self.documents.get(uri) {
            let document = self.parse_document(&text);
            let highlights =
                document_highlight::find_document_highlights(&text, position, &document);
            Ok(Some(highlights))
        } else {
            Ok(None)
        }
    }

    async fn document_link(
        &self,
        params: DocumentLinkParams,
    ) -> Result<Option<Vec<DocumentLink>>> {
        let uri = &params.text_document.uri;

        if let Some(text) = self.documents.get(uri) {
            let links = document_links::find_document_links(&text, &self.data_manager, uri);
            Ok(Some(links))
        } else {
            Ok(None)
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(text) = self.documents.get(uri) {
            let document = self.parse_document(&text);
            Ok(hover::do_hover(&text, position, &document, &self.data_manager))
        } else {
            Ok(None)
        }
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = &params.text_document.uri;

        if let Some(text) = self.documents.get(uri) {
            let ranges = folding::get_folding_ranges(&text, &self.data_manager, None);
            Ok(Some(ranges))
        } else {
            Ok(None)
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        if let Some(text) = self.documents.get(uri) {
            let document = self.parse_document(&text);
            let symbols = document_symbols::find_document_symbols(&text, &document);
            Ok(Some(DocumentSymbolResponse::Nested(symbols)))
        } else {
            Ok(None)
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(text) = self.documents.get(uri) {
            let document = self.parse_document(&text);
            let result = completion::do_complete(&text, position, &document, &self.data_manager, uri);
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;

        if let Some(text) = self.documents.get(uri) {
            let edits = formatting::format_html(&text, &params.options);
            Ok(Some(edits))
        } else {
            Ok(None)
        }
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(text) = self.documents.get(uri) {
            let document = self.parse_document(&text);
            Ok(rename::do_rename(
                &text,
                position,
                &params.new_name,
                &document,
                uri,
            ))
        } else {
            Ok(None)
        }
    }

    async fn linked_editing_range(
        &self,
        params: LinkedEditingRangeParams,
    ) -> Result<Option<LinkedEditingRanges>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(text) = self.documents.get(uri) {
            let document = self.parse_document(&text);
            Ok(linked_editing::find_linked_editing_ranges(
                &text, position, &document,
            ))
        } else {
            Ok(None)
        }
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        match params.command.as_str() {
            "xpath_from_regex" => {
                // Arguments: [document_uri: string, regex_pattern: string]
                if params.arguments.len() < 2 {
                    return Err(tower_lsp::jsonrpc::Error {
                        code: ErrorCode::InvalidParams,
                        message: "xpath_from_regex requires 2 arguments: [document_uri, regex_pattern]".into(),
                        data: None,
                    });
                }

                let uri_str = params.arguments[0]
                    .as_str()
                    .ok_or_else(|| tower_lsp::jsonrpc::Error {
                        code: ErrorCode::InvalidParams,
                        message: "First argument (document_uri) must be a string".into(),
                        data: None,
                    })?;

                let pattern = params.arguments[1]
                    .as_str()
                    .ok_or_else(|| tower_lsp::jsonrpc::Error {
                        code: ErrorCode::InvalidParams,
                        message: "Second argument (regex_pattern) must be a string".into(),
                        data: None,
                    })?;

                let uri = Url::parse(uri_str).map_err(|e| tower_lsp::jsonrpc::Error {
                    code: ErrorCode::InvalidParams,
                    message: format!("Invalid URI: {e}").into(),
                    data: None,
                })?;

                let text = self.documents.get(&uri).ok_or_else(|| tower_lsp::jsonrpc::Error {
                    code: ErrorCode::InvalidParams,
                    message: format!("Document not open: {uri}").into(),
                    data: None,
                })?;

                let document = self.parse_document(&text);
                let matches = xpath::xpath_from_regex(&text, &document, pattern).map_err(|e| {
                    tower_lsp::jsonrpc::Error {
                        code: ErrorCode::InvalidParams,
                        message: e.into(),
                        data: None,
                    }
                })?;

                Ok(Some(to_value(matches).unwrap()))
            }
            _ => {
                self.client.log_message(MessageType::INFO, format!("Unknown command: {}", params.command)).await;
                Err(tower_lsp::jsonrpc::Error::new(ErrorCode::InvalidParams))
            }
        }
    }
}
