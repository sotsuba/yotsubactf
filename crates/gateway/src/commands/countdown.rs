use async_trait::async_trait;
use chrono::Utc;
use shared::CtfResult;
use shared::ReadCtfRepository;
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::{CommandBuilder, StringBuilder};

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed, ephemeral_error};

pub struct CountdownCommand;

#[async_trait]
impl SlashCommand for CountdownCommand {
    fn name(&self) -> &'static str {
        "countdown"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "countdown",
            "Countdown to when a CTF starts or ends",
            CommandType::ChatInput,
        )
        .option(
            StringBuilder::new("name", "CTF name (or partial name, case-insensitive)")
                .required(true)
                .build(),
        )
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        handle(ctx.state.events.as_ref(), ctx.options).await
    }
}

pub async fn handle(
    repo: &dyn ReadCtfRepository,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    // Step 1: parse the required "name" option.
    let query = match parse_name(options) {
        Some(q) => q,
        None => return Ok(ephemeral_error("Please provide a CTF name.")),
    };

    // Step 2: find the event via fuzzy title match.
    let event = match repo.get_by_title_fuzzy(&query).await? {
        Some(e) => e,
        None => {
            let embed = CtfEmbed::warning("Not found")
                .description(format!(
                    "No CTF matching **\"{}\"** was found among upcoming or ongoing events.\n\
                     Try `/upcoming` to browse the full list.",
                    query
                ))
                .now()
                .build();
            return Ok(ephemeral_embed(embed));
        }
    };

    // Step 3: guard against the small race window where the event ended
    // between the DB query and now.
    let now = Utc::now();
    if now >= event.end_time {
        let embed = CtfEmbed::warning("CTF already ended")
            .description(format!("**{}** has already ended.", event.title))
            .now()
            .build();
        return Ok(ephemeral_embed(embed));
    }

    // Step 4: determine current phase.
    let (phase_label, target_time) = if now < event.start_time {
        ("Starts in", event.start_time)
    } else {
        // now >= start_time && now < end_time → running
        ("Ends in", event.end_time)
    };

    // Step 5: compute human-readable countdown.
    let duration = target_time - now;
    let days = duration.num_days();
    let hours = duration.num_hours() % 24;
    let minutes = duration.num_minutes() % 60;
    let total_minutes = duration.num_minutes();

    let countdown_str = if total_minutes < 60 {
        format!("{} minute(s)", minutes)
    } else if days == 0 {
        format!("{} hour(s) {} minute(s)", hours, minutes)
    } else {
        format!("{} day(s) {} hour(s)", days, hours)
    };

    // Step 6: Discord relative timestamp.
    let discord_ts = format!("<t:{}:R>", target_time.timestamp());

    // Step 7: build embed.
    let status_line = if now < event.start_time {
        "Not yet started"
    } else {
        "In progress"
    };

    let embed = CtfEmbed::new(&event.title)
        .description(format!(
            "**Status:** {status_line}\n\
             **{phase_label}:** {countdown_str} ({discord_ts})\n\
             {}",
            event.url,
        ))
        .now()
        .build();

    Ok(ephemeral_embed(embed))
}

fn parse_name(options: &[CommandDataOption]) -> Option<String> {
    options.iter().find(|o| o.name == "name").and_then(|o| {
        if let CommandOptionValue::String(s) = &o.value {
            Some(s.clone())
        } else {
            None
        }
    })
}
