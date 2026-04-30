use anyhow::{Context, Result};
use ctftime_core::CtfEvent;
use db::repository::CtfRepository;
use reqwest::Client;
use scraper::{api, html, rss};
use sqlx::PgPool;
use std::collections::HashMap;
use std::env;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let discord_token = env::var("DISCORD_TOKEN").context("DISCORD_TOKEN is not set")?;
    let discord_channel_id =
        env::var("DISCORD_CHANNEL_ID").context("DISCORD_CHANNEL_ID is not set")?;
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;

    let pool = PgPool::connect(&database_url)
        .await
        .context("Failed to connect to Postgres")?;
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("Failed to run migrations")?;

    let repo = CtfRepository::new(pool);
    let client = Client::new();

    discord::send_message(
        &client,
        &discord_token,
        &discord_channel_id,
        "CTF bot started. Running initial fetch.",
    )
    .await
    .context("Failed to send startup message")?;

    run_scraper_worker(&repo, &client, &discord_token, &discord_channel_id).await
}

pub async fn run_scraper_worker(
    db_repo: &CtfRepository,
    client: &Client,
    discord_token: &str,
    discord_channel_id: &str,
) -> Result<()> {
    let delay_ms = env::var("SCRAPER_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(500);

    let mut events_by_id: HashMap<i64, scraper::models::RawCtftimeEvent> = HashMap::new();
    let api_events = api::fetch_upcoming(client).await?;
    for event in api_events {
        events_by_id.insert(event.ctftime_id, event);
    }

    let rss_events = rss::fetch_upcoming(client).await?;
    for event in rss_events {
        if let Some(existing) = events_by_id.get_mut(&event.ctftime_id) {
            existing.merge_missing(event);
        } else {
            events_by_id.insert(event.ctftime_id, event);
        }
    }

    let mut raw_events: Vec<scraper::models::RawCtftimeEvent> =
        events_by_id.into_values().collect();
    raw_events.sort_by_key(|event| event.start.clone());

    for raw in raw_events {
        let mut enriched = raw;

        if enriched.ctftime_id > 0 {
            match html::fetch_event_patch(client, enriched.ctftime_id).await {
                Ok(patch) => enriched.apply_patch(patch),
                Err(e) => warn!("HTML enrichment failed for {}: {e:?}", enriched.ctftime_id),
            }

            if delay_ms > 0 {
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }

        let core_event: CtfEvent = match enriched.try_into() {
            Ok(event) => event,
            Err(e) => {
                warn!("Failed to parse event: {e:?}");
                continue;
            }
        };

        match db_repo.upsert_event(&core_event).await {
            Ok(is_new_or_updated) => {
                if is_new_or_updated {
                    let message = format!(
                        "New or updated CTF: {}\n{}\n{} -> {}",
                        core_event.title,
                        core_event.url,
                        core_event.start_time,
                        core_event.end_time
                    );

                    if let Err(e) =
                        discord::send_message(client, discord_token, discord_channel_id, &message)
                            .await
                    {
                        warn!("Discord send failed for {}: {e:?}", core_event.ctftime_id);
                    } else {
                        info!("Sent Discord notification for {}", core_event.ctftime_id);
                    }
                }
            }
            Err(e) => warn!("DB error on event {}: {e:?}", core_event.ctftime_id),
        }
    }

    Ok(())
}