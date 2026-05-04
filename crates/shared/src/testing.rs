use crate::contracts::{
    CommandLogRepository, GuildRepository, Notifier, ReadCtfRepository, ReminderAdvanceResult,
    ReminderRepository, Subscription, TeamRepository, UpcomingFilter, WriteCtfRepository,
    WriteupRepository,
};
use crate::error::{CtfError, CtfResult as Result};
use crate::models::{
    CtfEvent, DigestConfig, DigestTarget, PaginatedEvents, Reminder, ReminderKind, TeamResult,
    TrackedTeam, UpsertStatus, Writeup, WriteupSearchResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

/// A thread-safe in-memory CTF repository for unit testing.
#[derive(Default)]
pub struct InMemoryCtfRepository {
    pub events: RwLock<HashMap<i64, CtfEvent>>,
}

#[async_trait]
impl ReadCtfRepository for InMemoryCtfRepository {
    async fn list_upcoming(
        &self,
        limit: i64,
        offset: i64,
        filter: &UpcomingFilter,
    ) -> Result<PaginatedEvents> {
        let events = self.events.read().await;
        let mut list: Vec<CtfEvent> = events.values().cloned().collect();

        let now = Utc::now();
        list.retain(|e| {
            if e.end_time < now {
                return false;
            }
            if let Some(ref fmt) = filter.format
                && !e
                    .format
                    .as_deref()
                    .map(|f| f.to_lowercase().contains(&fmt.to_lowercase()))
                    .unwrap_or(false)
            {
                return false;
            }
            if let Some(w) = filter.min_weight
                && e.weight.unwrap_or(0.0) < w
            {
                return false;
            }
            if let Some(w) = filter.max_weight
                && e.weight.unwrap_or(0.0) > w
            {
                return false;
            }
            if let Some(onsite) = filter.onsite
                && e.is_onsite != onsite
            {
                return false;
            }
            true
        });

        if filter.sort_by.as_deref() == Some("weight") {
            list.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());
        } else {
            list.sort_by_key(|e| e.start_time);
        }

        let total_count = list.len() as i64;
        let events = list
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();

        Ok(PaginatedEvents {
            events,
            total_count,
        })
    }

    async fn get_by_ctftime_id(&self, id: i64) -> Result<Option<CtfEvent>> {
        let events = self.events.read().await;
        Ok(events.get(&id).cloned())
    }

    async fn list_current(&self, limit: i64, offset: i64) -> Result<PaginatedEvents> {
        let events = self.events.read().await;
        let now = Utc::now();
        let mut list: Vec<CtfEvent> = events
            .values()
            .filter(|e| now >= e.start_time && now <= e.end_time)
            .cloned()
            .collect();

        list.sort_by_key(|e| e.end_time);

        let total_count = list.len() as i64;
        let events = list
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();

        Ok(PaginatedEvents {
            events,
            total_count,
        })
    }

    async fn get_by_title_fuzzy(&self, query: &str) -> Result<Option<CtfEvent>> {
        let events = self.events.read().await;
        let now = Utc::now();
        Ok(events
            .values()
            .filter(|e| e.end_time > now && e.title.to_lowercase().contains(&query.to_lowercase()))
            .min_by_key(|e| (e.start_time - now).num_seconds().abs())
            .cloned())
    }

    async fn list_recently_ended(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CtfEvent>> {
        let events = self.events.read().await;
        let mut list: Vec<CtfEvent> = events
            .values()
            .filter(|e| e.end_time >= start && e.end_time <= end)
            .cloned()
            .collect();
        list.sort_by_key(|e| e.end_time);
        list.reverse();
        Ok(list)
    }

    async fn get_all_by_title_fuzzy(&self, query: &str) -> Result<Option<CtfEvent>> {
        let events = self.events.read().await;
        let now = Utc::now();
        Ok(events
            .values()
            .filter(|e| e.title.to_lowercase().contains(&query.to_lowercase()))
            .min_by_key(|e| (e.start_time - now).num_seconds().abs())
            .cloned())
    }

    async fn get_all_by_title_fuzzy_with_score(
        &self,
        query: &str,
        _min_similarity: f32,
    ) -> Result<Option<(CtfEvent, f32)>> {
        if let Some(event) = self.get_all_by_title_fuzzy(query).await? {
            Ok(Some((event, 1.0)))
        } else {
            Ok(None)
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl WriteCtfRepository for InMemoryCtfRepository {
    async fn upsert_event(&self, event: &CtfEvent) -> Result<UpsertStatus> {
        let mut events = self.events.write().await;
        if let Some(existing) = events.get(&event.ctftime_id) {
            if existing == event {
                Ok(UpsertStatus::Unchanged)
            } else {
                events.insert(event.ctftime_id, event.clone());
                Ok(UpsertStatus::Updated)
            }
        } else {
            events.insert(event.ctftime_id, event.clone());
            Ok(UpsertStatus::Inserted)
        }
    }

    async fn invalidate_upcoming_cache(&self) -> Result<()> {
        Ok(())
    }
}

/// A thread-safe in-memory Guild repository for unit testing.
#[derive(Default)]
pub struct InMemoryGuildRepository {
    pub guilds: RwLock<HashMap<String, ()>>,
    pub subscriptions: RwLock<HashMap<String, Subscription>>,
}

#[async_trait]
impl GuildRepository for InMemoryGuildRepository {
    async fn upsert_guild(&self, guild_id: &str) -> Result<()> {
        let mut guilds = self.guilds.write().await;
        guilds.insert(guild_id.to_string(), ());
        Ok(())
    }

    async fn subscribe(&self, guild_id: &str, channel_id: &str) -> Result<Subscription> {
        let mut subs = self.subscriptions.write().await;
        let sub = Subscription {
            id: uuid::Uuid::new_v4(),
            guild_id: guild_id.to_string(),
            channel_id: channel_id.to_string(),
        };
        subs.insert(guild_id.to_string(), sub.clone());
        Ok(sub)
    }

    async fn unsubscribe(&self, guild_id: &str) -> Result<bool> {
        let mut subs = self.subscriptions.write().await;
        Ok(subs.remove(guild_id).is_some())
    }

    async fn get_active_subscription(&self, guild_id: &str) -> Result<Option<Subscription>> {
        let subs = self.subscriptions.read().await;
        Ok(subs.get(guild_id).cloned())
    }

    async fn list_active_subscriptions(&self) -> Result<Vec<Subscription>> {
        let subs = self.subscriptions.read().await;
        Ok(subs.values().cloned().collect())
    }

    async fn set_digest(
        &self,
        _guild_id: &str,
        _enabled: bool,
        _channel_id: Option<&str>,
        _day_utc: i16,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_digest(&self, _guild_id: &str) -> Result<Option<DigestConfig>> {
        Ok(None)
    }

    async fn list_digest_guilds_for_day(&self, _day_of_week: i16) -> Result<Vec<DigestTarget>> {
        Ok(vec![])
    }

    async fn list_guilds_tracking_event(&self, _event_id: i64) -> Result<Vec<Subscription>> {
        Ok(vec![])
    }

    async fn set_writeup_notify(&self, _guild_id: &str, _enabled: bool) -> Result<()> {
        Ok(())
    }

    async fn list_writeup_opt_in_guilds(&self) -> Result<Vec<Subscription>> {
        let subs = self.subscriptions.read().await;
        Ok(subs.values().cloned().collect())
    }

    async fn mark_digest_sent(&self, _guild_id: &str) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn check_health(&self) -> bool {
        true
    }
}

/// A thread-safe in-memory reminder repository for unit testing.
#[derive(Default)]
pub struct InMemoryReminderRepository {
    pub reminders: RwLock<Vec<Reminder>>,
}

#[async_trait]
impl ReminderRepository for InMemoryReminderRepository {
    async fn create(&self, reminder: &Reminder) -> Result<Reminder> {
        let mut r = reminder.clone();
        r.id = Uuid::new_v4();
        self.reminders.write().await.push(r.clone());
        Ok(r)
    }

    async fn fetch_due(&self, now: DateTime<Utc>) -> Result<Vec<Reminder>> {
        Ok(self
            .reminders
            .read()
            .await
            .iter()
            .filter(|r| r.remind_at <= now)
            .cloned()
            .collect())
    }

    async fn list_pending(
        &self,
        user_id: &str,
        cursor: Option<DateTime<Utc>>,
    ) -> Result<Vec<Reminder>> {
        let reminders = self.reminders.read().await;
        let mut results: Vec<Reminder> = reminders
            .iter()
            .filter(|r| r.user_id == user_id && r.sent_count == 0)
            .filter(|r| cursor.is_none_or(|c| r.remind_at > c))
            .cloned()
            .collect();

        results.sort_by_key(|r| r.remind_at);
        Ok(results.into_iter().take(11).collect())
    }

    async fn count_active_recurring(&self, user_id: &str) -> Result<i64> {
        Ok(self
            .reminders
            .read()
            .await
            .iter()
            .filter(|r| r.user_id == user_id && matches!(r.kind, ReminderKind::Recurring))
            .count() as i64)
    }

    async fn advance_or_delete(&self, id: Uuid) -> Result<ReminderAdvanceResult> {
        let mut reminders = self.reminders.write().await;
        let pos = reminders
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| CtfError::NotFound(id.to_string()))?;

        let reminder = &reminders[pos];
        if let Some(next) = reminder.next_remind_at() {
            let new_count = reminder.sent_count + 1;
            reminders[pos].remind_at = next;
            reminders[pos].sent_count = new_count;
            Ok(ReminderAdvanceResult::Advanced {
                next_remind_at: next,
                sent_count: new_count,
            })
        } else {
            reminders.remove(pos);
            Ok(ReminderAdvanceResult::Deleted)
        }
    }

    async fn cancel(&self, id: Uuid, user_id: &str) -> Result<bool> {
        let mut reminders = self.reminders.write().await;
        let before = reminders.len();
        reminders.retain(|r| !(r.id == id && r.user_id == user_id));
        Ok(reminders.len() < before)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A mock notifier for unit testing.
#[derive(Default)]
pub struct MockNotifier {
    pub sent_events: RwLock<Vec<(CtfEvent, Vec<String>)>>,
    pub sent_reminders: RwLock<Vec<Reminder>>,
    pub fail_next: AtomicBool,
}

#[async_trait]
impl Notifier for MockNotifier {
    async fn send(&self, event: &CtfEvent, channel_ids: &[String]) -> Result<()> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(CtfError::ExternalApi {
                status: 503,
                message: "mock fail".into(),
            });
        }
        self.sent_events
            .write()
            .await
            .push((event.clone(), channel_ids.to_vec()));
        Ok(())
    }

    async fn send_result(
        &self,
        _result: &TeamResult,
        _event_title: &str,
        _team_name: &str,
        _channel_ids: &[String],
    ) -> Result<()> {
        Ok(())
    }
    async fn send_writeup(&self, _writeup: &Writeup, _channel_ids: &[String]) -> Result<()> {
        Ok(())
    }
    async fn send_due_reminders(&self, due: &[Reminder]) -> Result<()> {
        for r in due {
            self.send_reminder_dm(r).await?;
        }
        Ok(())
    }

    async fn send_reminder_dm(&self, reminder: &Reminder) -> Result<()> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(CtfError::ExternalApi {
                status: 503,
                message: "mock fail".into(),
            });
        }
        self.sent_reminders.write().await.push(reminder.clone());
        Ok(())
    }

    async fn send_digest(&self, _channel_id: &str, _embed: serde_json::Value) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A thread-safe in-memory Team repository for unit testing.
#[derive(Default)]
pub struct InMemoryTeamRepository {}

#[async_trait]
impl TeamRepository for InMemoryTeamRepository {
    async fn follow_team(&self, _guild_id: &str, _team_id: i64, _team_name: &str) -> Result<()> {
        Ok(())
    }
    async fn unfollow_team(&self, _guild_id: &str) -> Result<bool> {
        Ok(false)
    }
    async fn get_followed_team(&self, _guild_id: &str) -> Result<Option<TrackedTeam>> {
        Ok(None)
    }
    async fn upsert_result(&self, _result: &TeamResult) -> Result<bool> {
        Ok(true)
    }
    async fn mark_result_notified(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    async fn list_recent_results(&self, _team_id: i64, _limit: i64) -> Result<Vec<TeamResult>> {
        Ok(vec![])
    }
    async fn list_guilds_tracking_team(&self, _team_id: i64) -> Result<Vec<String>> {
        Ok(vec![])
    }
    async fn list_unnotified_results(&self) -> Result<Vec<(TeamResult, Vec<String>)>> {
        Ok(vec![])
    }
    async fn list_all_tracked_team_ids(&self) -> Result<Vec<i64>> {
        Ok(vec![])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A thread-safe in-memory Writeup repository for unit testing.
#[derive(Default)]
pub struct InMemoryWriteupRepository {}

#[async_trait]
impl WriteupRepository for InMemoryWriteupRepository {
    async fn upsert_writeup(&self, _writeup: &Writeup) -> Result<bool> {
        Ok(true)
    }
    async fn list_by_event(&self, _event_id: i64) -> Result<Vec<Writeup>> {
        Ok(vec![])
    }
    async fn list_unnotified_writeups(&self) -> Result<Vec<Writeup>> {
        Ok(vec![])
    }
    async fn mark_writeup_notified(&self, _id: uuid::Uuid) -> Result<()> {
        Ok(())
    }
    async fn search_writeups(
        &self,
        _query: &str,
        _category: Option<&str>,
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<WriteupSearchResult>> {
        Ok(vec![])
    }
    async fn list_recent(&self, _limit: i64, _offset: i64) -> Result<Vec<Writeup>> {
        Ok(vec![])
    }
    async fn list_by_event_name(
        &self,
        _name: &str,
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<Writeup>> {
        Ok(vec![])
    }
    async fn list_by_event_ids(
        &self,
        _event_ids: &[i64],
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<Writeup>> {
        Ok(vec![])
    }
    async fn autocomplete_event_name(&self, _prefix: &str, _limit: i64) -> Result<Vec<String>> {
        Ok(vec![])
    }
    async fn list_top_writeups_since(
        &self,
        _since: DateTime<Utc>,
        _limit: i64,
    ) -> Result<Vec<Writeup>> {
        Ok(vec![])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A thread-safe in-memory CommandLog repository for unit testing.
#[derive(Default)]
pub struct InMemoryCommandLogRepository {}

#[async_trait]
impl CommandLogRepository for InMemoryCommandLogRepository {
    async fn log_command(
        &self,
        _user_id: &str,
        _guild_id: Option<&str>,
        _command_name: &str,
        _kind: &str,
        _success: bool,
        _latency_ms: i64,
    ) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
