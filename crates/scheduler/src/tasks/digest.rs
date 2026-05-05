use super::SchedulerTask;
use crate::SharedState;
use async_trait::async_trait;
use chrono::{Datelike, Utc};
use shared::CtfResult;
use shared::{ReadCtfRepository, UpcomingFilter};
use tracing::{error, info};

pub struct DigestTask;

#[async_trait]
impl SchedulerTask for DigestTask {
    fn name(&self) -> &'static str {
        "digest"
    }
    async fn run_once(&self, state: &SharedState) -> CtfResult<()> {
        run_once(state).await
    }
}

pub async fn run_once(state: &SharedState) -> CtfResult<()> {
    let now = Utc::now();
    let dow = now.weekday().num_days_from_sunday() as i16;

    let targets = state.guild_repo.list_digest_guilds_for_day(dow).await?;
    if targets.is_empty() {
        return Ok(());
    }

    info!(
        count = targets.len(),
        day_of_week = dow,
        "Sending digest to guilds"
    );

    let mut sent = 0;
    for (i, target) in targets.iter().enumerate() {
        // Backoff slightly between guilds to avoid bursting Discord's global rate limit
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        match send_digest(
            &*state.event_repo,
            &*state.writeup_repo,
            &*state.notifier,
            &target.channel_id,
            now,
        )
        .await
        {
            Ok(_) => {
                sent += 1;
                if let Err(err) = state.guild_repo.mark_digest_sent(&target.guild_id).await {
                    error!(?err, guild_id = %target.guild_id, "Failed to mark digest as sent");
                }
            }
            Err(err) => {
                error!(?err, guild_id = %target.guild_id, "Failed to send digest — will retry next run");
            }
        }
    }

    info!(sent, total = targets.len(), "Finished sending digests");
    Ok(())
}

async fn send_digest(
    event_repo: &(impl ReadCtfRepository + ?Sized),
    writeup_repo: &(impl shared::WriteupRepository + ?Sized),
    notifier: &(impl shared::contracts::Notifier + ?Sized),
    channel_id: &str,
    now: chrono::DateTime<Utc>,
) -> CtfResult<()> {
    let filter = UpcomingFilter::default();

    // Fetch upcoming and current events for the digest.
    let upcoming = event_repo.list_upcoming(10, 0, &filter).await?;
    let current = event_repo.list_current(10, 0).await?;

    // Fetch top writeups from last 7 days.
    let top_writeups = writeup_repo
        .list_top_writeups_since(now - chrono::Duration::days(7), 5)
        .await?;

    // Build the week number label.
    let week_num = now.format("%W/%Y");
    let mut description = String::new();

    if !current.events.is_empty() {
        description.push_str("⚡  **Currently running**\n");
        for event in &current.events {
            let ends_in = event.end_time.signed_duration_since(now);
            let days_left = ends_in.num_days();
            description.push_str(&format!(
                "• **{}**  |  ends in {} day(s)  |  <t:{}:D>\n",
                event.title,
                days_left,
                event.end_time.timestamp(),
            ));
        }
        description.push('\n');
    }

    if !upcoming.events.is_empty() {
        description.push_str(&format!(
            "🔜  **Upcoming ({} events)**\n",
            upcoming.total_count
        ));
        for event in upcoming.events.iter().take(5) {
            let fmt = event.format.as_deref().unwrap_or("?");
            let weight = event
                .weight
                .map(|w| format!(" | ⚖️ {w:.1}"))
                .unwrap_or_default();
            description.push_str(&format!(
                "• **{}**  |  {}{}  |  <t:{}:d>\n",
                event.title,
                fmt,
                weight,
                event.start_time.timestamp(),
            ));
        }
        description.push('\n');
    }

    if !top_writeups.is_empty() {
        description.push_str("📝  **Recent Top Writeups**\n");
        for wu in top_writeups {
            let cat = wu
                .category
                .as_deref()
                .map(|c| format!("`{}`", c))
                .unwrap_or_else(|| "Misc".into());
            description.push_str(&format!("• **[{}]({})**  |  {}\n", wu.title, wu.url, cat));
        }
    }

    if description.is_empty() {
        description.push_str("No CTF events or writeups this week. Check back later!");
    }

    let embed = serde_json::json!({
        "title": format!("🗓️  Weekly CTF Digest — Week {week_num}"),
        "description": description,
        "color": 0x2f6fed,
        "footer": { "text": "CTF Bot" },
        "timestamp": now.to_rfc3339(),
    });

    notifier.send_digest(channel_id, embed).await
}
