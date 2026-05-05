//! `/event upcoming` subcommand + all its button interactions.
//!
//! ## Component custom_id schema
//!
//! | Pattern                           | Meaning                                          |
//! |-----------------------------------|--------------------------------------------------|
//! | `event:upcoming:page:<p>:<limit>` | Navigate to page `p` with `limit` events/page.  |
//! | `event:upcoming:join:<ctftime_id>`| Show ephemeral join-links for that CTF event.    |

use crate::embed::{
    CtfEmbed, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, PaginationNav, ephemeral_error,
    join_community_response, paged_response,
};
use crate::state::AppState;
use crate::util::truncate;
use shared::CtfEvent;
use shared::CtfResult;
use shared::ReadCtfRepository;
use shared::UpcomingFilter;
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
    let filter = parse_filter(options);
    let paginated = fetch_page(repo, 1, limit, &filter).await?;
    let has_next = paginated.events.len() as i64 >= limit && (limit < paginated.total_count);
    Ok(build_paged_response(
        &paginated.events,
        1,
        limit,
        false,
        has_next,
        paginated.total_count,
        &filter,
        false,
    ))
}

// ── Button component dispatcher ───────────────────────────────────────────────

pub async fn handle_component(
    repo: &dyn ReadCtfRepository,
    rest: &str,
) -> CtfResult<InteractionResponse> {
    // Route by the segments of the custom_id rest.
    // Schema: page:<rest>
    let parts: Vec<&str> = rest.splitn(2, ':').collect();
    match parts.as_slice() {
        ["page", rest] => handle_page_component(repo, rest).await,
        ["join", ctftime_id_str] => handle_join_component(repo, ctftime_id_str).await,
        _ => Ok(ephemeral_error("Unsupported interaction.")),
    }
}

// ── Pagination handler ────────────────────────────────────────────────────────

async fn handle_page_component(
    repo: &dyn ReadCtfRepository,
    rest: &str,
) -> CtfResult<InteractionResponse> {
    let (page, limit, filter) = match parse_page_rest(rest) {
        Some(v) => v,
        None => return Ok(ephemeral_error("Unsupported interaction.")),
    };
    let paginated = fetch_page(repo, page, limit, &filter).await?;
    let has_next = paginated.events.len() as i64 >= limit && (page * limit < paginated.total_count);
    Ok(build_paged_response(
        &paginated.events,
        page,
        limit,
        page > 1,
        has_next,
        paginated.total_count,
        &filter,
        true,
    ))
}

// ── Join-community handler ────────────────────────────────────────────────────

async fn handle_join_component(
    repo: &dyn ReadCtfRepository,
    ctftime_id_str: &str,
) -> CtfResult<InteractionResponse> {
    let ctftime_id: i64 = match ctftime_id_str.parse() {
        Ok(v) => v,
        Err(_) => return Ok(ephemeral_error("Invalid event ID.")),
    };

    match repo.get_by_ctftime_id(ctftime_id).await? {
        Some(event) => Ok(join_community_response(&event.title, &event.social_links)),
        None => Ok(ephemeral_error("Event not found in database.")),
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

async fn fetch_page(
    repo: &dyn ReadCtfRepository,
    page: i64,
    limit: i64,
    filter: &UpcomingFilter,
) -> CtfResult<shared::PaginatedEvents> {
    let offset = (page - 1) * limit;
    repo.list_upcoming(limit, offset, filter).await
}

#[allow(clippy::too_many_arguments)]
fn build_paged_response(
    events: &[CtfEvent],
    page: i64,
    limit: i64,
    has_prev: bool,
    has_next: bool,
    total_count: i64,
    filter: &UpcomingFilter,
    update: bool,
) -> InteractionResponse {
    let total_pages = (total_count as f64 / limit as f64).ceil() as i64;
    let title = if total_pages > 0 {
        format!("Upcoming CTF events — Page {page} of {total_pages}")
    } else {
        "Upcoming CTF events".to_string()
    };
    let mut embed = CtfEmbed::new(title);

    let mut extra_rows = Vec::new();

    if events.is_empty() {
        embed = embed.description("No upcoming events found.");
    } else {
        let mut desc = String::new();
        for (i, event) in events.iter().enumerate() {
            let global_n = (page - 1) * limit + i as i64 + 1;
            desc.push_str(&format_event_line(event, global_n as usize));
            desc.push_str("\n\n");
        }
        embed = embed.description(desc.trim_end());

        let join_buttons: Vec<Component> = events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let global_n = (page - 1) * limit + i as i64 + 1;
                let title_trunc = truncate(&e.title, 12);
                Component::Button(Button {
                    custom_id: Some(format!("event:upcoming:join:{}", e.ctftime_id)),
                    disabled: e.social_links.is_empty(),
                    emoji: None,
                    label: Some(format!("#{} {}", global_n, title_trunc)),
                    style: if !e.social_links.is_empty() {
                        ButtonStyle::Primary
                    } else {
                        ButtonStyle::Secondary
                    },
                    url: None,
                })
            })
            .collect();

        for chunk in join_buttons.chunks(5) {
            extra_rows.push(Component::ActionRow(ActionRow {
                components: chunk.to_vec(),
            }));
        }
    }

    let embed = embed
        .footer(format!("Total: {total_count} events"))
        .now()
        .build();

    let nav = PaginationNav {
        prev_id: format!(
            "event:upcoming:page:{}:{}:{}",
            page - 1,
            limit,
            filter_to_qs(filter, 80)
        ),
        next_id: format!(
            "event:upcoming:page:{}:{}:{}",
            page + 1,
            limit,
            filter_to_qs(filter, 80)
        ),
        has_prev,
        has_next,
    };

    paged_response(embed, Some(nav), extra_rows, update)
}

fn format_event_line(event: &CtfEvent, index: usize) -> String {
    let local_start = format!("<t:{}:F>", event.start_time.timestamp());
    let local_end = format!("<t:{}:F>", event.end_time.timestamp());
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

    let social_badge = if event.social_links.is_empty() {
        String::new()
    } else {
        let icons: Vec<&str> = event
            .social_links
            .iter()
            .map(|l| platform_icon(&l.platform))
            .collect();
        format!("\n🔗 {}", icons.join("  "))
    };

    // Show a short preview of the description (first 120 chars, no newlines).
    let desc_preview = event
        .description
        .as_deref()
        .map(|d| d.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|d| !d.is_empty())
        .map(|d| truncate(&d, 120))
        .map(|d| format!("\n> {}", d))
        .unwrap_or_default();

    format!(
        "{index}. **{title}**\n{url}\nTime: {local_start} → {local_end}\n{format_tag}{weight_tag}{onsite_tag}{social_badge}{desc_preview}",
        title = event.title,
        url = event.url,
    )
}

fn platform_icon(platform: &shared::SocialPlatform) -> &'static str {
    use shared::SocialPlatform::*;
    match platform {
        Discord => "🎮",
        Telegram => "✈️",
        Slack => "💬",
        Matrix => "🟦",
        Irc => "📡",
    }
}

// ── Custom-id parsing ─────────────────────────────────────────────────────────

fn parse_page_rest(rest: &str) -> Option<(i64, i64, UpcomingFilter)> {
    let mut parts = rest.splitn(3, ':');

    let page: i64 = parts.next()?.parse().ok().filter(|&p| p > 0)?;
    let limit: i64 = parts.next().and_then(|s| s.parse().ok()).map(clamp_limit)?;
    let filter = parts.next().map(qs_to_filter).unwrap_or_default();

    Some((page, limit, filter))
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
        .map(clamp_limit)
        .unwrap_or(DEFAULT_PAGE_SIZE)
}

fn clamp_limit(v: i64) -> i64 {
    v.clamp(1, MAX_PAGE_SIZE)
}

// ── Filter helpers ────────────────────────────────────────────────────────────

fn parse_filter(options: &[CommandDataOption]) -> UpcomingFilter {
    let mut f = UpcomingFilter::default();
    for opt in options {
        match opt.name.as_str() {
            "format" => {
                if let CommandOptionValue::String(ref v) = opt.value {
                    f.format = Some(v.clone());
                }
            }
            "weight_min" => {
                if let CommandOptionValue::Number(v) = opt.value {
                    f.min_weight = Some(v);
                }
            }
            "weight_max" => {
                if let CommandOptionValue::Number(v) = opt.value {
                    f.max_weight = Some(v);
                }
            }
            "onsite" => {
                if let CommandOptionValue::Boolean(v) = opt.value {
                    f.onsite = Some(v);
                }
            }
            "sort_by" => {
                if let CommandOptionValue::String(ref v) = opt.value {
                    f.sort_by = Some(v.clone());
                }
            }
            _ => {}
        }
    }
    f
}

pub fn filter_to_qs(f: &UpcomingFilter, budget: usize) -> String {
    let mut parts: Vec<String> = vec![];
    if let Some(ref fmt) = f.format {
        let mut fmt_str = fmt.clone();
        if fmt_str.len() > 20 {
            fmt_str.truncate(20);
        }
        parts.push(format!("f={fmt_str}"));
    }
    if let Some(w) = f.min_weight {
        parts.push(format!("min={w}"));
    }
    if let Some(w) = f.max_weight {
        parts.push(format!("max={w}"));
    }
    if let Some(o) = f.onsite {
        parts.push(format!("o={}", if o { 1 } else { 0 }));
    }
    if let Some(ref s) = f.sort_by {
        parts.push(format!("s={s}"));
    }

    let mut qs = String::new();
    for p in parts {
        if qs.len() + p.len() + 1 > budget {
            break;
        }
        if !qs.is_empty() {
            qs.push('&');
        }
        qs.push_str(&p);
    }
    qs
}

pub fn qs_to_filter(qs: &str) -> UpcomingFilter {
    let mut f = UpcomingFilter::default();
    if qs.is_empty() {
        return f;
    }
    for pair in qs.split('&') {
        let mut kv = pair.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("f"), Some(v)) => f.format = Some(v.to_string()),
            (Some("min"), Some(v)) => f.min_weight = v.parse().ok(),
            (Some("max"), Some(v)) => f.max_weight = v.parse().ok(),
            (Some("o"), Some(v)) => f.onsite = Some(v == "1"),
            (Some("s"), Some(v)) => f.sort_by = Some(v.to_string()),
            _ => {}
        }
    }
    f
}
