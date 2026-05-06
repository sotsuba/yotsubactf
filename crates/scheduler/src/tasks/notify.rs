use async_trait::async_trait;
use tracing::{error, info};

use super::SchedulerTask;
use crate::SharedState;
use shared::CtfResult;

pub struct NotifyTask;

#[async_trait]
impl SchedulerTask for NotifyTask {
    fn name(&self) -> &'static str {
        "notify"
    }

    async fn run_once(&self, state: &SharedState) -> CtfResult<()> {
        // 1. Notify Events
        let unnotified_events = state.event_repo.list_unnotified_events().await?;
        if !unnotified_events.is_empty() {
            info!(
                count = unnotified_events.len(),
                "Processing event notifications"
            );
        }

        let subscriptions = state.guild_repo.list_active_subscriptions().await?;
        let channel_ids: Vec<String> = subscriptions.iter().map(|s| s.channel_id.clone()).collect();

        for ev in unnotified_events {
            let id = ev.id.expect("Event must have an ID from DB");

            if channel_ids.is_empty() {
                state.event_repo.mark_event_notified(id).await?;
                continue;
            }

            if let Err(err) = state.notifier.send(&ev, &channel_ids).await {
                error!(?err, title = %ev.title, "Failed to send event notification");
            } else {
                state.event_repo.mark_event_notified(id).await?;
                info!(title = %ev.title, "Event notification sent to {} channels", channel_ids.len());
            }
        }

        // 2. Notify Writeups
        let unnotified_writeups = state.writeup_repo.list_unnotified_writeups().await?;
        if !unnotified_writeups.is_empty() {
            info!(
                count = unnotified_writeups.len(),
                "Processing writeup notifications"
            );
        }

        for wu in unnotified_writeups {
            // Find guilds that should receive this writeup.
            let mut target_guilds = if wu.event_id > 0 {
                state
                    .guild_repo
                    .list_guilds_tracking_event(wu.event_id)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            if target_guilds.is_empty() {
                target_guilds = state
                    .guild_repo
                    .list_writeup_opt_in_guilds()
                    .await
                    .unwrap_or_default();
            }

            let wu_channel_ids: Vec<String> =
                target_guilds.into_iter().map(|s| s.channel_id).collect();

            if wu_channel_ids.is_empty() {
                state.writeup_repo.mark_writeup_notified(wu.id).await?;
                continue;
            }

            if let Err(err) = state.notifier.send_writeup(&wu, &wu_channel_ids).await {
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
                    wu_channel_ids.len()
                );
            }
        }

        Ok(())
    }
}
