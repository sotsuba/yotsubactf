use async_trait::async_trait;
use shared::GuildRepository;
use shared::{CtfError, CtfResult};
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::channel::ChannelType;
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::{ChannelBuilder, CommandBuilder};

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed, ephemeral_error};

pub struct SubscribeCommand;

#[async_trait]
impl SlashCommand for SubscribeCommand {
    fn name(&self) -> &'static str {
        "subscribe"
    }
    fn requires_guild(&self) -> bool {
        true
    }
    fn requires_manage_guild(&self) -> bool {
        true
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "subscribe",
            "Subscribe this server to CTF event notifications",
            CommandType::ChatInput,
        )
        .option(
            ChannelBuilder::new("channel", "Channel to post CTF events in")
                .required(true)
                .channel_types([ChannelType::GuildText])
                .build(),
        )
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        let gid = ctx.guild_id.ok_or_else(|| {
            CtfError::InvalidInput("This command can only be used in a server.".to_string())
        })?;
        handle(ctx.state.guilds.as_ref(), gid, ctx.options).await
    }
}

pub async fn handle(
    repo: &dyn GuildRepository,
    guild_id: &str,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    // ── Parse the required `channel` option ───────────────────────────────────
    let channel_id = match extract_channel_id(options) {
        Some(id) => id as i64,
        None => {
            return Ok(ephemeral_error(
                "❌ Please specify a channel: `/subscribe channel:#general`",
            ));
        }
    };

    // ── Persist ───────────────────────────────────────────────────────────────
    // Convert numeric IDs to the string form the repository expects.
    repo.subscribe(guild_id, &channel_id.to_string()).await?;

    let embed = CtfEmbed::success("✅ Subscribed!")
        .description(format!(
            "New CTF events will be announced in <#{channel_id}>.\n\
             Use `/unsubscribe` at any time to stop notifications."
        ))
        .now()
        .build();

    Ok(ephemeral_embed(embed))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_channel_id(options: &[CommandDataOption]) -> Option<u64> {
    options.iter().find(|o| o.name == "channel").and_then(|o| {
        if let CommandOptionValue::Channel(id) = o.value {
            Some(id.get())
        } else {
            None
        }
    })
}
