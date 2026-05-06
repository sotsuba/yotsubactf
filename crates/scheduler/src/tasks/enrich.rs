use async_trait::async_trait;
use tracing::{info, warn};

use super::SchedulerTask;
use crate::SharedState;
use shared::CtfResult;

pub struct EnrichTask;

#[async_trait]
impl SchedulerTask for EnrichTask {
    fn name(&self) -> &'static str {
        "enrich"
    }

    async fn run_once(&self, state: &SharedState) -> CtfResult<()> {
        // 1. Process Events
        let unenriched_events = state.event_repo.list_unenriched_events(10).await?;
        if !unenriched_events.is_empty() {
            info!(count = unenriched_events.len(), "Enriching events");
        }

        for ev in unenriched_events {
            let id = ev.id.expect("Event must have an ID from DB");

            if let Some(llm) = &state.llm {
                let raw_desc = ev.description.as_deref().unwrap_or("");
                if !raw_desc.trim().is_empty() {
                    match llm.clean_event_description(&ev.title, raw_desc).await {
                        Some(cleaned) => {
                            state.event_repo.mark_event_enriched(id, &cleaned).await?;
                            info!(title = %ev.title, "Event enriched successfully");
                            continue;
                        }
                        None => {
                            warn!(title = %ev.title, "Gemini failed to enrich event; skipping for now");
                            // We don't mark as enriched so it can be retried next cycle
                            continue;
                        }
                    }
                }
            }

            // If LLM is disabled or there is no description, mark as enriched as-is
            state
                .event_repo
                .mark_event_enriched(id, ev.description.as_deref().unwrap_or(""))
                .await?;
        }

        // 2. Process Writeups
        let unenriched_writeups = state.writeup_repo.list_unenriched_writeups(10).await?;
        if !unenriched_writeups.is_empty() {
            info!(count = unenriched_writeups.len(), "Enriching writeups");
        }

        for w in unenriched_writeups {
            let mut summary = None;
            let mut category = w.category.clone();

            if let Some(llm) = &state.llm {
                // 1. Summarize
                if let Some(text) =
                    crate::tasks::writeups::fetch_writeup_text(&state.http, &w.url).await
                {
                    summary = llm.summarize_writeup(&w.title, &text).await;
                }

                // 2. Classify Category if missing
                if category.is_none()
                    && let Some(raw) = llm.classify_category(&w.title).await
                {
                    category = Some(crate::ctftime::writeups::standardize_category(&raw));
                }
            }

            // Update DB and mark as enriched
            state
                .writeup_repo
                .mark_writeup_enriched(w.id, summary.as_deref().unwrap_or(""), category.as_deref())
                .await?;

            info!(title = %w.title, "Writeup enriched successfully");
        }

        Ok(())
    }
}
