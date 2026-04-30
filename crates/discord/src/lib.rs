use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;

pub async fn send_message(
    client: &Client,
    token: &str,
    channel_id: &str,
    content: &str,
) -> Result<()> {
    let url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);

    client
        .post(url)
        .header("Authorization", format!("Bot {}", token))
        .json(&json!({ "content": content }))
        .send()
        .await
        .context("Failed to send Discord message")?
        .error_for_status()
        .context("Discord API returned an error status")?;

    Ok(())
}
