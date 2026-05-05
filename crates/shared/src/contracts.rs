//! Stable contracts shared between `scheduler` and `bot`.
//!
//! Nothing here has a concrete implementation. Implementations live in
//! the `scheduler` crate (store/, notify/) and are never imported by `bot`.
//! Both binaries depend only on `shared`; neither depends on the other.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{
    CtfEvent, PaginatedEvents, Reminder, TeamResult, TrackedTeam, UpsertStatus, Writeup,
    WriteupSearchResult,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UpcomingFilter {
    /// Case-insensitive match on the `format` column, e.g. `"Jeopardy"`.
    pub format: Option<String>,
    /// Only return events with `weight >= min_weight`.
    pub min_weight: Option<f64>,
    /// Only return events with `weight <= max_weight`.
    pub max_weight: Option<f64>,
    pub onsite: Option<bool>,
    pub sort_by: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CompletedFilter {
    pub format: Option<String>,
    pub min_weight: Option<f64>,
}

// ── Subscription domain type ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: Uuid,
    pub guild_id: String,
    pub channel_id: String,
}

// ── Repository traits ─────────────────────────────────────────────────────────

/// Read-only access to CTF events.
///
/// Implemented by the gateway and any context that only needs to query events.
/// Does **not** include `upsert_event` — gateways are read-only consumers.
#[async_trait]
pub trait ReadCtfRepository: Send + Sync {
    /// Return the next `limit` upcoming events starting at `offset`,
    /// ordered by `start_time ASC`.
    async fn list_upcoming(
        &self,
        limit: i64,
        offset: i64,
        filter: &UpcomingFilter,
    ) -> crate::error::CtfResult<PaginatedEvents>;

    /// Fetch a single event by its CTFTime ID. Returns `None` if not found.
    async fn get_by_ctftime_id(&self, id: i64) -> crate::error::CtfResult<Option<CtfEvent>>;

    /// Return CTFs currently in progress at the moment of the call.
    /// Condition: NOW() is within [start_time, end_time].
    /// Ordered by end_time ASC (soonest to end first).
    async fn list_current(
        &self,
        limit: i64,
        offset: i64,
    ) -> crate::error::CtfResult<PaginatedEvents>;

    /// Fuzzy case-insensitive title search among events that haven't ended yet (end_time > NOW()).
    /// Prioritises the event whose start_time is closest to NOW().
    /// Returns None if nothing matches.
    async fn get_by_title_fuzzy(&self, query: &str) -> crate::error::CtfResult<Option<CtfEvent>>;

    /// Return events that ended between `start` and `end`.
    async fn list_recently_ended(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> crate::error::CtfResult<Vec<CtfEvent>>;

    async fn list_completed(
        &self,
        limit: i64,
        offset: i64,
        filter: &CompletedFilter,
    ) -> crate::error::CtfResult<PaginatedEvents> {
        let _ = (limit, offset, filter);
        unimplemented!()
    }

    /// Fuzzy case-insensitive title search among ALL events (no time filter).
    async fn get_all_by_title_fuzzy(
        &self,
        query: &str,
    ) -> crate::error::CtfResult<Option<CtfEvent>>;

    /// Fuzzy case-insensitive title search among ALL events with a similarity score.
    /// Returns (event, similarity_score).
    async fn get_all_by_title_fuzzy_with_score(
        &self,
        query: &str,
        min_similarity: f32,
    ) -> crate::error::CtfResult<Option<(CtfEvent, f32)>>;

    fn as_any(&self) -> &dyn std::any::Any;
}

/// Full read-write access to CTF events. Used exclusively by the scheduler.
///
/// Extends [`ReadCtfRepository`] with the write path (`upsert_event`).
#[async_trait]
pub trait WriteCtfRepository: ReadCtfRepository {
    /// Upsert a CTF event.
    ///
    /// Returns [`UpsertStatus::Inserted`] for a brand-new row,
    /// [`UpsertStatus::Updated`] when an existing row changed, and
    /// [`UpsertStatus::Unchanged`] when nothing differed.
    async fn upsert_event(&self, event: &CtfEvent) -> crate::error::CtfResult<UpsertStatus>;

    /// Invalidate all cached upcoming event lists.
    async fn invalidate_upcoming_cache(&self) -> crate::error::CtfResult<()>;
}

/// Convenience alias: the full read-write trait used by the scheduler pipeline.
///
/// Keeping this alias means existing scheduler code (`impl CtfEventRepository`)
/// compiles without changes.
pub trait CtfEventRepository: WriteCtfRepository {}
impl<T: WriteCtfRepository + ?Sized> CtfEventRepository for T {}

#[async_trait]
pub trait GuildRepository: Send + Sync {
    /// Ensure a guild record exists (idempotent).
    async fn upsert_guild(&self, guild_id: &str) -> crate::error::CtfResult<()>;

    /// Subscribe a guild to a channel (soft-replaces any prior subscription).
    async fn subscribe(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> crate::error::CtfResult<Subscription>;
    /// Soft-delete the active subscription for a guild.
    /// Returns `true` if a subscription was found and deleted.
    async fn unsubscribe(&self, guild_id: &str) -> crate::error::CtfResult<bool>;

    /// Return the active subscription for a guild, if any.
    async fn get_active_subscription(
        &self,
        guild_id: &str,
    ) -> crate::error::CtfResult<Option<Subscription>>;

    /// Return all active subscriptions across every guild.
    async fn list_active_subscriptions(&self) -> crate::error::CtfResult<Vec<Subscription>>;

    /// Update the digest configuration for a guild.
    async fn set_digest(
        &self,
        guild_id: &str,
        enabled: bool,
        channel_id: Option<&str>,
        day_utc: i16,
    ) -> crate::error::CtfResult<()>;

    /// Return the digest configuration for a guild.
    async fn get_digest(
        &self,
        guild_id: &str,
    ) -> crate::error::CtfResult<Option<crate::models::DigestConfig>>;

    /// Return a list of all guilds that have enabled digest delivery for the given day of the week,
    /// and have NOT received a digest today.
    async fn list_digest_guilds_for_day(
        &self,
        day_of_week: i16,
    ) -> crate::error::CtfResult<Vec<crate::models::DigestTarget>>;

    /// Mark that a digest was sent to a guild today.
    async fn mark_digest_sent(&self, guild_id: &str) -> crate::error::CtfResult<()>;

    /// Return all guild IDs tracking a given event (based on teams they follow).
    async fn list_guilds_tracking_event(
        &self,
        event_id: i64,
    ) -> crate::error::CtfResult<Vec<Subscription>>;

    /// Set whether a guild wants writeup notifications.
    /// Set whether a guild wants writeup notifications.
    async fn set_writeup_notify(
        &self,
        guild_id: &str,
        enabled: bool,
    ) -> crate::error::CtfResult<()>;

    /// Return all active subscriptions for guilds that have writeup notifications enabled.
    async fn list_writeup_opt_in_guilds(&self) -> crate::error::CtfResult<Vec<Subscription>>;

    fn as_any(&self) -> &dyn std::any::Any;

    /// Check if the repository is healthy (e.g. database connection is alive).
    async fn check_health(&self) -> bool;
}

// ── Reminder repository ───────────────────────────────────────────────────────

pub enum ReminderAdvanceResult {
    /// One-shot deleted, or recurring exhausted and deleted.
    Deleted,
    /// Recurring advanced — contains updated remind_at and new sent_count.
    Advanced {
        next_remind_at: DateTime<Utc>,
        sent_count: i32,
    },
}

#[async_trait]
pub trait ReminderRepository: Send + Sync {
    /// Insert a new reminder. Returns the outcome (Created, AlreadyExists, QuotaExceeded).
    async fn create(
        &self,
        reminder: &Reminder,
    ) -> crate::error::CtfResult<crate::models::CreateReminderOutcome>;

    /// All reminders due at or before `now`.
    async fn fetch_due(&self, now: DateTime<Utc>) -> crate::error::CtfResult<Vec<Reminder>>;

    /// Called after a successful DM send.
    /// - One-shot (event/timer): DELETE.
    /// - Recurring, next fire exists: UPDATE remind_at += interval, sent_count += 1.
    /// - Recurring, exhausted: DELETE.
    /// Atomic — single query, no race condition.
    async fn advance_or_delete(&self, id: Uuid) -> crate::error::CtfResult<ReminderAdvanceResult>;

    /// Pending reminders for a user, ordered by remind_at ASC, limit 10.
    /// Supports cursor-based pagination via `after_remind_at`.
    async fn list_pending(
        &self,
        user_id: &str,
        after_remind_at: Option<DateTime<Utc>>,
    ) -> crate::error::CtfResult<Vec<Reminder>>;

    /// Count active recurring reminders for a user (for cap enforcement).
    async fn count_active_recurring(&self, user_id: &str) -> crate::error::CtfResult<i64>;

    /// Cancel by UUID, scoped to user_id. Returns false if not found.
    async fn cancel(&self, id: Uuid, user_id: &str) -> crate::error::CtfResult<bool>;

    fn as_any(&self) -> &dyn std::any::Any;
}

// ── Notifier trait ────────────────────────────────────────────────────────────

/// A delivery target that posts a CTF event notification to a set of channels.
///
/// The pipeline resolves channel IDs from [`GuildRepository`] and passes them
/// in. Implementations only format and send — no DB knowledge required.
#[async_trait]
pub trait Notifier: Send + Sync {
    /// Send a notification for a new or updated CTF event to the given channels.
    async fn send(&self, event: &CtfEvent, channel_ids: &[String]) -> crate::error::CtfResult<()>;

    /// Send a notification for a team's result to the given channels.
    async fn send_result(
        &self,
        result: &TeamResult,
        event_title: &str,
        team_name: &str,
        channel_ids: &[String],
    ) -> crate::error::CtfResult<()>;

    /// Send a notification for a new writeup to the given channels.
    async fn send_writeup(
        &self,
        writeup: &Writeup,
        channel_ids: &[String],
    ) -> crate::error::CtfResult<()>;

    /// Send DM reminders for every due reminder.
    async fn send_due_reminders(&self, due: &[Reminder]) -> crate::error::CtfResult<()>;

    /// Send a single reminder DM.
    async fn send_reminder_dm(&self, reminder: &Reminder) -> crate::error::CtfResult<()>;

    /// Gửi digest embed đến một channel cụ thể.
    async fn send_digest(
        &self,
        channel_id: &str,
        embed: serde_json::Value,
    ) -> crate::error::CtfResult<()>;

    /// Support downcasting for testing.
    fn as_any(&self) -> &dyn std::any::Any;
}

// ── Team repository ───────────────────────────────────────────────────────────

#[async_trait]
pub trait TeamRepository: Send + Sync {
    /// Follow a team for a guild (upserts on conflict).
    async fn follow_team(
        &self,
        guild_id: &str,
        team_id: i64,
        team_name: &str,
    ) -> crate::error::CtfResult<()>;
    /// Unfollow the team tracked by a guild. Returns true if one was removed.
    async fn unfollow_team(&self, guild_id: &str) -> crate::error::CtfResult<bool>;
    /// Get the team currently being tracked by a guild.
    async fn get_followed_team(
        &self,
        guild_id: &str,
    ) -> crate::error::CtfResult<Option<TrackedTeam>>;
    /// Upsert a CTF result for a team. Returns true if it is a new/unnotified row.
    async fn upsert_result(&self, result: &TeamResult) -> crate::error::CtfResult<bool>;
    /// Mark a result as having been notified to Discord.
    async fn mark_result_notified(&self, id: uuid::Uuid) -> crate::error::CtfResult<()>;
    /// Return the N most recent results for a team.
    async fn list_recent_results(
        &self,
        team_id: i64,
        limit: i64,
    ) -> crate::error::CtfResult<Vec<TeamResult>>;
    /// Return all guild IDs tracking a given team.
    async fn list_guilds_tracking_team(&self, team_id: i64)
    -> crate::error::CtfResult<Vec<String>>;
    /// Return all unnotified results along with the guilds that need notification.
    async fn list_unnotified_results(
        &self,
    ) -> crate::error::CtfResult<Vec<(TeamResult, Vec<String>)>>;

    /// Return all unique CTFTime team IDs that are currently being tracked by at least one guild.
    async fn list_all_tracked_team_ids(&self) -> crate::error::CtfResult<Vec<i64>>;

    fn as_any(&self) -> &dyn std::any::Any;
}

/// Repository for CTF writeups.
#[async_trait]
pub trait WriteupRepository: Send + Sync {
    /// Save a new writeup. Returns true if it was a new record.
    async fn upsert_writeup(&self, writeup: &Writeup) -> crate::error::CtfResult<bool>;
    /// Return writeups for a specific event.
    async fn list_by_event(&self, event_id: i64) -> crate::error::CtfResult<Vec<Writeup>>;
    /// Return all unnotified writeups.
    async fn list_unnotified_writeups(&self) -> crate::error::CtfResult<Vec<Writeup>>;
    /// Mark a writeup as notified.
    async fn mark_writeup_notified(&self, id: uuid::Uuid) -> crate::error::CtfResult<()>;

    /// Search writeups by query and optional category.
    async fn search_writeups(
        &self,
        query: &str,
        category: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> crate::error::CtfResult<Vec<WriteupSearchResult>>;
    /// Return the most recent writeups.
    async fn list_recent(&self, limit: i64, offset: i64) -> crate::error::CtfResult<Vec<Writeup>>;
    /// Return writeups by partial event name match.
    async fn list_by_event_name(
        &self,
        name: &str,
        limit: i64,
        offset: i64,
    ) -> crate::error::CtfResult<Vec<Writeup>>;

    /// Return writeups for a list of event IDs.
    async fn list_by_event_ids(
        &self,
        event_ids: &[i64],
        limit: i64,
        offset: i64,
    ) -> crate::error::CtfResult<Vec<Writeup>>;
    /// Return unique event names matching a prefix for autocomplete.
    async fn autocomplete_event_name(
        &self,
        prefix: &str,
        limit: i64,
    ) -> crate::error::CtfResult<Vec<String>>;
    /// Return top writeups since a given date, distinct by category.
    async fn list_top_writeups_since(
        &self,
        since: DateTime<Utc>,
        limit: i64,
    ) -> crate::error::CtfResult<Vec<Writeup>>;

    fn as_any(&self) -> &dyn std::any::Any;
}

#[async_trait]
pub trait CommandLogRepository: Send + Sync {
    /// Persist a command execution record.
    async fn log_command(
        &self,
        user_id: &str,
        guild_id: Option<&str>,
        command_name: &str,
        kind: &str,
        success: bool,
        latency_ms: i64,
    ) -> crate::error::CtfResult<()>;

    fn as_any(&self) -> &dyn std::any::Any;
}
