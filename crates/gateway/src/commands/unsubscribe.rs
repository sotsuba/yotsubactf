use async_trait::async_trait;
use shared::{AdminRole, CtfError, CtfResult, GuildRepository};
use twilight_model::application::command::CommandType;
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::CommandBuilder;

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed};

pub struct UnsubscribeCommand;

#[async_trait]
impl SlashCommand for UnsubscribeCommand {
    fn name(&self) -> &'static str {
        "unsubscribe"
    }
    fn requires_guild(&self) -> bool {
        true
    }
    fn requires_manage_guild(&self) -> bool {
        true
    }
    fn required_admin_role(&self) -> Option<AdminRole> {
        Some(AdminRole::Admin)
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "unsubscribe",
            "Stop receiving CTF event notifications in this server",
            CommandType::ChatInput,
        )
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        let gid = ctx.guild_id.ok_or_else(|| {
            CtfError::InvalidInput("This command can only be used in a server.".to_string())
        })?;
        handle(ctx.state.guilds.as_ref(), gid).await
    }
}

pub async fn handle(
    guild_repo: &dyn GuildRepository,
    guild_id: &str,
) -> CtfResult<InteractionResponse> {
    // ── Soft-delete ───────────────────────────────────────────────────────────
    let removed = guild_repo.unsubscribe(guild_id).await?;

    let embed = if removed {
        CtfEmbed::success("✅ Unsubscribed")
            .description(
                "This server will no longer receive CTF event notifications.\n\
                 You can re-subscribe any time with `/subscribe`.",
            )
            .now()
            .build()
    } else {
        CtfEmbed::warning("⚠️ Not subscribed")
            .description(
                "This server has no active CTF subscription.\n\
                 Use `/subscribe channel:#channel` to start receiving notifications.",
            )
            .now()
            .build()
    };

    Ok(ephemeral_embed(embed))
}
