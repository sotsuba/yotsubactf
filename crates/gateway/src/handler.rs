use anyhow::Result;
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
use shared::{AdminRole, CtfError, CtfResult, ReadCtfRepository, ReminderRepository};

/// Log a user-facing error at debug level (expected, not a bug).
macro_rules! log_user_error {
    ($cmd:expr, $guild:expr, $user:expr, $msg:expr) => {
        tracing::debug!(
            command = $cmd,
            guild_id = ?$guild,
            user_id = $user,
            reason = $msg,
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
        let interaction_inner = interaction.0;
        if let Err(err) = handle_interaction(interaction_inner, http, application_id, state).await {
            warn!(?err, ?shard_id, "interaction handler returned error");
        }
    }
    Ok(())
}

use std::str::FromStr;

enum ComponentAction {
    Remind(String),
    ReminderList(String),
    Writeups,
    EventUpcoming(String),
    EventCurrent(String),
    EventCompleted(String),
}

impl FromStr for ComponentAction {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let (name, rest) = s.split_once(':').unwrap_or((s, ""));
        match name {
            "remind" => Ok(Self::Remind(rest.to_string())),
            "reminder_list" => Ok(Self::ReminderList(rest.to_string())),
            "writeups" => Ok(Self::Writeups),
            "event_upcoming" => Ok(Self::EventUpcoming(rest.to_string())),
            "event_current" => Ok(Self::EventCurrent(rest.to_string())),
            "event_completed" => Ok(Self::EventCompleted(rest.to_string())),
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
                let mut allowed_by_admin_role = true;

                if cmd.requires_guild() && interaction.guild_id.is_none() {
                    Err(CtfError::InvalidInput(
                        "This command must be used in a server.".into(),
                    ))
                } else if cmd.requires_manage_guild() && !member_can_manage_guild {
                    log_user_error!(
                        data.name,
                        interaction.guild_id.map(|id| id.get()),
                        user_id,
                        "missing MANAGE_GUILD permission"
                    );
                    Err(CtfError::PermissionDenied(
                        "You need the **Manage Server** permission to use this command.".into(),
                    ))
                } else {
                    if let (Some(required_role), Some(guild_id)) =
                        (cmd.required_admin_role(), guild_id_str.as_deref())
                    {
                        let member_roles = interaction
                            .member
                            .as_ref()
                            .map(|m| {
                                m.roles
                                    .iter()
                                    .map(|id| id.get().to_string())
                                    .collect::<std::collections::HashSet<_>>()
                            })
                            .unwrap_or_default();

                        let role_assignments = state.admin_roles.list_admin_roles(guild_id).await?;

                        // If no admin roles are configured, fall back to MANAGE_GUILD.
                        if !role_assignments.is_empty() {
                            allowed_by_admin_role = role_assignments.iter().any(|assignment| {
                                member_roles.contains(&assignment.role_id)
                                    && assignment.role.allows(required_role)
                            });
                        }
                    }

                    if !allowed_by_admin_role {
                        log_user_error!(
                            data.name,
                            interaction.guild_id.map(|id| id.get()),
                            user_id,
                            "missing admin role"
                        );
                        Err(CtfError::PermissionDenied(format!(
                            "You need an admin role of **{}** or higher to use this command.",
                            cmd.required_admin_role()
                                .unwrap_or(AdminRole::Admin)
                                .as_str()
                        )))
                    } else if interaction.kind
                        == twilight_model::application::interaction::InteractionType::ApplicationCommandAutocomplete
                    {
                        match cmd.autocomplete(ctx).await {
                            Ok(Some(res)) => Ok(res),
                            Ok(None) => return Ok(()),
                            Err(e) => Err(e),
                        }
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
                    ComponentAction::Writeups => {
                        commands::writeups::handle_component(state, data).await
                    }
                    ComponentAction::EventUpcoming(rest) => {
                        commands::event::upcoming::handle_component(state.events.as_ref(), &rest)
                            .await
                    }
                    ComponentAction::EventCurrent(rest) => {
                        commands::event::current::handle_component(state.events.as_ref(), &rest)
                            .await
                    }
                    ComponentAction::EventCompleted(rest) => {
                        commands::event::completed::handle_component(
                            state,
                            guild_id_str.as_deref(),
                            &rest,
                        )
                        .await
                    }
                },
                Err(_) => Err(CtfError::InvalidInput("Unknown message component".into())),
            }
        }

        _ => return Ok(()),
    };

    let business_success = response_result.is_ok();
    let latency_ms = start.elapsed().as_millis() as i64;

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

    // ── Send Response to Discord ────────────────────────────────────────────
    let discord_result = http
        .interaction(application_id)
        .create_response(interaction.id, &interaction.token, &response)
        .await;

    let delivery_success = discord_result.is_ok();
    if let Err(err) = &discord_result {
        tracing::error!(
            ?err,
            command = get_interaction_name(&interaction),
            guild_id = ?guild_id_str,
            user_id = user_id,
            "failed to send interaction response to Discord"
        );
    }

    // ── Post-execution metrics and logging ──────────────────────────────────
    let success = business_success && delivery_success;

    // Track metrics
    metrics::counter!(
        shared::metrics::GATEWAY_COMMANDS_TOTAL,
        "command" => get_interaction_name(&interaction),
        "kind"    => get_interaction_kind(&interaction),
        "success" => success.to_string()
    )
    .increment(1);

    metrics::histogram!(
        shared::metrics::GATEWAY_COMMAND_LATENCY,
        "command" => get_interaction_name(&interaction),
        "kind"    => get_interaction_kind(&interaction)
    )
    .record(start.elapsed().as_secs_f64());

    tracing::info!(
        command = get_interaction_name(&interaction),
        kind = get_interaction_kind(&interaction),
        success = success,
        guild_id = ?guild_id_str,
        user_id = user_id,
        latency_ms = latency_ms,
        "interaction handled"
    );

    // Persist to DB for historical analytics
    let _ = state
        .command_logs
        .log_command(
            &user_id.to_string(),
            guild_id_str.as_deref(),
            &get_interaction_name(&interaction),
            get_interaction_kind(&interaction),
            business_success,
            latency_ms,
        )
        .await;

    Ok(())
}

async fn handle_remind_component(
    events: &dyn ReadCtfRepository,
    reminders: &dyn ReminderRepository,
    user_id: &str,
    ctftime_id_str: &str,
) -> CtfResult<InteractionResponse> {
    let ctftime_id = ctftime_id_str
        .parse::<i64>()
        .map_err(|_| CtfError::InvalidInput("Invalid CTFtime ID".into()))?;

    let event = events
        .get_by_ctftime_id(ctftime_id)
        .await?
        .ok_or_else(|| CtfError::NotFound("Event not found".into()))?;

    // Create a one-shot reminder at the start time.
    let outcome = reminders
        .create(&shared::Reminder {
            user_id: user_id.to_string(),
            kind: shared::ReminderKind::Event,
            ctftime_id: Some(ctftime_id),
            event_title: Some(event.title.clone()),
            event_start_at: Some(event.start_time),
            remind_at: event.start_time,
            ..Default::default()
        })
        .await?;

    match outcome {
        shared::CreateReminderOutcome::Created => {
            let embed = CtfEmbed::success("Reminder set")
                .description(format!(
                    "I'll remind you when **{}** starts (<t:{}:R>).",
                    event.title,
                    event.start_time.timestamp()
                ))
                .now()
                .build();
            Ok(ephemeral_embed(embed))
        }
        shared::CreateReminderOutcome::AlreadyExists => Ok(ephemeral_error(
            "You already have a reminder for this event.",
        )),
        shared::CreateReminderOutcome::QuotaExceeded => Ok(ephemeral_error(
            "You have too many active reminders. Please delete some before adding more.",
        )),
    }
}

fn get_interaction_name(interaction: &Interaction) -> String {
    match interaction.data {
        Some(InteractionData::ApplicationCommand(ref data)) => data.name.clone(),
        Some(InteractionData::MessageComponent(ref data)) => data
            .custom_id
            .split(':')
            .next()
            .unwrap_or("component")
            .to_string(),
        _ => "unknown".to_string(),
    }
}

fn get_interaction_kind(interaction: &Interaction) -> &'static str {
    match interaction.data {
        Some(InteractionData::ApplicationCommand(_)) => "slash",
        Some(InteractionData::MessageComponent(_)) => "component",
        _ => "unknown",
    }
}
