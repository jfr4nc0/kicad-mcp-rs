mod discovery;
mod server;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use server::KicadMcp;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // MCP uses stdout for protocol frames. Logs must stay on stderr.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "starting KiCad MCP server"
    );

    let service = KicadMcp.serve(stdio()).await.inspect_err(|error| {
        tracing::error!(?error, "MCP server failed");
    })?;

    service.waiting().await?;
    Ok(())
}
