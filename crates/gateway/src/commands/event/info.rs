use crate::embed::{ephemeral_error, CtfEmbed};
use chrono::Utc;
use shared::{CtfError, CtfResult, ReadCtfRepository};
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::channel::message::component::{ActionRow, Button, ButtonStyle, Component};
use twilight_model::http::interaction::InteractionResponse;

// ── Subcommand handler ────────────────────────────────────────────────────────

pub async fn handle(
    repo: &dyn ReadCtfRepository,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    let query = parse_name(options)
        .ok_or_else(|| CtfError::InvalidInput("Please provide a CTF name.".into()))?;

    let event = match repo.get_all_by_title_fuzzy(&query).await? {
        Some(e) => e,
        None => return Ok(ephemeral_error("No CTF found matching that name.")),
    };

    let now = Utc::now();
    let status = match (now < event.start_time, now < event.end_time) {
        (true, _) => format!("Upcoming — starts <t:{}:R>", event.start_time.timestamp()),
        (_, true) => format!("Live now — ends <t:{}:R>", event.end_time.timestamp()),
        _ => format!("Ended <t:{}:R>", event.end_time.timestamp()),
    };

    let mut embed = CtfEmbed::new(&event.title)
        .description(format!("**Status:** {status}"))
        .field("Format", event.format.as_deref().unwrap_or("Unknown"), true);

    if let Some(w) = event.weight {
        embed = embed.field("Weight", w.to_string(), true);
    }

    if let Some(ref org) = event.organiser {
        embed = embed.field("Organiser", org, true);
    }

    if let Some(ref desc) = event.description {
        let truncated = crate::util::truncate(desc, 300);
        embed = embed.field("Description", truncated, false);
    }

    if !event.social_links.is_empty() {
        let links = event
            .social_links
            .iter()
            .map(|l| format!("[{}]({})", l.platform.emoji_label(), l.url))
            .collect::<Vec<_>>()
            .join(" | ");
        embed = embed.field("Social Links", links, false);
    }

    let ctftime_url = format!("https://ctftime.org/event/{}", event.ctftime_id);
    let button = Component::Button(Button {
        custom_id: None,
        disabled: false,
        emoji: None,
        label: Some("View on CTFtime".to_string()),
        style: ButtonStyle::Link,
        url: Some(ctftime_url),
    });

    Ok(InteractionResponse {
        kind: twilight_model::http::interaction::InteractionResponseType::ChannelMessageWithSource,
        data: Some(twilight_model::http::interaction::InteractionResponseData {
            embeds: Some(vec![embed.now().build()]),
            components: Some(vec![Component::ActionRow(ActionRow {
                components: vec![button],
            })]),
            ..Default::default()
        }),
    })
}

#[allow(dead_code)]
pub async fn handle_component(
    _repo: &dyn ReadCtfRepository,
    _message_id: &str,
    _custom_id: &str,
) -> CtfResult<InteractionResponse> {
    Ok(ephemeral_error("Unsupported interaction."))
}

fn parse_name(options: &[CommandDataOption]) -> Option<String> {
    options.iter().find(|o| o.name == "query").and_then(|o| {
        if let CommandOptionValue::String(s) = &o.value {
            Some(s.clone())
        } else {
            None
        }
    })
}
