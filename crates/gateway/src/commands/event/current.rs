//! `/event current` subcommand + all its button interactions.

use crate::embed::{
    ephemeral_error, paged_response, CtfEmbed, PaginationNav, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
};
use crate::state::AppState;
use crate::util::truncate;
use shared::{CtfEvent, CtfResult, ReadCtfRepository};
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::channel::message::component::{ActionRow, Button, ButtonStyle, Component};
use twilight_model::http::interaction::InteractionResponse;

// ── Subcommand handler ────────────────────────────────────────────────────────

pub async fn handle(
    state: &AppState,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    let repo = state.events.as_ref();
    let limit = parse_limit(options);
    let paginated = fetch_page(repo, 1, limit).await?;
    let has_next = paginated.events.len() as i64 >= limit && (limit < paginated.total_count);
    Ok(build_response(
        &paginated.events,
        1,
        limit,
        false,
        has_next,
        paginated.total_count,
        false,
    ))
}

// ── Button component dispatcher ───────────────────────────────────────────────

pub async fn handle_component(
    repo: &dyn ReadCtfRepository,
    rest: &str,
) -> CtfResult<InteractionResponse> {
    // Route by the segments of the custom_id rest.
    // Schema: page:<p>:<limit>
    let parts: Vec<&str> = rest.splitn(2, ':').collect();
    match parts.as_slice() {
        ["page", rest] => {
            let mut parts = rest.splitn(2, ':');
            let page: i64 = parts
                .next()
                .and_then(|p| p.parse().ok())
                .filter(|&p| p > 0)
                .unwrap_or(1);
            let limit: i64 = parts
                .next()
                .and_then(|s| s.parse::<i64>().ok())
                .map(|v| v.clamp(1, MAX_PAGE_SIZE))
                .unwrap_or(DEFAULT_PAGE_SIZE);
            let paginated = fetch_page(repo, page, limit).await?;
            let has_next =
                paginated.events.len() as i64 >= limit && (page * limit < paginated.total_count);
            Ok(build_response(
                &paginated.events,
                page,
                limit,
                page > 1,
                has_next,
                paginated.total_count,
                true,
            ))
        }
        _ => Ok(ephemeral_error("Unsupported interaction.")),
    }
}

// ── Pagination handler ────────────────────────────────────────────────────────

async fn fetch_page(
    repo: &dyn ReadCtfRepository,
    page: i64,
    limit: i64,
) -> CtfResult<shared::PaginatedEvents> {
    let offset = (page - 1) * limit;
    repo.list_current(limit, offset).await
}

fn build_response(
    events: &[CtfEvent],
    page: i64,
    limit: i64,
    has_prev: bool,
    has_next: bool,
    total_count: i64,
    update: bool,
) -> InteractionResponse {
    if events.is_empty() {
        let embed = CtfEmbed::new("No CTFs currently running")
            .description(
                "There are no CTF events in progress right now.\n\
                 Use `/event upcoming` to see what's coming up next.",
            )
            .now()
            .build();
        return paged_response(embed, None, vec![], update);
    }

    let mut desc = String::new();
    let mut extra_rows = Vec::new();

    for (i, event) in events.iter().enumerate() {
        let global_n = (page - 1) * limit + i as i64 + 1;
        desc.push_str(&format_current_line(event, global_n as usize));
        desc.push_str("\n\n");
    }

    let total_pages = (total_count as f64 / limit as f64).ceil() as i64;
    let embed = CtfEmbed::new(format!("CTFs in progress — Page {page} of {total_pages}"))
        .description(desc.trim_end())
        .footer(format!("Total: {total_count} events"))
        .now()
        .build();

    let buttons: Vec<Component> = events
        .iter()
        .enumerate()
        .map(|(i, event)| {
            let global_n = (page - 1) * limit + i as i64 + 1;
            let title_trunc = truncate(&event.title, 12);
            Component::Button(Button {
                custom_id: Some(format!("remind:{}", event.ctftime_id)),
                disabled: false,
                emoji: None,
                label: Some(format!("#{} {}", global_n, title_trunc)),
                style: ButtonStyle::Primary,
                url: None,
            })
        })
        .collect();

    for chunk in buttons.chunks(5) {
        extra_rows.push(Component::ActionRow(ActionRow {
            components: chunk.to_vec(),
        }));
    }

    let nav = PaginationNav {
        prev_id: format!("event:current:page:{}:{}", page - 1, limit),
        next_id: format!("event:current:page:{}:{}", page + 1, limit),
        has_prev,
        has_next,
    };

    paged_response(embed, Some(nav), extra_rows, update)
}

fn parse_limit(options: &[CommandDataOption]) -> i64 {
    options
        .iter()
        .find(|o| o.name == "count")
        .and_then(|o| {
            if let CommandOptionValue::Integer(v) = o.value {
                Some(v)
            } else {
                None
            }
        })
        .map(|v: i64| v.clamp(1, MAX_PAGE_SIZE))
        .unwrap_or(DEFAULT_PAGE_SIZE)
}

fn format_current_line(event: &CtfEvent, index: usize) -> String {
    let end_rel = format!("<t:{}:R>", event.end_time.timestamp());

    let format_tag = event
        .format
        .as_deref()
        .map(|f| format!("Format: {f}"))
        .unwrap_or_else(|| "Format: unknown".to_string());

    let weight_tag = if let Some(w) = event.weight {
        format!(" | ⚖️ Weight: {w}")
    } else {
        String::new()
    };

    let onsite_tag = if event.is_onsite {
        " | 📍 Onsite"
    } else {
        ""
    };

    format!(
        "{index}. **{title}**\n{url}\nEnds: {end_rel}\n{format_tag}{weight_tag}{onsite_tag}",
        title = event.title,
        url = event.url,
    )
}
