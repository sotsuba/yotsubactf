pub mod completed;
pub mod countdown;
pub mod current;
pub mod info;
pub mod upcoming;

use super::{CommandContext, SlashCommand};
use crate::embed::ephemeral_error;
use async_trait::async_trait;
use shared::{CtfError, CtfResult};
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::CommandOptionValue;
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

pub struct EventCommand;

#[async_trait]
impl SlashCommand for EventCommand {
    fn name(&self) -> &'static str {
        "event"
    }

    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new("event", "Browse CTF events", CommandType::ChatInput)
            .option(SubCommandBuilder::new("upcoming", "Upcoming CTFs").build())
            .option(SubCommandBuilder::new("current", "CTFs in progress now").build())
            .option(SubCommandBuilder::new("completed", "Recently ended CTFs").build())
            .option(SubCommandBuilder::new("countdown", "Countdown to a CTF").build())
            .option(SubCommandBuilder::new("info", "Details about a CTF").build())
            .build()
    }

    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        let subcmd = ctx
            .options
            .first()
            .ok_or_else(|| CtfError::InvalidInput("Missing subcommand".into()))?;
        let opts = match &subcmd.value {
            CommandOptionValue::SubCommand(o) => o.as_slice(),
            _ => &[],
        };
        match subcmd.name.as_str() {
            "upcoming" => upcoming::handle(ctx.state, opts).await,
            "current" => current::handle(ctx.state, opts).await,
            "completed" => completed::handle(ctx.state, ctx.guild_id, opts).await,
            "countdown" => countdown::handle(ctx.state.events.as_ref(), opts).await,
            "info" => info::handle(ctx.state.events.as_ref(), opts).await,
            _ => Ok(ephemeral_error("Unknown subcommand.")),
        }
    }
}
