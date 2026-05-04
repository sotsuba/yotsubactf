pub mod completed;
pub mod countdown;
pub mod current;
pub mod info;
pub mod upcoming;

use super::{CommandContext, SlashCommand};
use crate::embed::ephemeral_error;
use async_trait::async_trait;
use crate::state::AppState;
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
        use twilight_util::builder::command::{
            BooleanBuilder, IntegerBuilder, NumberBuilder, StringBuilder,
        };

        CommandBuilder::new("event", "Browse CTF events", CommandType::ChatInput)
            .option(
                SubCommandBuilder::new("upcoming", "Upcoming CTFs")
                    .option(
                        IntegerBuilder::new("count", "Number of events to show (max 25)")
                            .min_value(1)
                            .max_value(25)
                            .build(),
                    )
                    .option(
                        StringBuilder::new("format", "Filter by format")
                            .choices([
                                ("Jeopardy", "Jeopardy"),
                                ("Attack-Defense", "Attack-Defense"),
                                ("Mixed", "Mixed"),
                            ])
                            .build(),
                    )
                    .option(NumberBuilder::new("weight_min", "Min weight").min_value(0.0).build())
                    .option(NumberBuilder::new("weight_max", "Max weight").min_value(0.0).build())
                    .option(BooleanBuilder::new("onsite", "true=onsite, false=online").build())
                    .option(
                        StringBuilder::new("sort_by", "Sort order")
                            .choices([
                                ("Time (Nearest)", "time"),
                                ("Reputation (Weight)", "weight"),
                            ])
                            .build(),
                    )
                    .build(),
            )
            .option(
                SubCommandBuilder::new("current", "CTFs in progress now")
                    .option(
                        IntegerBuilder::new("count", "Number of events to show")
                            .min_value(1)
                            .max_value(25)
                            .build(),
                    )
                    .build(),
            )
            .option(
                SubCommandBuilder::new("completed", "Recently ended CTFs")
                    .option(
                        IntegerBuilder::new("count", "Number of events to show")
                            .min_value(1)
                            .max_value(25)
                            .build(),
                    )
                    .option(StringBuilder::new("format", "Filter by format").build())
                    .option(NumberBuilder::new("weight_min", "Min weight").min_value(0.0).build())
                    .build(),
            )
            .option(
                SubCommandBuilder::new("countdown", "Countdown to a CTF")
                    .option(StringBuilder::new("query", "CTF name").required(true).build())
                    .build(),
            )
            .option(
                SubCommandBuilder::new("info", "Details about a CTF")
                    .option(StringBuilder::new("query", "CTF name").required(true).build())
                    .build(),
            )
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

pub async fn handle_component(
    state: &AppState,
    guild_id: Option<&str>,
    custom_id: &str,
) -> CtfResult<InteractionResponse> {
    let parts: Vec<&str> = custom_id.splitn(3, ':').collect();
    match parts.as_slice() {
        ["event", "upcoming", _] => upcoming::handle_component(state, custom_id).await,
        ["event", "current", _] => current::handle_component(state, custom_id).await,
        ["event", "completed", _] => completed::handle_component(state, guild_id, custom_id).await,
        _ => Ok(ephemeral_error("Unsupported interaction.")),
    }
}
