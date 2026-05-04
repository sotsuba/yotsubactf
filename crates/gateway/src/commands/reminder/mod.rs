use super::{CommandContext, SlashCommand};
use crate::state::AppState;
use async_trait::async_trait;
use shared::CtfResult;
use twilight_http::Client as HttpClient;
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::CommandOptionValue;
use twilight_model::http::interaction::{InteractionResponse, InteractionResponseType};
use twilight_util::builder::command::{
    CommandBuilder, IntegerBuilder, StringBuilder, SubCommandBuilder, SubCommandGroupBuilder,
};

mod cancel;
pub(crate) mod common;
#[cfg(test)]
mod common_tests;
mod list;
mod set_event;
mod set_recurring;
mod set_timer;
pub mod util;
pub mod validation;

pub struct ReminderCommand;

#[async_trait]
impl SlashCommand for ReminderCommand {
    fn name(&self) -> &'static str {
        "reminder"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new("reminder", "Manage your reminders", CommandType::ChatInput)
            .option(
                SubCommandGroupBuilder::new("set", "Create a reminder").subcommands(vec![
                    SubCommandBuilder::new("event", "Remind me before a CTF event")
                        .option(IntegerBuilder::new("event_id", "CTFTime event ID").required(true))
                        .option(
                            IntegerBuilder::new("days", "Days before start")
                                .min_value(0)
                                .max_value(365),
                        )
                        .option(IntegerBuilder::new("hours", "Hours before start").min_value(0))
                        .option(
                            IntegerBuilder::new("minutes", "Minutes before start").min_value(0),
                        ),
                    SubCommandBuilder::new("timer", "Set a simple one-shot timer")
                        .option(
                            StringBuilder::new("message", "Message to remind you with")
                                .required(true),
                        )
                        .option(
                            IntegerBuilder::new("days", "Days from now")
                                .min_value(0)
                                .max_value(365),
                        )
                        .option(IntegerBuilder::new("hours", "Hours from now").min_value(0))
                        .option(IntegerBuilder::new("minutes", "Minutes from now").min_value(0)),
                    SubCommandBuilder::new("recurring", "Set a recurring reminder")
                        .option(
                            IntegerBuilder::new("for_hours", "Remind for how many hours from now")
                                .required(true),
                        )
                        .option(
                            IntegerBuilder::new("every_minutes", "Remind every X minutes")
                                .required(true),
                        )
                        .option(IntegerBuilder::new(
                            "delay_minutes",
                            "Wait X minutes before first fire (default: fires immediately)",
                        ))
                        .option(StringBuilder::new("message", "Message to remind you with")),
                ]),
            )
            .option(SubCommandBuilder::new("list", "List your active reminders"))
            .option(
                SubCommandBuilder::new("cancel", "Cancel a pending reminder").option(
                    IntegerBuilder::new("number", "The number from /reminder list").required(true),
                ),
            )
            .build()
    }

    async fn handle(&self, _ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        Err(shared::CtfError::Internal(
            "Reminder command should be handled by dispatcher".to_string(),
        ))
    }
}

pub async fn handle_interaction(
    http: &HttpClient,
    interaction: &Interaction,
    state: &AppState,
) -> CtfResult<InteractionResponse> {
    let Some(twilight_model::application::interaction::InteractionData::ApplicationCommand(
        ref data,
    )) = interaction.data
    else {
        return Err(shared::CtfError::InvalidInput(
            "Invalid interaction data".into(),
        ));
    };
    let Some(option) = data.options.first() else {
        return Err(shared::CtfError::InvalidInput(
            "Missing command options".into(),
        ));
    };

    match option.name.as_str() {
        "set" => {
            let CommandOptionValue::SubCommandGroup(ref sub_options) = option.value else {
                return Err(shared::CtfError::InvalidInput(
                    "Missing set sub-command group".into(),
                ));
            };
            let Some(sub_option) = sub_options.first() else {
                return Err(shared::CtfError::InvalidInput(
                    "Missing set sub-command".into(),
                ));
            };
            match sub_option.name.as_str() {
                "event" => set_event::handle(http, interaction, state, sub_option).await,
                "timer" => set_timer::handle(http, interaction, state, sub_option).await,
                "recurring" => set_recurring::handle(http, interaction, state, sub_option).await,
                _ => Err(shared::CtfError::InvalidInput(
                    "Unknown set sub-command".into(),
                )),
            }
        }
        "list" => {
            list::handle(
                http,
                interaction,
                state,
                None,
                InteractionResponseType::ChannelMessageWithSource,
            )
            .await
        }
        "cancel" => cancel::handle(http, interaction, state, option).await,
        _ => Err(shared::CtfError::InvalidInput(
            "Unknown reminder sub-command".into(),
        )),
    }
}

pub async fn handle_list_component(
    http: &HttpClient,
    interaction: &Interaction,
    state: &AppState,
    cursor: &str,
) -> CtfResult<InteractionResponse> {
    list::handle_component(http, interaction, state, cursor).await
}
