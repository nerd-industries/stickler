//! `nelko-mcp`: stdio MCP server for the Nelko P21. Logs go to stderr only.

use nelko_core::service::Nelko;
use nelko_mcp::NelkoServer;
use rmcp::ServiceExt;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nelko = Arc::new(Nelko::new()?);
    let service = NelkoServer::new(nelko).serve(rmcp::transport::stdio()).await.inspect_err(|e| eprintln!("nelko-mcp: {e}"))?;
    service.waiting().await?;
    Ok(())
}
