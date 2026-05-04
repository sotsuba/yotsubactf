//! `/event completed` subcommand + all its button interactions.

use crate::embed::{ephemeral_error, paged_response, CtfEmbed, PaginationNav, MAX_PAGE_SIZE};
use crate::state::AppState;
use shared::{CompletedFilter, CtfEvent, CtfResult, TeamResult};
use std::collections::HashMap;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::InteractionResponse;

const DEFAULT_COMPLETED_PAGE_SIZE: i64 = 5;

// ── Subcommand handler ────────────────────────────────────────────────────────

pub async fn handle(
    state: &AppState,
    guild_id: Option<&str>,
    options: &[CommandDataOption],
) -> CtfResult<InteractionResponse> {
    let limit = parse_limit(options);
    let filter = parse_filter(options);

    let paginated = state.events.list_completed(limit, 0, &filter).await?;
    let team_results = fetch_team_results(state, guild_id).await?;

    let has_next = paginated.events.len() as i64 >= limit && (limit < paginated.total_count);
    Ok(build_response(
        &paginated.events,
        &team_results,
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
    state: &AppState,
    guild_id: Option<&str>,
    rest: &str,
) -> CtfResult<InteractionResponse> {
    // Route by the segments of the custom_id rest.
    // Schema: page:<rest>
    let parts: Vec<&str> = rest.splitn(2, ':').collect();
    match parts.as_slice() {
        ["page", rest] => {
            let (page, limit, filter) = match parse_page_rest(rest) {
                Some(v) => v,
                None => return Ok(ephemeral_error("Unsupported interaction.")),
            };

            let offset = (page - 1) * limit;
            let paginated = state.events.list_completed(limit, offset, &filter).await?;
            let team_results = fetch_team_results(state, guild_id).await?;

            let has_next =
                paginated.events.len() as i64 >= limit && (page * limit < paginated.total_count);
            Ok(build_response(
                &paginated.events,
                &team_results,
                page,
                limit,
                page > 1,
                has_next,
                paginated.total_count,
                &filter,
                true,
            ))
        }
        _ => Ok(ephemeral_error("Unsupported interaction.")),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn fetch_team_results(
    state: &AppState,
    guild_id: Option<&str>,
) -> CtfResult<HashMap<i64, TeamResult>> {
    let mut results = HashMap::new();
    if let Some(gid) = guild_id
        && let Some(team) = state.teams.get_followed_team(gid).await?
    {
        let list = state
            .teams
            .list_recent_results(team.ctftime_team_id, 50)
            .await?;
        for r in list {
            results.insert(r.ctf_event_id, r);
        }
    }
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn build_response(
    events: &[CtfEvent],
    team_results: &HashMap<i64, TeamResult>,
    page: i64,
    limit: i64,
    has_prev: bool,
    has_next: bool,
    total_count: i64,
    filter: &CompletedFilter,
    update: bool,
) -> InteractionResponse {
    let total_pages = (total_count as f64 / limit as f64).ceil() as i64;
    let title = if total_pages > 0 {
        format!("Recently ended CTFs — Page {page} of {total_pages}")
    } else {
        "Recently ended CTFs".to_string()
    };
    let mut embed = CtfEmbed::new(title);

    if events.is_empty() {
        embed = embed.description("No completed events found matching your filters.");
    } else {
        let mut desc = String::new();
        for event in events {
            let ended_rel = format!("<t:{}:R>", event.end_time.timestamp());
            let format_tag = event.format.as_deref().unwrap_or("unknown format");
            let weight_tag = event
                .weight
                .map(|w| format!(" | {w}"))
                .unwrap_or_else(String::new);

            desc.push_str(&format!(
                "**{title}** — ended {ended_rel} | {format_tag}{weight_tag}\n",
                title = event.title
            ));

            if let Some(res) = team_results.get(&event.ctftime_id) {
                let place_str = res
                    .place
                    .map(|p| format!("{}th", p))
                    .unwrap_or_else(|| "?.th".to_string());
                let total_str = res
                    .total_teams
                    .map(|t| format!(" / {t} teams"))
                    .unwrap_or_else(String::new);
                let score_str = res
                    .score
                    .map(|s| format!(" — {:.2} pts", s))
                    .unwrap_or_else(String::new);

                // Ordinal suffix helper for common ranks
                let ordinal = match res.place {
                    Some(1) => "1st",
                    Some(2) => "2nd",
                    Some(3) => "3rd",
                    _ => &place_str,
                };

                desc.push_str(&format!(
                    "  › Team: {ordinal}{total_str}{score_str}\n"
                ));
            }
            desc.push('\n');
        }
        embed = embed.description(desc.trim_end());
    }

    let embed = embed
        .footer(format!("Total: {total_count} events"))
        .now()
        .build();

    let nav = PaginationNav {
        prev_id: format!(
            "event:completed:page:{}:{}:{}",
            page - 1,
            limit,
            filter_to_qs(filter)
        ),
        next_id: format!(
            "event:completed:page:{}:{}:{}",
            page + 1,
            limit,
            filter_to_qs(filter)
        ),
        has_prev,
        has_next,
    };

    paged_response(embed, Some(nav), vec![], update)
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
        .map(|v| v.clamp(1, MAX_PAGE_SIZE))
        .unwrap_or(DEFAULT_COMPLETED_PAGE_SIZE)
}

fn parse_filter(options: &[CommandDataOption]) -> CompletedFilter {
    let mut f = CompletedFilter::default();
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
            _ => {}
        }
    }
    f
}

fn parse_page_rest(rest: &str) -> Option<(i64, i64, CompletedFilter)> {
    let mut parts = rest.splitn(3, ':');
    let page: i64 = parts.next()?.parse().ok().filter(|&p| p > 0)?;
    let limit: i64 = parts.next()?.parse().ok().map(|v: i64| v.clamp(1, MAX_PAGE_SIZE))?;
    let filter = parts.next().map(qs_to_filter).unwrap_or_default();
    Some((page, limit, filter))
}

fn filter_to_qs(f: &CompletedFilter) -> String {
    let mut parts = vec![];
    if let Some(ref fmt) = f.format {
        parts.push(format!("f={}", fmt));
    }
    if let Some(w) = f.min_weight {
        parts.push(format!("m={}", w));
    }
    parts.join("&")
}

fn qs_to_filter(qs: &str) -> CompletedFilter {
    let mut f = CompletedFilter::default();
    for pair in qs.split('&') {
        let mut kv = pair.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("f"), Some(v)) => f.format = Some(v.to_string()),
            (Some("m"), Some(v)) => f.min_weight = v.parse().ok(),
            _ => {}
        }
    }
    f
}
