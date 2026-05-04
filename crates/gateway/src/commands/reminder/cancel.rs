use crate::state::AppState;
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::CommandDataOption;

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
    let number = opt_int(&opts, "number").unwrap_or(0);
    let user_id = interaction
        .author_id()
        .ok_or_else(|| shared::CtfError::InvalidInput("Cannot identify user".into()))?
        .to_string();

    let reminders = state.reminders.list_pending(&user_id, None).await?;

    if number < 1 {
        return Ok(ephemeral_reply(
            "Please provide a valid reminder number (1 or higher).",
        ));
    }
    let index = (number as usize) - 1;
    let Some(reminder) = reminders.get(index) else {
        return Ok(ephemeral_reply(format!(
            "No reminder #{number}. You have {} active — use `/reminder list` to see them.",
            reminders.len()
        )));
    };

    let label = reminder.list_label();

    if state.reminders.cancel(reminder.id, &user_id).await? {
        Ok(ephemeral_reply(format!(
            "🗑️ **Reminder cancelled.**\n{label}"
        )))
    } else {
        Ok(ephemeral_reply(
            "Could not cancel — it may have already fired.",
        ))
    }
}
