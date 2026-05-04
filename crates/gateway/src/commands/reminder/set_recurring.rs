use crate::state::AppState;
use chrono::Utc;
use shared::{Reminder, ReminderKind};
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::CommandDataOption;
use uuid::Uuid;

use super::util::*;
use super::validation::{RecurringParams, validate_recurring};

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
    let now = Utc::now();
    let user_id = interaction
        .author_id()
        .ok_or_else(|| shared::CtfError::InvalidInput("Cannot identify user".into()))?
        .to_string();

    let params = RecurringParams {
        for_hours: opt_int(&opts, "for_hours").unwrap_or(1),
        every_minutes: opt_int(&opts, "every_minutes").unwrap_or(0),
        delay_minutes: opt_int(&opts, "delay_minutes").unwrap_or(0),
        message: opt_str(&opts, "message"),
    };

    let validated = match validate_recurring(params, now) {
        Ok(v) => v,
        Err(e) => return Ok(ephemeral_reply(e.to_string())),
    };

    let reminder = Reminder {
        id: Uuid::nil(),
        user_id,
        kind: ReminderKind::Recurring,
        ctftime_id: None,
        event_title: None,
        event_start_at: None,
        message: validated.message.clone(),
        remind_at: validated.first_remind_at,
        interval_secs: Some(validated.interval_secs),
        repeat_until: Some(validated.repeat_until),
        fire_count_max: Some(validated.fire_count),
        sent_count: 0,
        last_sent_at: None,
        created_at: now,
    };

    state.reminders.create(&reminder).await?;

    Ok(ephemeral_reply(format!(
        "🔁 **Recurring reminder set!**\n\
         Every {} · for {} hours · **{} fires total**\n\
         First: <t:{}:R> · Last: <t:{}:R>\n\
         {}",
        format_minutes(validated.interval_secs / 60),
        (validated.repeat_until.timestamp() - now.timestamp()) / 3600,
        validated.fire_count,
        validated.first_remind_at.timestamp(),
        validated.repeat_until.timestamp(),
        validated
            .message
            .as_deref()
            .map(|m| format!("\n> {m}"))
            .unwrap_or_default(),
    )))
}

fn format_minutes(m: i64) -> String {
    if m >= 60 {
        let h = m / 60;
        let mm = m % 60;
        if mm == 0 {
            format!("{h}h")
        } else {
            format!("{h}h {mm}m")
        }
    } else {
        format!("{m}m")
    }
}
