use async_trait::async_trait;
use shared::{CtfError, CtfResult};
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::application::interaction::message_component::MessageComponentInteractionData;
use twilight_model::channel::message::component::{ActionRow, Button, ButtonStyle, Component};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_util::builder::command::{CommandBuilder, StringBuilder, SubCommandBuilder};

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, DEFAULT_PAGE_SIZE, ephemeral_reply};
use crate::state::AppState;
use crate::util::get_string_option;
use shared::Writeup;

pub struct WriteupsCommand;

#[async_trait]
impl SlashCommand for WriteupsCommand {
    fn name(&self) -> &'static str {
        "writeups"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "writeups",
            "Search and browse CTF writeups",
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new("search", "Search writeups by keyword")
                .option(
                    StringBuilder::new("query", "Search term (e.g. 'bluehen')")
                        .required(true)
                        .build(),
                )
                .option(
                    StringBuilder::new("category", "Filter by category")
                        .choices([
                            ("Web", "web"),
                            ("Pwn", "pwn"),
                            ("Crypto", "crypto"),
                            ("Forensics", "forensics"),
                            ("Reverse", "rev"),
                            ("Misc", "misc"),
                            ("OSINT", "osint"),
                        ])
                        .build(),
                )
                .build(),
        )
        .option(SubCommandBuilder::new("recent", "Show most recent writeups").build())
        .option(
            SubCommandBuilder::new("event", "Browse writeups for a specific CTF event")
                .option(
                    StringBuilder::new("name", "CTF event name (partial match)")
                        .required(true)
                        .build(),
                )
                .build(),
        )
        .option(
            SubCommandBuilder::new("category", "Browse writeups by category")
                .option(
                    StringBuilder::new("name", "Category name")
                        .required(true)
                        .choices([
                            ("Web", "web"),
                            ("Pwn", "pwn"),
                            ("Crypto", "crypto"),
                            ("Forensics", "forensics"),
                            ("Reverse", "rev"),
                            ("Misc", "misc"),
                            ("OSINT", "osint"),
                        ])
                        .build(),
                )
                .build(),
        )
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        handle(ctx.guild_id, ctx.state, ctx.options).await
    }
    async fn autocomplete(
        &self,
        ctx: CommandContext<'_>,
    ) -> CtfResult<Option<InteractionResponse>> {
        Ok(Some(handle_autocomplete(ctx.state, ctx.options).await?))
    }
}

enum WriteupFetch {
    Search(String, Vec<Writeup>),
    Recent(Vec<Writeup>),
    Event(String, Vec<Writeup>),
    Category(String, Vec<Writeup>),
    Team(String, Vec<Writeup>),
}

pub async fn handle(
    guild_id: Option<&str>,
    state: &AppState,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    // Subcommands are nested in options
    let subcommand = options.iter().find_map(|opt| {
        if let twilight_model::application::interaction::application_command::CommandOptionValue::SubCommand(ref sub_options) = opt.value {
            Some((opt.name.as_str(), sub_options))
        } else {
            None
        }
    });

    let (name, sub_options) = match subcommand {
        Some(s) => s,
        None => return Ok(ephemeral_reply("Please specify a subcommand.")),
    };

    match name {
        "search" => handle_search(state, sub_options, 1).await,
        "recent" => handle_recent(state, sub_options, 1).await,
        "event" => handle_event(state, sub_options, 1).await,
        "category" => handle_category(state, sub_options, 1).await,
        "team" => handle_team(guild_id, state, sub_options, 1).await,
        "notify" => handle_notify(guild_id, state, sub_options).await,
        _ => Ok(ephemeral_reply("Unknown subcommand.")),
    }
}

pub async fn handle_component(
    state: &AppState,
    data: &MessageComponentInteractionData,
) -> CtfResult<InteractionResponse> {
    let parts: Vec<&str> = data.custom_id.split(':').collect();
    if parts.len() < 4 {
        return Ok(ephemeral_reply("Invalid interaction."));
    }

    let subcommand = parts[2];
    let page: i64 = parts[3].parse().unwrap_or(1);
    let query = parts.get(4).copied().unwrap_or("");

    let fetch = match subcommand {
        "recent" => WriteupFetch::Recent(
            state
                .writeups
                .list_recent(DEFAULT_PAGE_SIZE, (page - 1) * DEFAULT_PAGE_SIZE)
                .await?,
        ),
        "search" => {
            let res = state
                .writeups
                .search_writeups(
                    query,
                    None,
                    DEFAULT_PAGE_SIZE,
                    (page - 1) * DEFAULT_PAGE_SIZE,
                )
                .await?;
            WriteupFetch::Search(
                query.to_string(),
                res.into_iter().map(|r| r.writeup).collect(),
            )
        }
        "event" => {
            let res = state
                .writeups
                .list_by_event_name(query, DEFAULT_PAGE_SIZE, (page - 1) * DEFAULT_PAGE_SIZE)
                .await?;
            WriteupFetch::Event(query.to_string(), res)
        }
        "category" => {
            let res = state
                .writeups
                .search_writeups(
                    "",
                    Some(query),
                    DEFAULT_PAGE_SIZE,
                    (page - 1) * DEFAULT_PAGE_SIZE,
                )
                .await?;
            WriteupFetch::Category(
                query.to_string(),
                res.into_iter().map(|r| r.writeup).collect(),
            )
        }
        "team" => {
            let res = fetch_team_writeups_internal(state, query, page).await?;
            WriteupFetch::Team("Team".to_string(), res) // Actual team name will be fetched in handle_team, but for component we simplified
        }
        _ => return Ok(ephemeral_reply("Unknown subcommand interaction.")),
    };

    Ok(build_response(fetch, page, query, true))
}

async fn handle_team(
    guild_id: Option<&str>,
    state: &AppState,
    _options: &[CommandDataOption],
    page: i64,
) -> CtfResult<InteractionResponse> {
    let gid = guild_id.ok_or_else(|| {
        CtfError::InvalidInput("This subcommand can only be used in a server.".to_string())
    })?;

    let team = state.teams.get_followed_team(gid).await?;
    let team = match team {
        Some(t) => t,
        None => {
            return Ok(ephemeral_reply(
                "❌ No team is currently tracked for this guild. Use `/team follow` first.",
            ));
        }
    };

    let results = fetch_team_writeups_internal(state, gid, page).await?;

    if results.is_empty() && page == 1 {
        return Ok(ephemeral_reply(format!(
            "No writeups found for events participated by **{}**.",
            team.team_name
        )));
    }

    Ok(build_response(
        WriteupFetch::Team(team.team_name, results),
        page,
        gid,
        false,
    ))
}

async fn fetch_team_writeups_internal(
    state: &AppState,
    guild_id: &str,
    page: i64,
) -> CtfResult<Vec<Writeup>> {
    let team = state.teams.get_followed_team(guild_id).await?;
    let team = match team {
        Some(t) => t,
        None => return Ok(vec![]),
    };

    let results = state
        .teams
        .list_recent_results(team.ctftime_team_id, 20)
        .await?;
    let event_ids: Vec<i64> = results.into_iter().map(|r| r.ctf_event_id).collect();

    if event_ids.is_empty() {
        return Ok(vec![]);
    }

    state
        .writeups
        .list_by_event_ids(
            &event_ids,
            DEFAULT_PAGE_SIZE,
            (page - 1) * DEFAULT_PAGE_SIZE,
        )
        .await
}

pub async fn handle_autocomplete(
    state: &AppState,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    let focused = options.iter().find(|opt| {
        if let CommandOptionValue::Focused(ref _val, _kind) = opt.value {
            true
        } else if let CommandOptionValue::SubCommand(ref sub_opts) = opt.value {
            sub_opts
                .iter()
                .any(|so| matches!(so.value, CommandOptionValue::Focused(_, _)))
        } else {
            false
        }
    });

    let (opt_name, opt_val) = match focused {
        Some(opt) => {
            if let CommandOptionValue::SubCommand(ref sub_opts) = opt.value {
                let f = sub_opts
                    .iter()
                    .find(|so| matches!(so.value, CommandOptionValue::Focused(_, _)))
                    .unwrap();
                (
                    f.name.as_str(),
                    if let CommandOptionValue::Focused(ref v, _) = f.value {
                        v.as_str()
                    } else {
                        ""
                    },
                )
            } else if let CommandOptionValue::Focused(ref v, _) = opt.value {
                (opt.name.as_str(), v.as_str())
            } else {
                return Ok(InteractionResponse {
                    kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
                    data: Some(InteractionResponseData {
                        choices: Some(vec![]),
                        ..Default::default()
                    }),
                });
            }
        }
        None => {
            return Ok(InteractionResponse {
                kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
                data: Some(InteractionResponseData {
                    choices: Some(vec![]),
                    ..Default::default()
                }),
            });
        }
    };

    let choices = if opt_name == "name" || opt_name == "event" {
        let names = state.writeups.autocomplete_event_name(opt_val, 10).await?;
        names
            .into_iter()
            .map(
                |n| twilight_model::application::command::CommandOptionChoice {
                    name: n.clone(),
                    value: twilight_model::application::command::CommandOptionChoiceValue::String(
                        n,
                    ),
                    name_localizations: None,
                },
            )
            .collect()
    } else {
        vec![]
    };

    Ok(InteractionResponse {
        kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
        data: Some(InteractionResponseData {
            choices: Some(choices),
            ..Default::default()
        }),
    })
}

async fn handle_notify(
    guild_id: Option<&str>,
    state: &AppState,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    let enabled = options.iter()
        .find(|opt| opt.name == "enabled")
        .and_then(|opt| {
            if let twilight_model::application::interaction::application_command::CommandOptionValue::Boolean(b) = opt.value {
                Some(b)
            } else {
                None
            }
        })
        .unwrap_or(true);

    let gid = guild_id.ok_or_else(|| {
        CtfError::InvalidInput("This subcommand can only be used in a server.".to_string())
    })?;

    state.guilds.set_writeup_notify(gid, enabled).await?;

    let msg = if enabled {
        "✅ Writeup notifications enabled for this guild."
    } else {
        "❌ Writeup notifications disabled for this guild."
    };

    Ok(ephemeral_reply(msg))
}

async fn handle_search(
    state: &AppState,
    options: &[CommandDataOption],
    page: i64,
) -> CtfResult<InteractionResponse> {
    let query = get_string_option(options, "query").unwrap_or("");
    let category = get_string_option(options, "category");

    let results = state
        .writeups
        .search_writeups(
            query,
            category,
            DEFAULT_PAGE_SIZE,
            (page - 1) * DEFAULT_PAGE_SIZE,
        )
        .await?;

    if results.is_empty() && page == 1 {
        return Ok(ephemeral_reply(format!(
            "No writeups found for \"{}\".",
            query
        )));
    }

    let writeups: Vec<Writeup> = results.into_iter().map(|r| r.writeup).collect();
    Ok(build_response(
        WriteupFetch::Search(query.to_string(), writeups),
        page,
        query,
        false,
    ))
}

async fn handle_recent(
    state: &AppState,
    _options: &[CommandDataOption],
    page: i64,
) -> CtfResult<InteractionResponse> {
    let results = state
        .writeups
        .list_recent(DEFAULT_PAGE_SIZE, (page - 1) * DEFAULT_PAGE_SIZE)
        .await?;

    if results.is_empty() && page == 1 {
        return Ok(ephemeral_reply("No recent writeups found."));
    }

    Ok(build_response(
        WriteupFetch::Recent(results),
        page,
        "",
        false,
    ))
}

async fn handle_event(
    state: &AppState,
    options: &[CommandDataOption],
    page: i64,
) -> CtfResult<InteractionResponse> {
    let name = get_string_option(options, "name").unwrap_or("");

    let results = state
        .writeups
        .list_by_event_name(name, DEFAULT_PAGE_SIZE, (page - 1) * DEFAULT_PAGE_SIZE)
        .await?;

    if results.is_empty() && page == 1 {
        return Ok(ephemeral_reply(format!(
            "No writeups found for event \"{}\".",
            name
        )));
    }

    Ok(build_response(
        WriteupFetch::Event(name.to_string(), results),
        page,
        name,
        false,
    ))
}

async fn handle_category(
    state: &AppState,
    options: &[CommandDataOption],
    page: i64,
) -> CtfResult<InteractionResponse> {
    let category = get_string_option(options, "category").unwrap_or("");

    let results = state
        .writeups
        .search_writeups(
            "",
            Some(category),
            DEFAULT_PAGE_SIZE,
            (page - 1) * DEFAULT_PAGE_SIZE,
        )
        .await?;

    if results.is_empty() && page == 1 {
        return Ok(ephemeral_reply(format!(
            "No writeups found in category \"{}\".",
            category
        )));
    }

    let writeups: Vec<Writeup> = results.into_iter().map(|r| r.writeup).collect();
    Ok(build_response(
        WriteupFetch::Category(category.to_string(), writeups),
        page,
        category,
        false,
    ))
}

fn build_pagination_components(
    subcommand: &str,
    page: i64,
    query: &str,
    has_next: bool,
) -> Vec<Component> {
    let mut buttons = Vec::new();
    const MAX_QUERY_LEN: usize = 50;
    let safe_query = if query.len() > MAX_QUERY_LEN {
        &query[..MAX_QUERY_LEN]
    } else {
        query
    };

    if page > 1 {
        buttons.push(Component::Button(Button {
            custom_id: Some(format!(
                "writeups:page:{}:{}:{}",
                subcommand,
                page - 1,
                safe_query
            )),
            disabled: false,
            emoji: None,
            label: Some("⬅️ Previous".to_string()),
            style: ButtonStyle::Secondary,
            url: None,
        }));
    }

    if has_next {
        buttons.push(Component::Button(Button {
            custom_id: Some(format!(
                "writeups:page:{}:{}:{}",
                subcommand,
                page + 1,
                safe_query
            )),
            disabled: false,
            emoji: None,
            label: Some("Next ➡️".to_string()),
            style: ButtonStyle::Secondary,
            url: None,
        }));
    }

    if buttons.is_empty() {
        vec![]
    } else {
        vec![Component::ActionRow(ActionRow {
            components: buttons,
        })]
    }
}

#[allow(clippy::too_many_arguments)]
fn build_response(
    fetch: WriteupFetch,
    page: i64,
    query: &str,
    update: bool,
) -> InteractionResponse {
    let (subcmd, title, results) = match fetch {
        WriteupFetch::Search(q, r) => ("search", format!("Search: \"{}\"", q), r),
        WriteupFetch::Recent(r) => ("recent", "Recent Writeups".to_string(), r),
        WriteupFetch::Event(n, r) => ("event", format!("Writeups for \"{}\"", n), r),
        WriteupFetch::Category(c, r) => ("category", format!("Category: {}", c), r),
        WriteupFetch::Team(n, r) => ("team", format!("Writeups for Team: {}", n), r),
    };

    let has_next = results.len() as i64 >= DEFAULT_PAGE_SIZE;
    let embed = build_list_embed(title, &results);
    let components = build_pagination_components(subcmd, page, query, has_next);

    InteractionResponse {
        kind: if update {
            InteractionResponseType::UpdateMessage
        } else {
            InteractionResponseType::ChannelMessageWithSource
        },
        data: Some(InteractionResponseData {
            embeds: Some(vec![embed]),
            components: Some(components),
            ..Default::default()
        }),
    }
}

fn build_list_embed(
    title: impl Into<String>,
    results: &[Writeup],
) -> twilight_model::channel::message::embed::Embed {
    let description = if results.is_empty() {
        "No results found on this page.".to_string()
    } else {
        results
            .iter()
            .map(format_writeup_line)
            .collect::<Vec<_>>()
            .join("\n")
    };

    CtfEmbed::new(title).description(description).now().build()
}

fn format_writeup_line(w: &Writeup) -> String {
    let cat = w
        .category
        .as_deref()
        .map(|c| format!(" `{}`", c))
        .unwrap_or_default();
    let event = w
        .event_name
        .as_deref()
        .map(|e| format!(" — *{}*", e))
        .unwrap_or_default();

    format!("• **[{}]({})**{}{}", w.title, w.url, cat, event)
}
