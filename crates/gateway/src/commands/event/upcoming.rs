use crate::state::AppState;
use shared::CtfResult;
use twilight_model::application::interaction::application_command::CommandDataOption;
use twilight_model::http::interaction::InteractionResponse;

pub async fn handle(
    _state: &AppState,
    _opts: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    todo!()
}

pub async fn handle_component(
    _state: &AppState,
    _message_id: &str,
    _custom_id: &str,
) -> CtfResult<InteractionResponse> {
    todo!()
}
