use crate::SharedState;
use async_trait::async_trait;
use html_scraper::Html;
use reqwest_middleware::ClientWithMiddleware as Client;
use shared::CtfResult;
use tracing::info;

use super::SchedulerTask;
use crate::ctftime::writeups::fetch_recent_writeups;

pub struct WriteupsTask;

#[async_trait]
impl SchedulerTask for WriteupsTask {
    fn name(&self) -> &'static str {
        "writeups"
    }
    async fn run_once(&self, state: &SharedState) -> CtfResult<()> {
        run_once(state).await
    }
}

pub async fn run_once(state: &SharedState) -> CtfResult<()> {
    // 1. Fetch recent writeups from RSS
    let recent = fetch_recent_writeups(&state.http).await?;
    info!(count = recent.len(), "Fetched recent writeups");

    for wu in recent {
        // Try to resolve event_id from the database by title matching
        let mut wu = wu;
        if let Some(event_name) = &wu.event_name
            && let Ok(Some((event, score))) = state
                .event_repo
                .get_all_by_title_fuzzy_with_score(event_name, 0.6)
                .await
        {
            info!(
                event_name,
                matched = event.title,
                score,
                "Resolved writeup event via fuzzy match"
            );
            wu.event_id = event.ctftime_id;
        }

        // If event_name didn't help, try title directly
        if wu.event_id == 0
            && let Ok(Some((event, score))) = state
                .event_repo
                .get_all_by_title_fuzzy_with_score(&wu.title, 0.4)
                .await
        {
            info!(
                title = wu.title,
                matched = event.title,
                score,
                "Resolved writeup event via title fuzzy match"
            );
            wu.event_id = event.ctftime_id;
        }

        let inserted = state.writeup_repo.upsert_writeup(&wu).await?;
        if inserted {
            info!(
                ctftime_id = wu.ctftime_id,
                "New writeup saved (queued for enrichment)"
            );
        }
    }

    Ok(())
}

const WRITEUP_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const WRITEUP_TEXT_LIMIT: usize = 6000;

pub(crate) async fn fetch_writeup_text(client: &Client, url: &str) -> Option<String> {
    if url.trim().is_empty() {
        return None;
    }

    let resp = client
        .get(url)
        .timeout(WRITEUP_FETCH_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }

    if let Some(content_type) = resp.headers().get(reqwest::header::CONTENT_TYPE)
        && let Ok(ct) = content_type.to_str()
    {
        let ct = ct.to_lowercase();
        if !ct.contains("text/html") && !ct.contains("application/xhtml+xml") {
            return None;
        }
    }

    let html = resp.text().await.ok()?;
    let document = Html::parse_document(&html);
    let mut text = document
        .root_element()
        .text()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    text = text.replace("  ", " ");
    if text.chars().count() > WRITEUP_TEXT_LIMIT {
        text = text.chars().take(WRITEUP_TEXT_LIMIT).collect();
    }

    if text.is_empty() { None } else { Some(text) }
}
