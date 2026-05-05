use async_trait::async_trait;
use shared::{CtfError, CtfResult};
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::channel::message::component::{ActionRow, Button, ButtonStyle, Component};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_util::builder::command::{
    CommandBuilder, IntegerBuilder, StringBuilder, SubCommandBuilder,
};

use super::{CommandContext, SlashCommand};
use crate::ctftime_api;
use crate::embed::{CtfEmbed, ephemeral_embed, ephemeral_error};
use crate::state::AppState;

pub struct TeamCommand;

#[async_trait]
impl SlashCommand for TeamCommand {
    fn name(&self) -> &'static str {
        "team"
    }
    fn requires_guild(&self) -> bool {
        true
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "team",
            "CTFTime team management and lookup",
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new("search", "Search for a team on CTFtime")
                .option(
                    StringBuilder::new("name", "Team name")
                        .required(true)
                        .build(),
                )
                .build(),
        )
        .option(
            SubCommandBuilder::new("follow", "Track a team's results (1 team per guild)")
                .option(
                    IntegerBuilder::new("id", "CTFTime Team ID")
                        .required(true)
                        .build(),
                )
                .build(),
        )
        .option(SubCommandBuilder::new("unfollow", "Stop tracking this team's results").build())
        .option(
            SubCommandBuilder::new("following", "Show the team currently tracked by this guild")
                .build(),
        )
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        let gid = ctx.guild_id.ok_or_else(|| {
            CtfError::InvalidInput("This command can only be used in a server.".to_string())
        })?;
        handle(ctx.state, gid, ctx.options).await
    }
}

pub async fn handle(
    state: &AppState,
    guild_id: &str,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    if options.is_empty() {
        return Ok(ephemeral_error("Invalid subcommand."));
    }

    let subcmd = &options[0];
    match subcmd.name.as_str() {
        "search" => {
            let opts = match &subcmd.value {
                CommandOptionValue::SubCommand(o) => o,
                _ => return Ok(ephemeral_error("Invalid search options.")),
            };

            let name = match opts.iter().find(|o| o.name == "name") {
                Some(o) => match &o.value {
                    CommandOptionValue::String(s) => s.clone(),
                    _ => return Ok(ephemeral_error("Invalid team name.")),
                },
                None => return Ok(ephemeral_error("Please provide a team name.")),
            };

            handle_search(state, name).await
        }
        "follow" => {
            let opts = match &subcmd.value {
                CommandOptionValue::SubCommand(o) => o,
                _ => return Ok(ephemeral_error("Invalid follow options.")),
            };

            let team_id = match opts.iter().find(|o| o.name == "id") {
                Some(o) => match &o.value {
                    CommandOptionValue::Integer(i) => *i,
                    _ => return Ok(ephemeral_error("Invalid team ID.")),
                },
                None => return Ok(ephemeral_error("Please provide a team ID.")),
            };

            // Fetch team name from API to verify ID and get name
            let team_name = match ctftime_api::get_team_name(&state.http_api, team_id).await {
                Ok(Some(name)) => name,
                Ok(None) => return Ok(ephemeral_error("Team not found on CTFtime.")),
                Err(e) => {
                    tracing::warn!(error = ?e, team_id, "Failed to verify team with CTFtime API");
                    return Ok(ephemeral_error("Failed to verify team with CTFtime API."));
                }
            };

            state
                .teams
                .follow_team(guild_id, team_id, &team_name)
                .await?;

            let embed = CtfEmbed::success("Team Followed")
                .description(format!(
                    "Successfully followed **{}** (ID: {}). New results will be posted here.",
                    team_name, team_id
                ))
                .build();
            Ok(ephemeral_embed(embed))
        }
        "unfollow" => {
            if state.teams.unfollow_team(guild_id).await? {
                let embed = CtfEmbed::success("Team Unfollowed")
                    .description("Stopped tracking team results for this server.")
                    .build();
                Ok(ephemeral_embed(embed))
            } else {
                Ok(ephemeral_error("This server is not following any team."))
            }
        }
        "following" => match state.teams.get_followed_team(guild_id).await? {
            Some(team) => {
                let embed = CtfEmbed::new("Currently Following")
                    .description(format!(
                        "**Team:** {}\n**ID:** {}\n**Since:** <t:{}:R>",
                        team.team_name,
                        team.ctftime_team_id,
                        team.created_at.timestamp()
                    ))
                    .build();
                Ok(ephemeral_embed(embed))
            }
            None => {
                let embed = CtfEmbed::new("Following Status")
                    .description("This server is not following any team.")
                    .build();
                Ok(ephemeral_embed(embed))
            }
        },
        _ => Ok(ephemeral_error("Unknown subcommand.")),
    }
}

async fn handle_search(state: &AppState, name: String) -> CtfResult<InteractionResponse> {
    let mut results = if let Some(cached) = state.team_cache.get(&name).await {
        cached
    } else {
        let r = match ctftime_api::search_team(&state.http_api, &name).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = ?e, team_name = %name, "Failed to fetch team search results from CTFtime");
                return Ok(ephemeral_error("Failed to fetch data from CTFtime."));
            }
        };
        state.team_cache.insert(name.clone(), r.clone()).await;
        r
    };

    if results.is_empty() {
        return Ok(ephemeral_error("Team not found."));
    }

    // Fast-track: if we find an exact case-insensitive match, treat it as the only result
    let query_lower = name.to_lowercase();
    let exact_match = results
        .iter()
        .find(|t| t.name.to_lowercase() == query_lower)
        .cloned();

    let (team_data, results_is_single) = if let Some(exact) = exact_match {
        (Some(exact), true)
    } else if results.len() == 1 {
        (Some(results[0].clone()), true)
    } else {
        (None, false)
    };

    if results_is_single {
        let team = team_data.unwrap();
        let url = format!("https://ctftime.org/team/{}", team.id);
        let mut embed = CtfEmbed::new(&team.name).now();

        if !team.country.is_empty() {
            embed = embed.field("Country", &team.country, true);
        }
        if let Some(r) = team.rating {
            embed = embed.field("Rating", format!("{:.2}", r), true);
        }
        if !team.aliases.is_empty() {
            embed = embed.field("Aliases", team.aliases.join(", "), false);
        }

        let embed = embed.build();
        let button = Component::Button(Button {
            custom_id: None,
            disabled: false,
            emoji: None,
            id: None,
            label: Some("View on CTFtime".to_string()),
            sku_id: None,
            style: ButtonStyle::Link,
            url: Some(url),
        });

        Ok(InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseData {
                embeds: Some(vec![embed]),
                components: Some(vec![Component::ActionRow(ActionRow {
                    components: vec![button],
                    id: None,
                })]),
                ..Default::default()
            }),
        })
    } else {
        results.truncate(5);

        let mut desc = String::new();
        let mut buttons = Vec::new();

        for (i, team) in results.iter().enumerate() {
            let url = format!("https://ctftime.org/team/{}", team.id);
            desc.push_str(&format!("{}. **{}** (ID: {})\n", i + 1, team.name, team.id));
            if !team.country.is_empty() {
                desc.push_str(&format!("Country: {}\n", team.country));
            }
            if let Some(r) = team.rating {
                desc.push_str(&format!("Rating: {:.2}\n", r));
            }
            desc.push('\n');

            buttons.push(Component::Button(Button {
                custom_id: None,
                disabled: false,
                emoji: None,
                id: None,
                label: Some(team.name.clone()),
                sku_id: None,
                style: ButtonStyle::Link,
                url: Some(url),
            }));
        }

        let embed = CtfEmbed::new(format!("Multiple teams found matching '{}'", name))
            .description(desc.trim_end())
            .now()
            .build();

        Ok(InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseData {
                embeds: Some(vec![embed]),
                components: Some(vec![Component::ActionRow(ActionRow {
                    components: buttons,
                    id: None,
                })]),
                ..Default::default()
            }),
        })
    }
}
