use crate::SharedState;
use async_trait::async_trait;
use shared::CtfResult;
use tracing::{error, info};

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

    for mut wu in recent {
        // Try to resolve event_id from the database by title matching
        if let Some(event_name) = &wu.event_name {
            if let Ok(Some((event, score))) = state
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
        }

        // If event_name didn't help, try title directly
        if wu.event_id == 0 {
            if let Ok(Some((event, score))) = state
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
        }

        if state.writeup_repo.upsert_writeup(&wu).await? {
            info!(ctftime_id = wu.ctftime_id, "New writeup saved");
        }
    }

    // 2. Notify guilds about unnotified writeups
    let unnotified = state.writeup_repo.list_unnotified_writeups().await?;
    if unnotified.is_empty() {
        return Ok(());
    }

    info!(count = unnotified.len(), "Processing unnotified writeups");

    for wu in unnotified {
        // Find guilds that should receive this writeup.
        // Targeted: guilds tracking a team that participated in this event.
        let mut target_guilds = if wu.event_id > 0 {
            state
                .guild_repo
                .list_guilds_tracking_event(wu.event_id)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Fallback: if no targeted guilds (or event unknown), send to all active subscriptions that opted in.
        if target_guilds.is_empty() {
            target_guilds = state
                .guild_repo
                .list_writeup_opt_in_guilds()
                .await
                .unwrap_or_default();
        }

        let channel_ids: Vec<String> = target_guilds.into_iter().map(|s| s.channel_id).collect();

        if channel_ids.is_empty() {
            info!(
                ctftime_id = wu.ctftime_id,
                "No recipients found. Marking as notified."
            );
            state.writeup_repo.mark_writeup_notified(wu.id).await?;
            continue;
        }

        if let Err(err) = state.notifier.send_writeup(&wu, &channel_ids).await {
            error!(
                ?err,
                ctftime_id = wu.ctftime_id,
                "Failed to send writeup notification"
            );
        } else {
            state.writeup_repo.mark_writeup_notified(wu.id).await?;
            info!(
                ctftime_id = wu.ctftime_id,
                "Writeup notification sent to {} channels",
                channel_ids.len()
            );
        }
    }

    Ok(())
}
