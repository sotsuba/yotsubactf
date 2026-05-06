pub mod ctftime;
pub mod llm;
pub mod notify;
pub mod pipeline;
#[cfg(test)]
mod pipeline_tests;
pub mod server;
pub mod tasks;

use anyhow::{Context, Result};
use db::PgPool;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use crate::notify::discord::DiscordNotifier;
use db::{
    PostgresCtfRepository, PostgresGuildRepository, PostgresReminderRepository,
    PostgresTeamRepository, PostgresWriteupRepository,
};
use shared::{
    GuildRepository, Notifier, ReminderRepository, TeamRepository, WriteCtfRepository,
    WriteupRepository,
};
use tracing::warn;

pub struct SharedState {
    pub pool: PgPool,
    pub http: reqwest::Client,
    pub llm: Option<llm::GeminiClient>,
    pub notifier: Arc<dyn Notifier>,
    pub event_repo: Arc<dyn WriteCtfRepository>,
    pub guild_repo: Arc<dyn GuildRepository>,
    pub team_repo: Arc<dyn TeamRepository>,
    pub writeup_repo: Arc<dyn WriteupRepository>,
    pub reminder_repo: Arc<dyn ReminderRepository>,
    pub discord_token: String,
    pub discord_api_base: String,
}

impl SharedState {
    pub async fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL not set")?;
        let discord_token = env::var("DISCORD_TOKEN").context("DISCORD_TOKEN not set")?;
        let discord_api_base = env::var("DISCORD_CHANNEL_API")
            .unwrap_or_else(|_| "https://discord.com/api/v10".to_string());
        let bot_email =
            env::var("BOT_CONTACT_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
        let gemini_enabled = env::var("GEMINI_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);
        let gemini_api_key = env::var("GEMINI_API_KEY").ok();
        let gemini_model =
            env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());
        let gemini_timeout_secs = env::var("GEMINI_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        let gemini_digest_timeout_secs = env::var("GEMINI_DIGEST_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(6);
        let gemini_gated_min_interval_ms = env::var("GEMINI_GATED_MIN_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000);
        let gemini_gated_max_interval_ms = env::var("GEMINI_GATED_MAX_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20000);

        let pool = db::connect_and_migrate(&database_url)
            .await
            .context("Failed to connect and migrate Postgres")?;

        let redis_url = env::var("REDIS_URL").ok();
        let redis_client = redis_url.and_then(|url| db::redis::Client::open(url).ok());

        let http = reqwest::Client::builder()
            .user_agent(shared::build_user_agent(&bot_email))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        let notifier = Arc::new(DiscordNotifier::new(
            http.clone(),
            discord_token.clone(),
            discord_api_base.clone(),
        ));

        let llm = if gemini_enabled {
            gemini_api_key
                .filter(|key| !key.trim().is_empty())
                .map(|key| {
                    llm::GeminiClient::new(
                        http.clone(),
                        key,
                        gemini_model,
                        Duration::from_secs(gemini_timeout_secs),
                        Duration::from_secs(gemini_digest_timeout_secs),
                        Duration::from_millis(gemini_gated_min_interval_ms),
                        Duration::from_millis(gemini_gated_max_interval_ms),
                    )
                })
        } else {
            None
        };

        if llm.is_none() {
            if !gemini_enabled {
                warn!("GEMINI_ENABLED is false; LLM enrichment disabled");
            } else {
                warn!("GEMINI_API_KEY is not set; LLM enrichment disabled");
            }
        }

        Ok(Self {
            event_repo: Arc::new(PostgresCtfRepository::new(
                pool.clone(),
                redis_client.clone(),
            )),
            guild_repo: Arc::new(PostgresGuildRepository::new(pool.clone())),
            team_repo: Arc::new(PostgresTeamRepository::new(pool.clone())),
            writeup_repo: Arc::new(PostgresWriteupRepository::new(pool.clone(), redis_client)),
            reminder_repo: Arc::new(PostgresReminderRepository::new(pool.clone())),
            pool,
            http,
            llm,
            notifier,
            discord_token,
            discord_api_base,
        })
    }

    #[cfg(test)]
    pub fn new_mock() -> Self {
        use shared::testing::{
            InMemoryCtfRepository, InMemoryGuildRepository, InMemoryReminderRepository,
            InMemoryTeamRepository, InMemoryWriteupRepository, MockNotifier,
        };

        let pool = db::PgPool::connect_lazy("postgres://localhost/unused").unwrap();
        let http = reqwest::Client::new();

        Self {
            pool,
            http,
            llm: None,
            notifier: Arc::new(MockNotifier::default()),
            event_repo: Arc::new(InMemoryCtfRepository::default()),
            guild_repo: Arc::new(InMemoryGuildRepository::default()),
            team_repo: Arc::new(InMemoryTeamRepository::default()),
            writeup_repo: Arc::new(InMemoryWriteupRepository::default()),
            reminder_repo: Arc::new(InMemoryReminderRepository::default()),
            discord_token: "test".to_string(),
            discord_api_base: "test".to_string(),
        }
    }
}
