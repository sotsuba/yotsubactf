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

    let event_id = opt_int_or_zero(&opts, "event_id");
    let days = opt_int(&opts, "days").unwrap_or(0);
    let hours = opt_int(&opts, "hours").unwrap_or(0);
    let minutes = opt_int(&opts, "minutes").unwrap_or(0);

    let offset_secs = days * 86400 + hours * 3600 + minutes * 60;
    if offset_secs == 0 {
        return Ok(ephemeral_reply(
            "Specify at least one of: days, hours, minutes.",
        ));
    }

    super::common::create_event_reminder(
        chrono::Utc::now(),
        state.events.as_ref(),
        state.reminders.as_ref(),
        &interaction
            .author_id()
            .ok_or_else(|| shared::CtfError::InvalidInput("Cannot identify user".into()))?
            .to_string(),
        event_id,
        offset_secs,
    )
    .await
}
