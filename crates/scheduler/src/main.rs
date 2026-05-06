//! Unified scheduler — runs all tasks in a single process.
//!
//! Spawns 5 concurrent tokio tasks, each running its own task loop.
//!
//! Environment variables
//! ─────────────────────
//!   DATABASE_URL            Postgres connection string (required)
//!   DISCORD_TOKEN           Bot token (required)
//!   REDIS_URL               Redis URL (optional)
//!   SCRAPE_INTERVAL_SECS    default 3600  (1 hour)
//!   RESULTS_INTERVAL_SECS   default 21600 (6 hours)
//!   WRITEUPS_INTERVAL_SECS  default 7200  (2 hours)
//!   DIGEST_INTERVAL_SECS    default 3600  (1 hour)
//!   REMIND_INTERVAL_SECS    default 60    (1 minute)
//!   ENRICH_INTERVAL_SECS    default 30    (30 seconds)
//!   NOTIFY_INTERVAL_SECS    default 10    (10 seconds)
//!   RUST_LOG                default "info"

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use scheduler::SharedState;
use scheduler::tasks;

fn interval_from_env(key: &str, default: u64) -> Duration {
    let secs = std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default);
    Duration::from_secs(secs)
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    // Initialize shared resources once
    let state = Arc::new(
        SharedState::from_env()
            .await
            .context("Failed to initialize SharedState")?,
    );

    // One-shot task support
    let args: Vec<String> = std::env::args().collect();
    if let Some(task_name) = args
        .iter()
        .position(|a| a == "--task")
        .and_then(|i| args.get(i + 1))
    {
        info!(task_name, "Running one-shot task");
        let task: Box<dyn tasks::SchedulerTask> = match task_name.as_str() {
            "scrape" => Box::new(tasks::scrape::ScrapeTask),
            "results" => Box::new(tasks::results::ResultsTask),
            "writeups" => Box::new(tasks::writeups::WriteupsTask),
            "digest" => Box::new(tasks::digest::DigestTask),
            "remind" => Box::new(tasks::remind::RemindTask),
            "enrich" => Box::new(tasks::enrich::EnrichTask),
            "notify" => Box::new(tasks::notify::NotifyTask),
            _ => anyhow::bail!(
                "Unknown task: {}. Available: scrape, results, writeups, digest, remind, enrich, notify",
                task_name
            ),
        };
        task.run_once(&state).await?;
        info!(task_name, "One-shot task complete");
        return Ok(());
    }

    info!("Unified Scheduler starting — spawning tasks");

    let scrape_interval = interval_from_env("SCRAPE_INTERVAL_SECS", 3600);
    let results_interval = interval_from_env("RESULTS_INTERVAL_SECS", 21600);
    let writeups_interval = interval_from_env("WRITEUPS_INTERVAL_SECS", 7200);
    let digest_interval = interval_from_env("DIGEST_INTERVAL_SECS", 3600);
    let remind_interval = interval_from_env("REMIND_INTERVAL_SECS", 60);
    let enrich_interval = interval_from_env("ENRICH_INTERVAL_SECS", 30);
    let notify_interval = interval_from_env("NOTIFY_INTERVAL_SECS", 10);

    info!(
        scrape_secs = scrape_interval.as_secs(),
        results_secs = results_interval.as_secs(),
        writeups_secs = writeups_interval.as_secs(),
        digest_secs = digest_interval.as_secs(),
        remind_secs = remind_interval.as_secs(),
        enrich_secs = enrich_interval.as_secs(),
        notify_secs = notify_interval.as_secs(),
        "Task intervals configured"
    );

    let mut set = JoinSet::new();

    set.spawn(tasks::run_task_loop(
        Arc::new(tasks::scrape::ScrapeTask),
        state.clone(),
        scrape_interval,
    ));
    set.spawn(tasks::run_task_loop(
        Arc::new(tasks::results::ResultsTask),
        state.clone(),
        results_interval,
    ));
    set.spawn(tasks::run_task_loop(
        Arc::new(tasks::writeups::WriteupsTask),
        state.clone(),
        writeups_interval,
    ));
    set.spawn(tasks::run_task_loop(
        Arc::new(tasks::digest::DigestTask),
        state.clone(),
        digest_interval,
    ));
    set.spawn(tasks::run_task_loop(
        Arc::new(tasks::remind::RemindTask),
        state.clone(),
        remind_interval,
    ));
    set.spawn(tasks::run_task_loop(
        Arc::new(tasks::enrich::EnrichTask),
        state.clone(),
        enrich_interval,
    ));
    set.spawn(tasks::run_task_loop(
        Arc::new(tasks::notify::NotifyTask),
        state.clone(),
        notify_interval,
    ));

    // Wait for shutdown or task panic
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("SIGINT received, shutting down gracefully");
        }
        _ = async {
            #[cfg(unix)]
            {
                let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
                term.recv().await;
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c().await.unwrap();
            }
        } => {
            info!("SIGTERM received, shutting down gracefully");
        }
        _ = async {
            let health_port = std::env::var("HEALTH_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8081);
            scheduler::server::run_server(state.clone(), health_port).await;
        } => {
            error!("Health server exited unexpectedly");
        }
        res = set.join_next() => {
            match res {
                Some(Err(e)) => error!(?e, "A scheduler task panicked"),
                Some(Ok(_))  => error!("A scheduler task exited unexpectedly"),
                None         => {}
            }
        }
    }

    info!("Scheduler stopped");
    Ok(())
}
