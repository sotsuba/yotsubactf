use shared::{CtfResult, ReadCtfRepository};
use twilight_model::application::interaction::application_command::CommandDataOption;
use twilight_model::http::interaction::InteractionResponse;

pub async fn handle(
    _repo: &dyn ReadCtfRepository,
    _opts: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    todo!()
}

pub async fn handle_component(
    _repo: &dyn ReadCtfRepository,
    _message_id: &str,
    _custom_id: &str,
) -> CtfResult<InteractionResponse> {
    todo!()
}
