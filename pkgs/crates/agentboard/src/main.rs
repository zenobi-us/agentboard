use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    agentboard::cli::run().await
}
