use crate::SharedState;
use async_trait::async_trait;
use shared::CtfResult;
use tracing::{error, info};

use super::SchedulerTask;
use crate::ctftime::results::fetch_event_results;
use chrono::{Duration as ChronoDuration, Utc};

pub struct ResultsTask;

#[async_trait]
impl SchedulerTask for ResultsTask {
    fn name(&self) -> &'static str {
        "results"
    }
    async fn run_once(&self, state: &SharedState) -> CtfResult<()> {
        run_once(state).await
    }
}

pub async fn run_once(state: &SharedState) -> CtfResult<()> {
    let now = Utc::now();
    let seven_days_ago = now - ChronoDuration::days(7);

    // 1. Get all unique tracked team IDs
    let tracked_team_ids = state.team_repo.list_all_tracked_team_ids().await?;
    if tracked_team_ids.is_empty() {
        info!("No teams are being tracked. Skipping.");
        return Ok(());
    }

    // 2. Get events that ended in the last 7 days
    let recent_events = state
        .event_repo
        .list_recently_ended(seven_days_ago, now)
        .await?;
    info!(
        count = recent_events.len(),
        "Checking results for recently ended events"
    );

    for event in recent_events {
        let results = match fetch_event_results(&state.http, event.ctftime_id).await {
            Ok(r) => r,
            Err(err) => {
                error!(
                    ?err,
                    event_id = event.ctftime_id,
                    "Failed to fetch results for event"
                );
                continue;
            }
        };

        for result in results {
            if tracked_team_ids.contains(&result.ctftime_team_id) {
                // This result is for a team we track.
                if state.team_repo.upsert_result(&result).await? {
                    info!(
                        team_id = result.ctftime_team_id,
                        event_id = event.ctftime_id,
                        "New result found"
                    );
                }
            }
        }
    }

    // 3. Process unnotified results
    let unnotified = state.team_repo.list_unnotified_results().await?;
    info!(count = unnotified.len(), "Processing unnotified results");

    for (result, guild_ids) in unnotified {
        let event = state
            .event_repo
            .get_by_ctftime_id(result.ctf_event_id)
            .await?;
        let event_title = event
            .map(|e| e.title)
            .unwrap_or_else(|| "Unknown Event".to_string());

        let team_name = if let Some(gid) = guild_ids.first() {
            state
                .team_repo
                .get_followed_team(gid)
                .await?
                .map(|t| t.team_name)
                .unwrap_or_else(|| "Unknown Team".to_string())
        } else {
            "Unknown Team".to_string()
        };

        let mut channel_ids = Vec::new();
        for gid in guild_ids {
            if let Ok(Some(sub)) = state.guild_repo.get_active_subscription(&gid).await {
                channel_ids.push(sub.channel_id);
            }
        }

        if !channel_ids.is_empty() {
            if let Err(err) = state
                .notifier
                .send_result(&result, &event_title, &team_name, &channel_ids)
                .await
            {
                error!(
                    ?err,
                    team_id = result.ctftime_team_id,
                    "Failed to send result notification"
                );
            } else {
                state.team_repo.mark_result_notified(result.id).await?;
                info!(
                    team_id = result.ctftime_team_id,
                    event_id = result.ctf_event_id,
                    "Result notification sent"
                );
            }
        } else {
            // No channels to notify (likely guild unfollowed), but we must mark as notified
            // to avoid processing this result again.
            state.team_repo.mark_result_notified(result.id).await?;
            info!(
                team_id = result.ctftime_team_id,
                event_id = result.ctf_event_id,
                "Result marked as notified (no active subscriptions)"
            );
        }
    }

    Ok(())
}
