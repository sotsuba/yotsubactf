use anyhow::{Context, Result};
use reqwest::Client;
use crate::models::RawCtftimeEvent;

const UPCOMING_CTF_API_URL: &str = "https://ctftime.org/api/v1/events/?limit=50";

pub async fn fetch_upcoming(client: &Client) -> Result<Vec<RawCtftimeEvent>> {
    let events = client
        .get(UPCOMING_CTF_API_URL)
        .header("User-Agent", "Mozilla/5.0 (ctftime-discord-bot/v0.1.0 - contact: ntson.1303@gmail.com)")
        .send()
        .await
        .context("Failed to send request to CTFTime API")?
        .error_for_status()
        .context("CTFTime API returned an error status")?
        .json::<Vec<RawCtftimeEvent>>()
        .await
        .context("Failed to deserialize CTFTime JSON response")?;

    Ok(events)
}