use std::sync::Arc;

use mcdev_ktlsp::McdevMixinProvider;
use tower_lsp_server::{LspService, Server};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("ktlsp=info")),
        )
        .init();
    let (service, socket) = LspService::new(|client| {
        ktlsp::lsp::Backend::with_analysis_providers(client, vec![Arc::new(McdevMixinProvider)])
    });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}
