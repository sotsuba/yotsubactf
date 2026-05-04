use crate::state::AppState;
use chrono::{Duration, Utc};
use shared::{Reminder, ReminderKind};
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::CommandDataOption;
use uuid::Uuid;

use super::util::*;

use crate::embed::ephemeral_reply;
use shared::CtfResult;
use twilight_model::http::interaction::InteractionResponse;

pub async fn handle(
    _http: &HttpClient,
    interaction: &Interaction,
    state: &AppState,
    option: &CommandDataOption,
) -> CtfResult<InteractionResponse> {
    let sub_opts = match &option.value {
        twilight_model::application::interaction::application_command::CommandOptionValue::SubCommand(o) => o,
        _ => return Err(shared::CtfError::InvalidInput("Missing subcommand options".to_string())),
    };
    let opts = parse_options(sub_opts);

    let message = opt_str(&opts, "message").unwrap_or_default();
    if message.chars().count() > 200 {
        return Ok(ephemeral_reply("Message must be 200 characters or less."));
    }
    let days = opt_int(&opts, "days").unwrap_or(0);
    let hours = opt_int(&opts, "hours").unwrap_or(0);
    let minutes = opt_int(&opts, "minutes").unwrap_or(0);

    let offset_secs = days * 86400 + hours * 3600 + minutes * 60;
    if offset_secs == 0 {
        return Ok(ephemeral_reply(
            "Specify at least one of: days, hours, minutes.",
        ));
    }

    let now = Utc::now();
    let remind_at = now + Duration::seconds(offset_secs);

    if remind_at > now + Duration::days(super::validation::MAX_REMINDER_DAYS) {
        return Ok(ephemeral_reply(format!(
            "Reminder time is too far in the future (max {} days).",
            super::validation::MAX_REMINDER_DAYS
        )));
    }

    let reminder = Reminder {
        id: Uuid::nil(),
        user_id: interaction
            .author_id()
            .ok_or_else(|| shared::CtfError::InvalidInput("Cannot identify user".into()))?
            .to_string(),
        kind: ReminderKind::Timer,
        ctftime_id: None,
        event_title: None,
        event_start_at: None,
        message: Some(message.clone()),
        remind_at,
        interval_secs: None,
        repeat_until: None,
        fire_count_max: None,
        sent_count: 0,
        last_sent_at: None,
        created_at: now,
    };

    state.reminders.create(&reminder).await?;

    Ok(ephemeral_reply(format!(
        "✅ **Timer set!**\n\
         You'll be notified <t:{}:R>\n\
         > {}",
        remind_at.timestamp(),
        message
    )))
}
