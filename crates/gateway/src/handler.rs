use anyhow::Result;
use metrics;
use std::sync::Arc;
use tracing::warn;
use twilight_gateway::Event;
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::{Interaction, InteractionData};
use twilight_model::guild::Permissions;
use twilight_model::http::interaction::InteractionResponse;
use twilight_model::id::Id;
use twilight_model::id::marker::ApplicationMarker;

use crate::commands;
use crate::embed::{CtfEmbed, ephemeral_embed, ephemeral_error};
use crate::state::AppState;
use shared::{CtfError, CtfResult, ReadCtfRepository, ReminderRepository};

/// Log a user-facing error at debug level (expected, not a bug).
macro_rules! log_user_error {
    ($cmd:expr, $guild:expr, $user:expr, $msg:expr) => {
        tracing::debug!(
            command  = $cmd,
            guild_id = ?$guild,
            user_id  = $user,
            reason   = $msg,
            "command rejected (user error)"
        );
    };
}

pub async fn handle_event(
    shard_id: twilight_gateway::ShardId,
    event: Event,
    http: &HttpClient,
    application_id: Id<ApplicationMarker>,
    state: &Arc<AppState>,
) -> Result<()> {
    if let Event::InteractionCreate(interaction) = event {
        if let Err(err) = handle_interaction(interaction.0, http, application_id, state).await {
            warn!(?err, ?shard_id, "interaction handler returned error");
        }
    }
    Ok(())
}

use std::str::FromStr;

enum ComponentAction {
    Remind(String),
    ReminderList(String),
    Current(String),
    Writeups(String),
    Upcoming(String),
}

impl FromStr for ComponentAction {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let (name, rest) = s.split_once(':').unwrap_or((s, ""));
        match name {
            "remind" => Ok(Self::Remind(rest.to_string())),
            "reminder_list" => Ok(Self::ReminderList(rest.to_string())),
            "current" => Ok(Self::Current(rest.to_string())),
            "writeups" => Ok(Self::Writeups(rest.to_string())),
            "upcoming" => Ok(Self::Upcoming(rest.to_string())),
            _ => Err(()),
        }
    }
}

async fn handle_interaction(
    interaction: Interaction,
    http: &HttpClient,
    application_id: Id<ApplicationMarker>,
    state: &AppState,
) -> Result<()> {
    let start = std::time::Instant::now();

    // Extract user/member ID for logging — available on both guild and DM interactions.
    let user_id = interaction.author_id().map(|id| id.get()).unwrap_or(0);

    let guild_id_str = interaction.guild_id.map(|id| id.get().to_string());

    // ── Per-user Rate Limiting ──────────────────────────────────────────────
    if user_id != 0 && !state.check_rate_limit(user_id).await {
        metrics::counter!(shared::metrics::GATEWAY_RATE_LIMIT_TOTAL).increment(1);
        let response = ephemeral_embed(
            CtfEmbed::error("Slow Down")
                .description("You are sending commands too fast! Please wait a few seconds.")
                .now()
                .build(),
        );
        let _ = state
            .command_logs
            .log_command(
                &user_id.to_string(),
                guild_id_str.as_deref(),
                "rate_limited",
                "system",
                false,
                0,
            )
            .await;

        let _ = http
            .interaction(application_id)
            .create_response(interaction.id, &interaction.token, &response)
            .await;
        return Ok(());
    }

    // Helper: check if the invoking member has MANAGE_GUILD permission.
    // Used to guard admin-only commands (/subscribe, /unsubscribe).
    let member_can_manage_guild = interaction
        .member
        .as_ref()
        .and_then(|m| m.permissions)
        .map(|p| p.contains(Permissions::MANAGE_GUILD))
        .unwrap_or(false);

    let response_result: CtfResult<InteractionResponse> = match interaction.data {
        Some(InteractionData::ApplicationCommand(ref data)) => {
            let ctx = commands::CommandContext {
                state,
                guild_id: guild_id_str.as_deref(),
                user_id: interaction.author_id().unwrap_or(Id::new(1)),
                options: &data.options,
            };

            if data.name == "reminder" {
                // Special case for reminder command group to use custom dispatcher
                commands::reminder::handle_interaction(http, &interaction, state).await
            } else if let Some(cmd) = state.registry.get(data.name.as_str()) {
                if interaction.kind == twilight_model::application::interaction::InteractionType::ApplicationCommandAutocomplete {
                    match cmd.autocomplete(ctx).await {
                        Ok(Some(res)) => Ok(res),
                        Ok(None) => return Ok(()),
                        Err(e) => Err(e),
                    }
                } else {
                    if cmd.requires_guild() && interaction.guild_id.is_none() {
                        Err(CtfError::InvalidInput("This command must be used in a server.".into()))
                    } else if cmd.requires_manage_guild() && !member_can_manage_guild {
                        log_user_error!(data.name, interaction.guild_id.map(|id| id.get()), user_id, "missing MANAGE_GUILD permission");
                        Err(CtfError::PermissionDenied("You need the **Manage Server** permission to use this command.".into()))
                    } else {
                        cmd.handle(ctx).await
                    }
                }
            } else {
                log_user_error!(
                    "unknown",
                    interaction.guild_id.map(|id| id.get()),
                    user_id,
                    "unknown command"
                );
                Err(CtfError::InvalidInput(format!(
                    "Command `{}` is not recognized. Use `/help` to see available commands.",
                    data.name
                )))
            }
        }

        Some(InteractionData::MessageComponent(ref data)) => {
            match ComponentAction::from_str(&data.custom_id) {
                Ok(action) => match action {
                    ComponentAction::Remind(ctftime_id_str) => {
                        let user_id_str = interaction
                            .author_id()
                            .map(|id| id.get().to_string())
                            .unwrap_or_default();
                        handle_remind_component(
                            state.events.as_ref(),
                            state.reminders.as_ref(),
                            &user_id_str,
                            &ctftime_id_str,
                        )
                        .await
                    }
                    ComponentAction::ReminderList(cursor) => {
                        commands::reminder::handle_list_component(
                            http,
                            &interaction,
                            state,
                            &cursor,
                        )
                        .await
                    }
                    ComponentAction::Current(_) => {
                        commands::current::handle_component(state.events.as_ref(), data).await
                    }
                    ComponentAction::Writeups(_) => {
                        commands::writeups::handle_component(state, data).await
                    }
                    ComponentAction::Upcoming(_) => {
                        commands::upcoming::handle_component(state.events.as_ref(), data).await
                    }
                },
                Err(_) => Err(CtfError::InvalidInput("Unknown message component".into())),
            }
        }

        _ => return Ok(()),
    };

    let business_success = response_result.is_ok();
    let response = match response_result {
        Ok(res) => res,
        Err(e) => {
            if !matches!(
                e,
                CtfError::NotFound(_) | CtfError::PermissionDenied(_) | CtfError::InvalidInput(_)
            ) {
                tracing::error!(?e, "Command execution failed");
            }
            ephemeral_embed(CtfEmbed::from_shared(e.to_embed()).now().build())
        }
    };

    let (command_name, kind) = match &interaction.data {
        Some(InteractionData::ApplicationCommand(data)) => (data.name.as_str(), "slash"),
        Some(InteractionData::MessageComponent(data)) => (
            data.custom_id.splitn(2, ':').next().unwrap_or("component"),
            "component",
        ),
        _ => ("unknown", "unknown"),
    };

    // ── Send Response to Discord ────────────────────────────────────────────
    let discord_result = http
        .interaction(application_id)
        .create_response(interaction.id, &interaction.token, &response)
        .await;

    let delivery_success = discord_result.is_ok();
    if let Err(err) = &discord_result {
        tracing::error!(
            ?err,
            command  = command_name,
            guild_id = ?guild_id_str,
            user_id  = user_id,
            "failed to send interaction response to Discord"
        );
    }

    // ── Post-execution metrics and logging ──────────────────────────────────
    let success = business_success && delivery_success;
    let elapsed_ms = start.elapsed().as_millis();
    let elapsed_secs = start.elapsed().as_secs_f64();

    // Track metrics
    metrics::counter!(
        shared::metrics::GATEWAY_COMMANDS_TOTAL,
        "command" => command_name.to_string(),
        "kind"    => kind.to_string(),
        "success" => success.to_string()
    )
    .increment(1);

    metrics::histogram!(
        shared::metrics::GATEWAY_COMMAND_LATENCY,
        "command" => command_name.to_string(),
        "kind"    => kind.to_string()
    )
    .record(elapsed_secs);

    tracing::info!(
        command    = command_name,
        kind       = kind,
        success    = success,
        guild_id   = ?guild_id_str,
        user_id    = user_id,
        latency_ms = elapsed_ms,
        "interaction handled"
    );

    // Persist to DB for historical analytics
    let _ = state
        .command_logs
        .log_command(
            &user_id.to_string(),
            guild_id_str.as_deref(),
            command_name,
            kind,
            success,
            elapsed_ms as i64,
        )
        .await;

    Ok(())
}

// ── Remind Me handler ─────────────────────────────────────────────────────────

async fn handle_remind_component(
    event_repo: &dyn ReadCtfRepository,
    reminder_repo: &dyn ReminderRepository,
    user_id: &str,
    ctftime_id_str: &str,
) -> CtfResult<InteractionResponse> {
    let ctftime_id: i64 = match ctftime_id_str.parse() {
        Ok(v) => v,
        Err(_) => return Ok(ephemeral_error("Invalid event ID in button.")),
    };

    commands::reminder::common::create_event_reminder(
        chrono::Utc::now(),
        event_repo,
        reminder_repo,
        user_id,
        ctftime_id,
        3600, // 1 hour offset for button
    )
    .await
}
