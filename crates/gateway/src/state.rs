//! Shared application state passed into every event handler.
//!
//! Wrapping all repos in a single `Arc<AppState>` means:
//!   • shard tasks only clone one `Arc` instead of three.
//!   • adding future state (rate-limit counters, feature flags, …) is a
//!     one-field change here, zero change at call sites.

use crate::ctftime_api::{TeamEntry, TeamSearchResult};
use moka::future::Cache;
use shared::{
    CommandLogRepository, GuildRepository, ReadCtfRepository, ReminderRepository, TeamRepository,
    WriteupRepository,
};
use std::sync::Arc;

use governor::{Quota, RateLimiter, state::keyed::DefaultKeyedStateStore};
use std::num::NonZeroU32;

#[allow(dead_code)]
pub type UserRateLimiter =
    RateLimiter<u64, DefaultKeyedStateStore<u64>, governor::clock::DefaultClock>;

pub struct AppState {
    pub guilds: Arc<dyn GuildRepository>,
    pub events: Arc<dyn ReadCtfRepository>,
    pub reminders: Arc<dyn ReminderRepository>,
    pub teams: Arc<dyn TeamRepository>,
    pub writeups: Arc<dyn WriteupRepository>,
    pub command_logs: Arc<dyn CommandLogRepository>,
    pub http_api: reqwest::Client,
    pub leaderboard_cache: Cache<i32, Vec<(u32, TeamEntry)>>,
    pub team_cache: Cache<String, Vec<TeamSearchResult>>,
    pub rate_limit_cache: Cache<u64, Arc<governor::DefaultDirectRateLimiter>>,
    pub registry: crate::commands::CommandRegistry,
    pub shard_total: u16,
    pub shard_range: (u16, u16),
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        events: Arc<dyn ReadCtfRepository>,
        guilds: Arc<dyn GuildRepository>,
        reminders: Arc<dyn ReminderRepository>,
        teams: Arc<dyn TeamRepository>,
        writeups: Arc<dyn WriteupRepository>,
        command_logs: Arc<dyn CommandLogRepository>,
        http_api: reqwest::Client,
        leaderboard_cache: Cache<i32, Vec<(u32, TeamEntry)>>,
        team_cache: Cache<String, Vec<TeamSearchResult>>,
        shard_total: u16,
        shard_range: (u16, u16),
    ) -> Self {
        let rate_limit_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_idle(std::time::Duration::from_secs(60))
            .build();

        Self {
            events,
            guilds,
            reminders,
            teams,
            writeups,
            command_logs,
            http_api,
            leaderboard_cache,
            team_cache,
            rate_limit_cache,
            registry: crate::commands::CommandRegistry::new(),
            shard_total,
            shard_range,
        }
    }

    pub async fn check_rate_limit(&self, user_id: u64) -> bool {
        let limiter = if let Some(limiter) = self.rate_limit_cache.get(&user_id).await {
            limiter
        } else {
            let limiter = Arc::new(RateLimiter::direct(
                Quota::with_period(std::time::Duration::from_secs(10))
                    .unwrap()
                    .allow_burst(NonZeroU32::new(5).unwrap()),
            ));
            self.rate_limit_cache.insert(user_id, limiter.clone()).await;
            limiter
        };

        limiter.check().is_ok()
    }
}
