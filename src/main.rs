mod parser;
mod language_facts;
mod server;
mod services;
pub mod custom_capabilities;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| server::HtmlLanguageServer::new(client));

    Server::new(stdin, stdout, socket).serve(service).await;
}
