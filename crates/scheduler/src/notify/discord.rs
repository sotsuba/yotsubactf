//! Discord implementation of [`Notifier`] and the DM reminder sender.

use metrics;
use serde_json::{Value, json};
use shared::{CtfError, CtfResult as Result};
use tracing::{error, info, warn};

use async_trait::async_trait;
use shared::{CtfEvent, Notifier, Reminder};

use super::event_embed::build_event_notification;
use super::reminder_embed::build_reminder_dm;
use super::result_embed::build_result_notification;
use super::writeup_embed::build_writeup_notification;

// ── Notifier struct ───────────────────────────────────────────────────────────

pub struct DiscordNotifier {
    /// Shared HTTP client — same connection pool as the rest of the bot.
    client: reqwest_middleware::ClientWithMiddleware,
    /// Discord API base URL (e.g. "https://discord.com/api/v10").
    api_base: String,
    /// Pre-formatted `Authorization` header value (`"Bot <token>"`).
    /// Cached to avoid a heap allocation on every API call.
    auth_header: String,
}

impl DiscordNotifier {
    /// Construct a notifier.
    ///
    /// `client` should be the same `reqwest::Client` used by the scraper so
    /// the two subsystems share a connection pool.
    pub fn new(
        client: reqwest_middleware::ClientWithMiddleware,
        token: impl Into<String>,
        api_base: impl Into<String>,
    ) -> Self {
        let auth_header = format!("Bot {}", token.into());
        Self {
            client,
            auth_header,
            api_base: api_base.into(),
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    async fn post_json(&self, url: &str, body: Value) -> Result<()> {
        let resp = self
            .client
            .post(url)
            .header("Authorization", &self.auth_header)
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                let err_msg = format!("{}", err);
                if err_msg.contains("timeout") {
                    CtfError::Timeout
                } else {
                    CtfError::ExternalApi {
                        status: 0,
                        message: format!("HTTP request to Discord failed: {err}"),
                    }
                }
            })?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            metrics::counter!(shared::metrics::GATEWAY_RATE_LIMIT_TOTAL).increment(1);
            return Err(CtfError::PermissionDenied(
                "Discord rate-limited (429)".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(CtfError::ExternalApi {
                status: status.as_u16(),
                message: format!("Discord API error: {status}"),
            });
        }

        Ok(())
    }

    async fn post_to_channel(&self, channel_id: &str, body: Value, category: &str) -> Result<()> {
        let url = format!("{}/channels/{channel_id}/messages", self.api_base);

        let start = std::time::Instant::now();
        let res = self.post_json(&url, body).await;
        let latency = start.elapsed().as_secs_f64();

        let status = if res.is_ok() { "ok" } else { "err" };
        metrics::counter!(
            shared::metrics::DISCORD_DELIVERY_TOTAL,
            "status" => status,
            "type" => category.to_string()
        )
        .increment(1);

        metrics::histogram!(
            shared::metrics::DISCORD_DELIVERY_LATENCY,
            "type" => category.to_string()
        )
        .record(latency);

        res?;
        info!(channel_id, category, "Notification sent");
        Ok(())
    }

    /// Open (or reuse) a DM channel with a user and return its channel ID.
    async fn open_dm_channel(&self, user_id: &str) -> Result<String> {
        let url = format!("{}/users/@me/channels", self.api_base);
        let body = json!({ "recipient_id": user_id });
        let resp = self
            .client
            .post(&url)
            .header("Authorization", &self.auth_header)
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                let err_msg = format!("{}", err);
                if err_msg.contains("timeout") {
                    CtfError::Timeout
                } else {
                    CtfError::ExternalApi {
                        status: 0,
                        message: format!("HTTP request to Discord failed: {err}"),
                    }
                }
            })?;

        let status = resp.status();
        let data: Value = resp.json().await.map_err(|e| CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("Failed to parse DM channel response: {e}"),
        })?;
        data["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| CtfError::Internal("DM channel response missing 'id' field".to_string()))
    }
}

// ── Notifier impl ─────────────────────────────────────────────────────────────

#[async_trait]
impl Notifier for DiscordNotifier {
    async fn send(&self, event: &CtfEvent, channel_ids: &[String]) -> Result<()> {
        if channel_ids.is_empty() {
            warn!(title = %event.title, "No channel IDs supplied — nothing to notify");
            return Ok(());
        }

        let body = build_event_notification(event);
        let mut failed = 0usize;
        let mut last_err = None;

        for channel_id in channel_ids {
            if let Err(err) = self
                .post_to_channel(channel_id, body.clone(), "event")
                .await
            {
                failed += 1;
                error!(
                    channel_id = %channel_id,
                    title      = %event.title,
                    ?err,
                    "Failed to notify channel"
                );
                last_err = Some(err);
            }
        }

        if failed == channel_ids.len() {
            return Err(last_err.unwrap_or_else(|| {
                CtfError::Internal("Discord event notification failed for all channels".to_string())
            }));
        }

        if failed > 0 {
            warn!(
                title = %event.title,
                failed,
                total = channel_ids.len(),
                "Partial Discord delivery"
            );
        }

        Ok(())
    }

    async fn send_result(
        &self,
        result: &shared::TeamResult,
        event_title: &str,
        team_name: &str,
        channel_ids: &[String],
    ) -> Result<()> {
        if channel_ids.is_empty() {
            return Ok(());
        }

        let body = build_result_notification(result, event_title, team_name);
        let mut failed = 0usize;
        let mut last_err = None;

        for channel_id in channel_ids {
            if let Err(err) = self
                .post_to_channel(channel_id, body.clone(), "result")
                .await
            {
                failed += 1;
                error!(?err, %channel_id, %team_name, "Failed to send result notification");
                last_err = Some(err);
            }
        }

        if failed == channel_ids.len() {
            return Err(last_err.unwrap_or_else(|| {
                CtfError::Internal(
                    "Discord result notification failed for all channels".to_string(),
                )
            }));
        }

        if failed > 0 {
            warn!(
                team_name = %team_name,
                failed,
                total = channel_ids.len(),
                "Partial Discord delivery"
            );
        }

        Ok(())
    }

    async fn send_writeup(&self, writeup: &shared::Writeup, channel_ids: &[String]) -> Result<()> {
        if channel_ids.is_empty() {
            return Ok(());
        }

        let body = build_writeup_notification(writeup);
        let mut failed = 0usize;
        let mut last_err = None;

        for channel_id in channel_ids {
            if let Err(err) = self
                .post_to_channel(channel_id, body.clone(), "writeup")
                .await
            {
                failed += 1;
                error!(?err, %channel_id, writeup_id = writeup.ctftime_id, "Failed to send writeup notification");
                last_err = Some(err);
            }
        }

        if failed == channel_ids.len() {
            return Err(last_err.unwrap_or_else(|| {
                CtfError::Internal(
                    "Discord writeup notification failed for all channels".to_string(),
                )
            }));
        }

        if failed > 0 {
            warn!(
                writeup_id = writeup.ctftime_id,
                failed,
                total = channel_ids.len(),
                "Partial Discord delivery"
            );
        }

        Ok(())
    }

    async fn send_due_reminders(&self, due: &[Reminder]) -> Result<()> {
        let mut last_err = None;
        for reminder in due {
            if let Err(err) = self.send_reminder_dm(reminder).await {
                error!(
                    user_id    = %reminder.user_id,
                    kind       = ?reminder.kind,
                    ?err,
                    "Failed to send reminder DM"
                );
                last_err = Some(err);
            }
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn send_reminder_dm(&self, reminder: &Reminder) -> Result<()> {
        let channel_id = self.open_dm_channel(&reminder.user_id).await?;
        let body = build_reminder_dm(reminder);
        self.post_to_channel(&channel_id, body, "reminder").await
    }

    async fn send_digest(&self, channel_id: &str, embed: serde_json::Value) -> Result<()> {
        let body = serde_json::json!({ "embeds": [embed] });
        self.post_to_channel(channel_id, body, "digest").await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
