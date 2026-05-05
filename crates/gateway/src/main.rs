use anyhow::{Context, Result};
use std::env;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use twilight_gateway::{ConfigBuilder, Intents, Shard, ShardId, StreamExt};
use twilight_http::Client as HttpClient;
use twilight_model::gateway::payload::outgoing::update_presence::UpdatePresencePayload;
use twilight_model::gateway::presence::{Activity, ActivityType, Status};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, GuildMarker};

mod commands;
mod embed;
mod handler;
mod server;
mod state;

mod ctftime_api;
mod util;
use db::{
    PostgresCommandLogRepository, PostgresCtfRepository, PostgresGuildRepository,
    PostgresReminderRepository, PostgresTeamRepository, PostgresWriteupRepository,
};
use moka::future::Cache;
use shared::{
    CommandLogRepository, GuildRepository, ReadCtfRepository, ReminderRepository, TeamRepository,
    WriteupRepository,
};
use state::AppState;
use std::time::Duration;
use util::{parse_id, parse_u16_env};

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    dotenvy::dotenv().ok();

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let token = env::var("DISCORD_TOKEN").context("DISCORD_TOKEN is not set")?;
    let token = token
        .strip_prefix("Bot ")
        .unwrap_or(&token)
        .trim()
        .to_string();
    let application_id_raw =
        env::var("DISCORD_APPLICATION_ID").context("DISCORD_APPLICATION_ID is not set")?;
    let application_id =
        parse_id::<ApplicationMarker>(&application_id_raw, "DISCORD_APPLICATION_ID")?;

    let guild_id: Option<Id<GuildMarker>> = env::var("DISCORD_GUILD_ID")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| parse_id::<GuildMarker>(&v, "DISCORD_GUILD_ID"))
        .transpose()?;

    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let pool = db::connect_and_migrate(&database_url)
        .await
        .context("Failed to connect and migrate Postgres")?;

    let bot_email =
        env::var("BOT_CONTACT_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
    let http_api = reqwest::Client::builder()
        .user_agent(shared::build_user_agent(&bot_email))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let redis_url = env::var("REDIS_URL").ok();
    let redis_client = redis_url.and_then(|url| db::redis::Client::open(url).ok());

    let leaderboard_cache = Cache::builder()
        .max_capacity(100)
        .time_to_live(Duration::from_secs(3600)) // 1 hour
        .build();

    let team_cache = Cache::builder()
        .max_capacity(500)
        .time_to_live(Duration::from_secs(1800)) // 30 mins
        .build();

    let shard_total = parse_u16_env("DISCORD_SHARD_TOTAL").unwrap_or(1);
    let shard_start = parse_u16_env("DISCORD_SHARD_START").unwrap_or(0);
    let shard_end = parse_u16_env("DISCORD_SHARD_END").unwrap_or(shard_start);

    if shard_start > shard_end {
        return Err(anyhow::anyhow!(
            "DISCORD_SHARD_START ({shard_start}) must not exceed DISCORD_SHARD_END ({shard_end})"
        ));
    }
    if shard_end >= shard_total {
        return Err(anyhow::anyhow!(
            "Shard range end ({shard_end}) must be less than total ({shard_total})"
        ));
    }

    let state = Arc::new(AppState::new(
        Arc::new(PostgresCtfRepository::new(
            pool.clone(),
            redis_client.clone(),
        )) as Arc<dyn ReadCtfRepository>,
        Arc::new(PostgresGuildRepository::new(pool.clone())) as Arc<dyn GuildRepository>,
        Arc::new(PostgresReminderRepository::new(pool.clone())) as Arc<dyn ReminderRepository>,
        Arc::new(PostgresTeamRepository::new(pool.clone())) as Arc<dyn TeamRepository>,
        Arc::new(PostgresWriteupRepository::new(pool.clone(), redis_client))
            as Arc<dyn WriteupRepository>,
        Arc::new(PostgresCommandLogRepository::new(pool)) as Arc<dyn CommandLogRepository>,
        http_api,
        leaderboard_cache,
        team_cache,
        shard_total,
        (shard_start, shard_end),
    ));

    // Never log token characters — even a short prefix narrows brute-force
    // search space. Log only the application_id and guild scope instead.
    info!(%application_id, ?guild_id, "Connecting to Discord API");

    let http = Arc::new(HttpClient::new(token.clone()));
    commands::register(&http, application_id, guild_id, &state.registry).await?;

    let health_port = parse_u16_env("HEALTH_PORT").unwrap_or(8080);
    tokio::spawn(server::run_server(state.clone(), health_port));

    info!(
        shard_total,
        shard_start,
        shard_end,
        health_port,
        guild_scoped = guild_id.is_some(),
        leaderboard_cache_ttl_secs = 3600,
        team_cache_ttl_secs = 1800,
        "gateway service starting"
    );

    // Note: twilight-gateway 0.15.x does not support per-call event-type
    // filtering via a mask — that API landed in 0.16+. All events matching
    // the declared `intents` will be received; we simply ignore anything
    // that isn't InteractionCreate in the handler.
    let intents = Intents::GUILDS;

    let mut join_set = JoinSet::new();

    // Graceful shutdown: wake all shard loops on SIGTERM or Ctrl-C so they
    // send a proper close frame before the process exits. Without this,
    // Docker / k8s will SIGKILL after the stop-grace-period and Discord will
    // see an unclean disconnect.
    let shutdown = Arc::new(tokio::sync::Notify::new());
    {
        let shutdown = Arc::clone(&shutdown);
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{SignalKind, signal};
                if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                    tokio::select! {
                        _ = sigterm.recv() => {},
                        _ = tokio::signal::ctrl_c() => {},
                    }
                } else {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            info!("Shutdown signal received — stopping shards");
            shutdown.notify_waiters();
        });
    }

    for shard_id in shard_start..=shard_end {
        let token = token.clone();
        let http = Arc::clone(&http);
        let state = Arc::clone(&state);
        let shutdown = Arc::clone(&shutdown);

        join_set.spawn(async move {
            let shard_id = ShardId::new(shard_id as u32, shard_total as u32);

            let presence = UpdatePresencePayload::new(
                vec![Activity {
                    application_id: None,
                    assets: None,
                    buttons: Vec::new(),
                    created_at: None,
                    details: None,
                    emoji: None,
                    flags: None,
                    id: None,
                    instance: None,
                    kind: ActivityType::Watching,
                    name: "🍀 This is zaza!?".to_string(),
                    party: None,
                    secrets: None,
                    state: None,
                    timestamps: None,
                    url: None,
                }],
                false,
                None,
                Status::Online,
            )
            .expect("valid presence");

            let config = ConfigBuilder::new(token, intents)
                .presence(presence)
                .build();
            let mut shard = Shard::with_config(shard_id, config);

            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.notified() => {
                        info!(?shard_id, "Shard shutting down gracefully");
                        shard.close(twilight_gateway::CloseFrame::NORMAL);
                        break;
                    }
                    item = shard.next_event(twilight_gateway::EventTypeFlags::all()) => {
                        match item {
                            Some(Ok(event)) => {
                                if let twilight_gateway::Event::Ready(ref ready) = event {
                                    tracing::info!(
                                        ?shard_id,
                                        session_id = %ready.session_id,
                                        user       = %ready.user.name,
                                        "shard connected and ready"
                                    );
                                }

                                if let Err(err) = handler::handle_event(
                                    shard_id, event, &http, application_id, &state,
                                ).await {
                                    warn!(?err, ?shard_id, "Failed to handle gateway event");
                                }
                            }
                            Some(Err(err)) => {
                                error!(?err, ?shard_id, "Shard receive error");
                            }
                            None => break,
                        }
                    }
                }
            }
        });
    }

    while join_set.join_next().await.is_some() {}
    info!("All shards stopped — exiting");
    Ok(())
}
