//! Core scrape → enrich → store → notify pipeline.
//!
//! Dependency inversion
//! ────────────────────
//! The notifier receives pre-resolved channel IDs and has no DB access.
//!   pipeline → guild_repo.list_active_subscriptions()  (resolve channel IDs)
//!   pipeline → notifier.send(event, &channel_ids)       (just send, no DB)
//!
//! Re-notify guard
//! ───────────────
//! `upsert_event` returns [`UpsertStatus`]. Only [`UpsertStatus::Inserted`]
//! (a brand-new event) triggers a notification. Updates caused by schedule
//! tweaks or enrichment passes are intentionally silent.
//!
//! Concurrent enrichment
//! ─────────────────────
//! Each event requires two HTTP round-trips (CTFTime page + the CTF's own
//! website). With 50 events that is ~100 sequential requests without
//! parallelism. Events are now enriched concurrently, bounded by
//! `MAX_CONCURRENT_ENRICHMENT` to avoid hammering CTFTime.
//!
//! Single CTFTime fetch per event
//! ──────────────────────────────
//! Previously `fetch_event_patch` and `fetch_social_links` both fetched
//! `ctftime.org/event/{id}`. Now `fetch_event_patch` is the single CTFTime
//! fetch: it extracts patch data *and* on-page social links in one pass.
//! `fetch_external_social_links` separately checks only the CTF's own URL.

use std::sync::Arc;

use reqwest::Client;
use shared::CtfResult;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use metrics;
use shared::{CtfEventRepository, GuildRepository, Notifier, UpsertStatus};

use crate::ctftime::{api, html, models::EnrichedEvent};

const DEFAULT_ENRICH_CONCURRENCY: usize = 5;
const MAX_ENRICH_CONCURRENCY: usize = 50;

fn enrich_concurrency() -> usize {
    let parsed = std::env::var("ENRICH_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_ENRICH_CONCURRENCY);
    parsed.clamp(1, MAX_ENRICH_CONCURRENCY)
}

// ── Public entry points ───────────────────────────────────────────────────────

#[derive(Default, Debug, Clone, Copy)]
pub struct ScrapeStats {
    pub inserted: usize,
    pub updated: usize,
    pub notified: usize,
    pub errors: usize,
}

/// Scrape CTFTime, upsert new/changed events, notify only for new inserts.
pub async fn run_once(
    http: &Client,
    llm: Option<&crate::llm::GeminiClient>,
    event_repo: &(impl CtfEventRepository + ?Sized),
    guild_repo: &(impl GuildRepository + ?Sized),
    notifier: &(impl Notifier + ?Sized),
) -> CtfResult<ScrapeStats> {
    // 1. Fetch raw events from the REST API.
    let raw_events = api::fetch_upcoming(http).await?;
    info!(count = raw_events.len(), "Fetched events from CTFTime API");

    // 2. Enrich all events concurrently.
    let sem = Arc::new(Semaphore::new(enrich_concurrency()));
    let mut join_set: JoinSet<EnrichedEvent> = JoinSet::new();

    for raw in raw_events {
        let http = http.clone();
        let llm = llm.cloned();
        let sem = Arc::clone(&sem);
        join_set.spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore never closed");
            let mut ev = EnrichedEvent::new(raw);
            enrich_event(&http, llm.as_ref(), &mut ev).await;
            ev
        });
    }

    let mut enriched: Vec<shared::CtfEvent> = Vec::with_capacity(join_set.len());
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(ev) => match shared::CtfEvent::try_from(ev) {
                Ok(e) => enriched.push(e),
                Err(err) => error!(?err, "Failed to convert enriched event"),
            },
            Err(e) => error!(?e, "enrichment task panicked — event skipped"),
        }
    }

    // 3. Process the enriched events.
    process_events(&enriched, event_repo, guild_repo, notifier).await
}

/// Core processing logic: upsert events and notify if brand new.
pub async fn process_events(
    events: &[shared::CtfEvent],
    event_repo: &(impl CtfEventRepository + ?Sized),
    guild_repo: &(impl GuildRepository + ?Sized),
    notifier: &(impl Notifier + ?Sized),
) -> CtfResult<ScrapeStats> {
    // Resolve notification channels once.
    let subscriptions = guild_repo.list_active_subscriptions().await?;
    let channel_ids: Vec<String> = subscriptions.iter().map(|s| s.channel_id.clone()).collect();

    let mut stats = ScrapeStats::default();
    for event in events {
        match event_repo.upsert_event(event).await {
            Ok(UpsertStatus::Inserted) => {
                stats.inserted += 1;
                info!(title = %event.title, "New event inserted — sending notification");
                if !channel_ids.is_empty() {
                    if let Err(err) = notifier.send(event, &channel_ids).await {
                        error!(title = %event.title, ?err, "Notification failed");
                        stats.errors += 1;
                    } else {
                        stats.notified += 1;
                    }
                }
            }
            Ok(UpsertStatus::Updated) => {
                stats.updated += 1;
                info!(title = %event.title, "Event updated (no re-notify)");
            }
            Ok(UpsertStatus::Unchanged) => {}
            Err(err) => {
                error!(title = %event.title, ?err, "DB upsert failed");
                stats.errors += 1;
            }
        }
    }

    // Invalidate cache if anything changed
    if (stats.inserted > 0 || stats.updated > 0)
        && let Err(err) = event_repo.invalidate_upcoming_cache().await
    {
        warn!(?err, "Failed to invalidate upcoming cache after batch");
    }

    info!(?stats, "Scrape cycle complete");
    metrics::counter!(shared::metrics::SCHEDULER_EVENTS_SCRAPED).increment(stats.inserted as u64);
    Ok(stats)
}

// ── Per-event enrichment ──────────────────────────────────────────────────────

/// Enrich a single event in-place.
///
/// Performs at most two outbound fetches:
/// 1. `ctftime.org/event/{id}` — patch data + on-page social links (always)
/// 2. The CTF's own website   — additional social links (when URL is present)
///
/// Previously the code fetched the CTFTime page *twice* per event (once in
/// `fetch_event_patch`, once inside `fetch_social_links`). This function
/// collapses that into a single CTFTime request.
async fn enrich_event(
    http: &Client,
    _llm: Option<&crate::llm::GeminiClient>,
    ev: &mut EnrichedEvent,
) {
    // ── Fetch #1: CTFTime event page ──────────────────────────────────────
    // Always fetch, regardless of whether the API fields are blank: the page
    // also carries social links that the REST API never returns.
    match html::fetch_event_patch(http, ev.raw.ctftime_id).await {
        Ok(patch) => {
            let found = patch.social_links.len();
            ev.apply_patch(patch); // fills blank fields AND merges social links
            if found > 0 {
                info!(
                    ctftime_id = ev.raw.ctftime_id,
                    count = found,
                    "Social links found on CTFTime page"
                );
            }
        }
        Err(err) => {
            metrics::counter!(shared::metrics::SCHEDULER_ENRICH_FAIL_TOTAL).increment(1);
            error!(
                ctftime_id = ev.raw.ctftime_id,
                ?err,
                "CTFTime HTML fetch failed"
            );
        }
    }

    // ── Fetch #2: CTF's own website ───────────────────────────────────────
    // Skipped when the URL is missing or deemed unsafe by the HTML module.
    if !ev.raw.url.is_empty() {
        let extra = html::fetch_external_social_links(http, &ev.raw.url).await;
        if !extra.is_empty() {
            info!(
                ctftime_id = ev.raw.ctftime_id,
                count = extra.len(),
                "Social links found on CTF website"
            );
            ev.merge_social_links(extra);
        }
    }

    let total = ev.social_links.len();
    if total == 0 {
        info!(
            ctftime_id = ev.raw.ctftime_id,
            "No social links found this cycle (existing DB links retained)"
        );
    }
}
