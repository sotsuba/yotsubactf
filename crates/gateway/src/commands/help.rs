use async_trait::async_trait;
use shared::CtfResult;
use twilight_model::application::command::CommandType;
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::CommandBuilder;

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed};

pub struct HelpCommand;

#[async_trait]
impl SlashCommand for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new("help", "Show bot commands", CommandType::ChatInput).build()
    }
    async fn handle(&self, _ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        Ok(handle())
    }
}

pub fn handle() -> InteractionResponse {
    let embed = CtfEmbed::new("CTF Bot — Commands")
        .description(
            "**`/ping`** — Check bot responsiveness\n\
             **`/upcoming [count]`** — List the next scheduled CTFs (max 20)\n\
             **`/subscribe channel:#channel`** — Subscribe this server to CTF notifications\n\
             **`/unsubscribe`** — Stop receiving CTF notifications\n\
             **`/help`** — Show this message",
        )
        .now()
        .build();

    ephemeral_embed(embed)
}
