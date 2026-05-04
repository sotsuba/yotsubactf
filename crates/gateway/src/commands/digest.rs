use async_trait::async_trait;
use shared::{CtfError, CtfResult};
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::channel::ChannelType;
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::{
    ChannelBuilder, CommandBuilder, StringBuilder, SubCommandBuilder,
};

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed, ephemeral_error};
use shared::GuildRepository;

pub struct DigestCommand;

#[async_trait]
impl SlashCommand for DigestCommand {
    fn name(&self) -> &'static str {
        "digest"
    }
    fn requires_guild(&self) -> bool {
        true
    }
    fn requires_manage_guild(&self) -> bool {
        true
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "digest",
            "Manage weekly CTF digest settings",
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new("enable", "Enable weekly digest")
                .option(
                    StringBuilder::new("day", "Day of the week to send the digest")
                        .choices([
                            ("Sunday", "0"),
                            ("Monday", "1"),
                            ("Tuesday", "2"),
                            ("Wednesday", "3"),
                            ("Thursday", "4"),
                            ("Friday", "5"),
                            ("Saturday", "6"),
                        ])
                        .required(true)
                        .build(),
                )
                .option(
                    ChannelBuilder::new("channel", "Channel to post the digest in")
                        .channel_types([ChannelType::GuildText])
                        .required(true)
                        .build(),
                )
                .build(),
        )
        .option(SubCommandBuilder::new("disable", "Disable weekly digest").build())
        .option(SubCommandBuilder::new("status", "Check current digest status").build())
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        let gid = ctx.guild_id.ok_or_else(|| {
            CtfError::InvalidInput("This command can only be used in a server.".to_string())
        })?;
        handle(ctx.state.guilds.as_ref(), gid, ctx.options).await
    }
}

fn to_day_name(day: i16) -> &'static str {
    match day {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        _ => "Unknown",
    }
}

pub async fn handle(
    repo: &dyn GuildRepository,
    guild_id: &str,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    if options.is_empty() {
        return Ok(ephemeral_error("Invalid subcommand"));
    }

    let subcmd = &options[0];
    match subcmd.name.as_str() {
        "enable" => {
            let mut day_utc = 1; // Default to Monday
            let mut channel_id = None;

            if let CommandOptionValue::SubCommand(ref opts) = subcmd.value {
                for opt in opts {
                    match opt.name.as_str() {
                        "day" => {
                            if let CommandOptionValue::String(ref val) = opt.value {
                                day_utc = val.parse::<i16>().map_err(|_| {
                                    CtfError::InvalidInput("Invalid day of the week.".to_string())
                                })?;
                                if !(0..=6).contains(&day_utc) {
                                    return Err(CtfError::InvalidInput(
                                        "Day must be between 0 (Sunday) and 6 (Saturday)."
                                            .to_string(),
                                    ));
                                }
                            }
                        }
                        "channel" => {
                            if let CommandOptionValue::Channel(id) = opt.value {
                                channel_id = Some(id.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }

            let cid = channel_id.ok_or_else(|| {
                CtfError::InvalidInput("A channel must be specified.".to_string())
            })?;

            // Upsert guild in case it's new
            repo.upsert_guild(guild_id).await?;

            repo.set_digest(guild_id, true, Some(&cid), day_utc).await?;

            let day_name = to_day_name(day_utc);
            let embed = CtfEmbed::success("Digest Enabled")
                .description(format!(
                    "Weekly digest will be sent on **{}s** to <#{}>.",
                    day_name, cid
                ))
                .build();

            Ok(ephemeral_embed(embed))
        }
        "disable" => {
            repo.set_digest(guild_id, false, None, 1).await?;
            let embed = CtfEmbed::success("Digest Disabled")
                .description("The weekly digest has been turned off for this server.")
                .build();
            Ok(ephemeral_embed(embed))
        }
        "status" => match repo.get_digest(guild_id).await? {
            Some(config) if config.enabled => {
                let day_name = to_day_name(config.day_utc);
                let channel = config
                    .channel_id
                    .map(|id| format!("<#{}>", id))
                    .unwrap_or_else(|| "Unknown".to_string());

                let embed = CtfEmbed::new("Digest Status")
                    .description(format!(
                        "**Enabled:** ✅ Yes\n**Day:** {}\n**Channel:** {}",
                        day_name, channel
                    ))
                    .build();
                Ok(ephemeral_embed(embed))
            }
            _ => {
                let embed = CtfEmbed::new("Digest Status")
                    .description("**Enabled:** ❌ No")
                    .build();
                Ok(ephemeral_embed(embed))
            }
        },
        _ => Ok(ephemeral_error("Unknown subcommand.")),
    }
}
