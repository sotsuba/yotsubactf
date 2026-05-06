use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Upsert outcome ────────────────────────────────────────────────────────────

/// What happened when a CTF event was upserted.
///
/// The pipeline uses this to decide whether to fire a notification:
/// only [`UpsertStatus::Inserted`] (a brand-new event) triggers one.
/// Updates to existing events (schedule tweaks, enrichment passes) are
/// intentionally silent so users are not spammed on every scrape cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertStatus {
    /// A row was created for the first time. → Notify.
    Inserted,
    /// The row already existed and at least one field changed. → Do NOT notify.
    Updated,
    /// The row already existed and nothing changed. → Do NOT notify.
    Unchanged,
}

// ── Reminder creation outcome ─────────────────────────────────────────────────

/// Describes every possible result of a `create_reminder` call.
///
/// Using a typed enum instead of `bool` makes call sites exhaustive at
/// compile time — adding a new variant forces every handler to be updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateReminderOutcome {
    /// A new reminder row was inserted.
    Created,
    /// A reminder for this `(user_id, ctftime_id)` pair already exists —
    /// the second click is idempotent, nothing was changed.
    AlreadyExists,
    /// The user already has too many pending reminders.
    /// The bot should tell them to wait until some fire before adding more.
    QuotaExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReminderKind {
    #[default]
    Event,
    Timer,
    Recurring,
}

/// A flexible reminder record (event-linked, timer, or recurring).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: Uuid,
    pub user_id: String,
    pub kind: ReminderKind,

    // Event-linked (kind = 'event')
    pub ctftime_id: Option<i64>,
    pub event_title: Option<String>,
    pub event_start_at: Option<DateTime<Utc>>,

    // Human message (kind = 'timer' | 'recurring')
    pub message: Option<String>,

    // Scheduling
    pub remind_at: DateTime<Utc>,

    // Recurring only
    pub interval_secs: Option<i64>,
    pub repeat_until: Option<DateTime<Utc>>,
    pub fire_count_max: Option<i32>,

    // Lifecycle
    pub sent_count: i32,
    pub last_sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Default for Reminder {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            user_id: String::new(),
            kind: ReminderKind::Event,
            ctftime_id: None,
            event_title: None,
            event_start_at: None,
            message: None,
            remind_at: Utc::now(),
            interval_secs: None,
            repeat_until: None,
            fire_count_max: None,
            sent_count: 0,
            last_sent_at: None,
            created_at: Utc::now(),
        }
    }
}

impl Reminder {
    pub const STALENESS_THRESHOLD: Duration = Duration::minutes(10);

    /// Compute next remind_at after a successful send.
    /// Returns None when the recurring series is exhausted.
    pub fn next_remind_at(&self) -> Option<DateTime<Utc>> {
        let interval = chrono::Duration::seconds(self.interval_secs?);
        let until = self.repeat_until?;
        let next = self.remind_at + interval;
        (next <= until).then_some(next)
    }

    /// Whether this reminder has more fires remaining.
    pub fn is_exhausted(&self) -> bool {
        match self.kind {
            ReminderKind::Event | ReminderKind::Timer => true, // one-shot
            ReminderKind::Recurring => self.next_remind_at().is_none(),
        }
    }

    /// Fires remaining, for display in /reminder list.
    pub fn fires_remaining(&self) -> Option<i32> {
        let max = self.fire_count_max?;
        Some(max - self.sent_count)
    }

    /// Label line for /reminder list embed.
    pub fn list_label(&self) -> String {
        match self.kind {
            ReminderKind::Event => format!(
                "📅 **{}** starts <t:{}:R>\n  › fires <t:{}:R>",
                self.event_title.as_deref().unwrap_or("Unknown event"),
                self.event_start_at.map(|t| t.timestamp()).unwrap_or(0),
                self.remind_at.timestamp(),
            ),
            ReminderKind::Timer => format!(
                "⏰ {} — fires <t:{}:R>",
                self.message.as_deref().unwrap_or("*(no message)*"),
                self.remind_at.timestamp(),
            ),
            ReminderKind::Recurring => format!(
                "🔁 {} — next <t:{}:R> · {} fires left · until <t:{}:F>",
                self.message.as_deref().unwrap_or("*(no message)*"),
                self.remind_at.timestamp(),
                self.fires_remaining().unwrap_or(0),
                self.repeat_until.map(|t| t.timestamp()).unwrap_or(0),
            ),
        }
    }
}

// ── Social platform links ─────────────────────────────────────────────────────

/// A community / chat platform where a CTF team can be reached.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum SocialPlatform {
    Discord,
    Telegram,
    Slack,
    Matrix,
    Irc,
}

impl SocialPlatform {
    /// Emoji label used in Discord button labels.
    pub fn emoji_label(&self) -> &'static str {
        match self {
            SocialPlatform::Discord => "🎮 Discord",
            SocialPlatform::Telegram => "✈️ Telegram",
            SocialPlatform::Slack => "💬 Slack",
            SocialPlatform::Matrix => "🔷 Matrix",
            SocialPlatform::Irc => "📡 IRC",
        }
    }
}

/// A resolved invite / join link for a CTF's community channel.
///
/// Equality and hashing are keyed on `url` alone so that the same invite link
/// is never stored twice regardless of how the platform field was classified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLink {
    pub platform: SocialPlatform,
    pub url: String,
}

impl PartialEq for SocialLink {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl Eq for SocialLink {}

impl std::hash::Hash for SocialLink {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.url.hash(state);
    }
}

// ── CTF event (enriched, canonical) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CtfEvent {
    pub id: Option<Uuid>,
    pub ctftime_id: i64,
    pub title: String,
    pub url: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub weight: Option<f64>,
    pub format: Option<String>,
    pub organiser: Option<String>,
    pub description: Option<String>,
    /// Community / chat links extracted by the social-link enricher.
    #[serde(default)]
    pub social_links: Vec<SocialLink>,
    pub is_onsite: bool,
    pub enriched_at: Option<DateTime<Utc>>,
    pub notified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PaginatedEvents {
    pub events: Vec<CtfEvent>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Default)]
pub struct DigestConfig {
    pub enabled: bool,
    pub channel_id: Option<String>,
    pub day_utc: i16,
}

#[derive(Debug, Clone, Default)]
pub struct DigestTarget {
    pub guild_id: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct TrackedTeam {
    pub id: uuid::Uuid,
    pub guild_id: String,
    pub ctftime_team_id: i64,
    pub team_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub struct TeamResult {
    pub id: uuid::Uuid,
    pub ctftime_team_id: i64,
    pub ctf_event_id: i64,
    pub place: Option<i32>,
    pub score: Option<f64>,
    pub total_teams: Option<i32>,
    pub notified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub struct Writeup {
    pub id: uuid::Uuid,
    pub ctftime_id: i64,
    pub title: String,
    pub url: String,
    pub summary: Option<String>,
    pub event_id: i64,
    pub category: Option<String>,
    pub event_name: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub enriched_at: Option<DateTime<Utc>>,
    pub notified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct WriteupSearchResult {
    pub writeup: Writeup,
    pub rank: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CommandLog {
    pub id: Uuid,
    pub user_id: String,
    pub guild_id: Option<String>,
    pub command_name: String,
    pub kind: String, // slash, component
    pub success: bool,
    pub latency_ms: i64,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reminder_next_remind_at() {
        let now = Utc::now();
        let reminder = Reminder {
            kind: ReminderKind::Recurring,
            remind_at: now,
            interval_secs: Some(3600),
            repeat_until: Some(now + Duration::hours(10)),
            ..Default::default()
        };

        let next = reminder.next_remind_at().unwrap();
        assert_eq!(next, now + Duration::hours(1));

        // Exhausted case
        let reminder_exhausted = Reminder {
            kind: ReminderKind::Recurring,
            remind_at: now,
            interval_secs: Some(3600),
            repeat_until: Some(now + Duration::minutes(30)),
            ..Default::default()
        };
        assert!(reminder_exhausted.next_remind_at().is_none());
    }

    #[test]
    fn test_reminder_is_exhausted() {
        let now = Utc::now();
        let timer = Reminder {
            kind: ReminderKind::Timer,
            ..Default::default()
        };
        assert!(timer.is_exhausted());

        let recurring = Reminder {
            kind: ReminderKind::Recurring,
            remind_at: now,
            interval_secs: Some(3600),
            repeat_until: Some(now + Duration::hours(1)),
            ..Default::default()
        };
        assert!(!recurring.is_exhausted());

        let recurring_exhausted = Reminder {
            kind: ReminderKind::Recurring,
            remind_at: now,
            interval_secs: Some(3600),
            repeat_until: Some(now),
            ..Default::default()
        };
        assert!(recurring_exhausted.is_exhausted());
    }

    #[test]
    fn test_reminder_fires_remaining() {
        let reminder = Reminder {
            fire_count_max: Some(60),
            sent_count: 10,
            ..Default::default()
        };
        assert_eq!(reminder.fires_remaining(), Some(50));

        let reminder_no_max = Reminder {
            fire_count_max: None,
            ..Default::default()
        };
        assert!(reminder_no_max.fires_remaining().is_none());
    }
}
