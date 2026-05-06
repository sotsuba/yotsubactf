use async_trait::async_trait;
use shared::{AdminRole, AdminRoleRepository, CtfError, CtfResult};
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::{CommandBuilder, StringBuilder, SubCommandBuilder};

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed, ephemeral_error};

pub struct AdminRoleCommand;

#[async_trait]
impl SlashCommand for AdminRoleCommand {
    fn name(&self) -> &'static str {
        "adminrole"
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
            "adminrole",
            "Manage admin role mappings",
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new("add", "Grant an admin role to a Discord role")
                .option(
                    StringBuilder::new("role_id", "Discord role ID")
                        .required(true)
                        .build(),
                )
                .option(
                    StringBuilder::new("level", "Admin level")
                        .choices([
                            ("Owner", "owner"),
                            ("Admin", "admin"),
                            ("Moderator", "moderator"),
                            ("Analyst", "analyst"),
                        ])
                        .required(true)
                        .build(),
                )
                .build(),
        )
        .option(
            SubCommandBuilder::new("remove", "Remove an admin role mapping")
                .option(
                    StringBuilder::new("role_id", "Discord role ID")
                        .required(true)
                        .build(),
                )
                .build(),
        )
        .option(SubCommandBuilder::new("list", "List admin role mappings").build())
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        let gid = ctx.guild_id.ok_or_else(|| {
            CtfError::InvalidInput("This command can only be used in a server.".to_string())
        })?;
        handle(ctx.state.admin_roles.as_ref(), gid, ctx.options).await
    }
}

pub async fn handle(
    repo: &dyn AdminRoleRepository,
    guild_id: &str,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    if options.is_empty() {
        return Ok(ephemeral_error("Invalid subcommand"));
    }

    let subcmd = &options[0];
    match subcmd.name.as_str() {
        "add" => {
            let (role_id, level) = extract_role_and_level(subcmd)?;
            repo.upsert_admin_role(guild_id, &role_id, level).await?;

            let embed = CtfEmbed::success("Admin role added")
                .description(format!("Mapped <@&{role_id}> to **{}**.", level.as_str()))
                .build();

            Ok(ephemeral_embed(embed))
        }
        "remove" => {
            let role_id = extract_role_id(subcmd)?;
            let removed = repo.delete_admin_role(guild_id, &role_id).await?;

            let embed = if removed {
                CtfEmbed::success("Admin role removed")
                    .description(format!("Removed mapping for <@&{role_id}>."))
                    .build()
            } else {
                CtfEmbed::warning("No mapping found")
                    .description(format!("No admin mapping exists for <@&{role_id}>."))
                    .build()
            };

            Ok(ephemeral_embed(embed))
        }
        "list" => {
            let roles = repo.list_admin_roles(guild_id).await?;

            if roles.is_empty() {
                return Ok(ephemeral_embed(
                    CtfEmbed::new("Admin roles")
                        .description("No admin roles configured yet.")
                        .build(),
                ));
            }

            let mut lines = Vec::with_capacity(roles.len());
            for assignment in roles {
                lines.push(format!(
                    "- <@&{}>: **{}**",
                    assignment.role_id,
                    assignment.role.as_str()
                ));
            }

            Ok(ephemeral_embed(
                CtfEmbed::new("Admin roles")
                    .description(lines.join("\n"))
                    .build(),
            ))
        }
        _ => Ok(ephemeral_error("Unknown subcommand.")),
    }
}

fn extract_role_id(subcmd: &CommandDataOption) -> CtfResult<String> {
    if let CommandOptionValue::SubCommand(ref opts) = subcmd.value {
        for opt in opts {
            if opt.name == "role_id"
                && let CommandOptionValue::String(ref val) = opt.value
            {
                if val.parse::<u64>().is_err() {
                    return Err(CtfError::InvalidInput(
                        "Role ID must be a valid snowflake.".to_string(),
                    ));
                }
                return Ok(val.to_string());
            }
        }
    }

    Err(CtfError::InvalidInput("Role ID is required.".to_string()))
}

fn extract_role_and_level(subcmd: &CommandDataOption) -> CtfResult<(String, AdminRole)> {
    let mut role_id = None;
    let mut level = None;

    if let CommandOptionValue::SubCommand(ref opts) = subcmd.value {
        for opt in opts {
            match opt.name.as_str() {
                "role_id" => {
                    if let CommandOptionValue::String(ref val) = opt.value {
                        if val.parse::<u64>().is_err() {
                            return Err(CtfError::InvalidInput(
                                "Role ID must be a valid snowflake.".to_string(),
                            ));
                        }
                        role_id = Some(val.to_string());
                    }
                }
                "level" => {
                    if let CommandOptionValue::String(ref val) = opt.value {
                        level = Some(val.parse::<AdminRole>().map_err(|_| {
                            CtfError::InvalidInput(
                                "Admin level must be one of: owner, admin, moderator, analyst."
                                    .to_string(),
                            )
                        })?);
                    }
                }
                _ => {}
            }
        }
    }

    let role_id =
        role_id.ok_or_else(|| CtfError::InvalidInput("Role ID is required.".to_string()))?;
    let level =
        level.ok_or_else(|| CtfError::InvalidInput("Admin level is required.".to_string()))?;

    Ok((role_id, level))
}
