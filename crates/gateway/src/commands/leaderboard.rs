use async_trait::async_trait;
use chrono::Datelike;
use shared::CtfResult;
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_util::builder::command::{CommandBuilder, IntegerBuilder};

use super::{CommandContext, SlashCommand};
use crate::ctftime_api;
use crate::embed::{CtfEmbed, ephemeral_error};
use crate::state::AppState;

pub struct LeaderboardCommand;

#[async_trait]
impl SlashCommand for LeaderboardCommand {
    fn name(&self) -> &'static str {
        "leaderboard"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new("leaderboard", "CTFtime Leaderboard", CommandType::ChatInput)
            .option(IntegerBuilder::new("year", "Year (default: current year)").build())
            .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        handle(ctx.state, ctx.options).await
    }
}

pub async fn handle(
    state: &AppState,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    let mut year = chrono::Utc::now().year();
    if let Some(opt) = options.iter().find(|o| o.name == "year")
        && let CommandOptionValue::Integer(y) = &opt.value
    {
        year = *y as i32;
    }

    let entries = if let Some(cached) = state.leaderboard_cache.get(&year).await {
        cached
    } else {
        let e = match ctftime_api::fetch_top(&state.http_api, year).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(error = ?e, year, "Failed to fetch leaderboard from CTFtime");
                return Ok(ephemeral_error("Failed to fetch data from CTFtime."));
            }
        };
        state.leaderboard_cache.insert(year, e.clone()).await;
        e
    };

    let mut desc = String::new();
    if entries.is_empty() {
        if year == chrono::Utc::now().year() {
            desc = "CTFtime does not have leaderboard data for the current year yet. This usually populates as events conclude.".to_string();
        } else {
            desc = "No data found for this year.".to_string();
        }
    } else {
        for (rank, entry) in entries.into_iter().take(10) {
            desc.push_str(&format!(
                "**{}.** {} — {:.2} pts\n",
                rank, entry.team_name, entry.points
            ));
        }
    }

    let embed = CtfEmbed::new(format!("CTFtime Leaderboard {}", year))
        .description(desc)
        .field(
            "Source",
            format!("https://ctftime.org/stats/{}", year),
            false,
        )
        .now()
        .build();

    Ok(InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            embeds: Some(vec![embed]),
            ..Default::default()
        }),
    })
}
