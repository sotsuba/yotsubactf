use super::{CommandContext, SlashCommand};
use async_trait::async_trait;
use shared::CtfResult;
use twilight_model::application::command::CommandType;
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_util::builder::command::CommandBuilder;

pub struct PingCommand;

#[async_trait]
impl SlashCommand for PingCommand {
    fn name(&self) -> &'static str {
        "ping"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new("ping", "Check bot responsiveness", CommandType::ChatInput).build()
    }
    async fn handle(&self, _ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        Ok(handle())
    }
}

pub fn handle() -> InteractionResponse {
    InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            content: Some("🏓 Pong!".to_string()),
            flags: Some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    }
}
