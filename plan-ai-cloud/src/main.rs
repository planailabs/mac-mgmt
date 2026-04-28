use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "plan_ai_cloud=info,rmcp=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = plan_ai_cloud::Cli::parse();
    plan_ai_cloud::run(cli).await
}
