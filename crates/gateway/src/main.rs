use anyhow::{Context, Result};
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let token = env::var("DISCORD_TOKEN").context("DISCORD_TOKEN is not set")?;
    let application_id = env::var("DISCORD_APPLICATION_ID")
        .context("DISCORD_APPLICATION_ID is not set")?;

    let has_token = !token.is_empty();

    let shard_total = env::var("DISCORD_SHARD_TOTAL").ok();
    let shard_start = env::var("DISCORD_SHARD_START").ok();
    let shard_end = env::var("DISCORD_SHARD_END").ok();

    info!(
        discord_application_id = %application_id,
        has_token,
        shard_total = ?shard_total,
        shard_start = ?shard_start,
        shard_end = ?shard_end,
        "Gateway service bootstrapped"
    );

    Ok(())
}
