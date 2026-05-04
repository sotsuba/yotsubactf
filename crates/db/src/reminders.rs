use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::{CtfResult, Reminder, ReminderAdvanceResult, ReminderRepository};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::Type, Debug, Clone, Copy)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
enum DbReminderKind {
    Event,
    Timer,
    Recurring,
}

impl From<shared::ReminderKind> for DbReminderKind {
    fn from(k: shared::ReminderKind) -> Self {
        match k {
            shared::ReminderKind::Event => Self::Event,
            shared::ReminderKind::Timer => Self::Timer,
            shared::ReminderKind::Recurring => Self::Recurring,
        }
    }
}

impl From<DbReminderKind> for shared::ReminderKind {
    fn from(k: DbReminderKind) -> Self {
        match k {
            DbReminderKind::Event => Self::Event,
            DbReminderKind::Timer => Self::Timer,
            DbReminderKind::Recurring => Self::Recurring,
        }
    }
}

pub struct PostgresReminderRepository {
    pool: PgPool,
}

impl PostgresReminderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReminderRepository for PostgresReminderRepository {
    async fn create(&self, reminder: &Reminder) -> CtfResult<Reminder> {
        let kind = DbReminderKind::from(reminder.kind);

        let row = sqlx::query!(r#"
            INSERT INTO reminders (
                user_id, kind, ctftime_id, event_title, event_start_at,
                message, remind_at, interval_secs, repeat_until, fire_count_max
            )
            SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
            WHERE $2 != 'recurring' 
               OR (SELECT COUNT(*) FROM reminders WHERE user_id = $1 AND kind = 'recurring') < 10
            RETURNING id, user_id, kind as "kind: DbReminderKind", ctftime_id, event_title, event_start_at,
                      message, remind_at, interval_secs, repeat_until, fire_count_max,
                      sent_count, last_sent_at, created_at
            "#,
            reminder.user_id,
            kind as _,
            reminder.ctftime_id,
            reminder.event_title,
            reminder.event_start_at,
            reminder.message,
            reminder.remind_at,
            reminder.interval_secs,
            reminder.repeat_until,
            reminder.fire_count_max,
        )
        .fetch_optional(&self.pool)
        .await.map_err(crate::db_err)?;

        let row = match row {
            Some(r) => r,
            None => {
                return Err(shared::CtfError::InvalidInput(
                    "You have reached the maximum limit of 10 recurring reminders.".to_string(),
                ));
            }
        };

        Ok(Reminder {
            kind: row.kind.into(),
            id: row.id,
            user_id: row.user_id,
            ctftime_id: row.ctftime_id,
            event_title: row.event_title,
            event_start_at: row.event_start_at,
            message: row.message,
            remind_at: row.remind_at,
            interval_secs: row.interval_secs,
            repeat_until: row.repeat_until,
            fire_count_max: row.fire_count_max,
            sent_count: row.sent_count,
            last_sent_at: row.last_sent_at,
            created_at: row.created_at,
        })
    }

    async fn fetch_due(&self, now: DateTime<Utc>) -> CtfResult<Vec<Reminder>> {
        let rows = sqlx::query!(r#"
            SELECT id, user_id, kind as "kind: DbReminderKind", ctftime_id, event_title, event_start_at,
                   message, remind_at, interval_secs, repeat_until, fire_count_max,
                   sent_count, last_sent_at, created_at
            FROM reminders
            WHERE remind_at <= $1
              AND remind_at >= $1 - INTERVAL '1 hour'
              AND (
                  -- One-shot: never sent
                  (kind IN ('event', 'timer') AND last_sent_at IS NULL)
                  OR
                  -- Recurring: next fire is due and series not exhausted
                  (kind = 'recurring' AND remind_at <= repeat_until)
              )
            ORDER BY remind_at ASC
            LIMIT 100
        "#, now)
        .fetch_all(&self.pool).await.map_err(crate::db_err)?;

        Ok(rows
            .into_iter()
            .map(|r| Reminder {
                kind: r.kind.into(),
                id: r.id,
                user_id: r.user_id,
                ctftime_id: r.ctftime_id,
                event_title: r.event_title,
                event_start_at: r.event_start_at,
                message: r.message,
                remind_at: r.remind_at,
                interval_secs: r.interval_secs,
                repeat_until: r.repeat_until,
                fire_count_max: r.fire_count_max,
                sent_count: r.sent_count,
                last_sent_at: r.last_sent_at,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn advance_or_delete(&self, id: Uuid) -> CtfResult<ReminderAdvanceResult> {
        let row = sqlx::query!(r#"
            WITH action AS (
                SELECT
                    id,
                    kind,
                    interval_secs,
                    remind_at,
                    repeat_until,
                    sent_count,
                    (remind_at + (
                        FLOOR(EXTRACT(EPOCH FROM (GREATEST(NOW(), remind_at) - remind_at)) / interval_secs) + 1
                    ) * (interval_secs || ' seconds')::interval) AS next_remind_at,
                    CASE
                        WHEN kind IN ('event', 'timer') THEN 'delete'
                        WHEN kind = 'recurring'
                             AND (remind_at + (
                                 FLOOR(EXTRACT(EPOCH FROM (GREATEST(NOW(), remind_at) - remind_at)) / interval_secs) + 1
                             ) * (interval_secs || ' seconds')::interval) <= repeat_until
                             THEN 'advance'
                        ELSE 'delete'
                    END AS op
                FROM reminders
                WHERE id = $1
            ),
            updated AS (
                UPDATE reminders r
                SET
                    remind_at    = CASE WHEN a.op = 'advance'
                                        THEN a.next_remind_at
                                        ELSE r.remind_at END,
                    sent_count   = r.sent_count + 1,
                    last_sent_at = NOW()
                FROM action a
                WHERE r.id = a.id AND a.op = 'advance'
                RETURNING r.id, r.remind_at, r.sent_count
            ),
            deleted AS (
                DELETE FROM reminders r
                USING action a
                WHERE r.id = a.id AND a.op = 'delete'
                RETURNING r.id
            )
            SELECT
                COALESCE(u.id, d.id)          AS id,
                CASE WHEN u.id IS NOT NULL THEN 'advanced' ELSE 'deleted' END AS result,
                u.remind_at                   AS next_remind_at,
                u.sent_count                  AS sent_count
            FROM action
            LEFT JOIN updated u ON u.id = action.id
            LEFT JOIN deleted d ON d.id = action.id
        "#, id)
        .fetch_one(&self.pool).await.map_err(crate::db_err)?;

        Ok(match row.result.as_deref() {
            Some("advanced") => ReminderAdvanceResult::Advanced {
                next_remind_at: row.next_remind_at.unwrap(),
                sent_count: row.sent_count.unwrap(),
            },
            _ => ReminderAdvanceResult::Deleted,
        })
    }

    async fn list_pending(
        &self,
        user_id: &str,
        after_remind_at: Option<DateTime<Utc>>,
    ) -> CtfResult<Vec<Reminder>> {
        let rows = sqlx::query!(r#"
            SELECT id, user_id, kind as "kind: DbReminderKind", ctftime_id, event_title, event_start_at,
                   message, remind_at, interval_secs, repeat_until, fire_count_max,
                   sent_count, last_sent_at, created_at
            FROM reminders
            WHERE user_id = $1
              AND (
                  -- One-shot: not sent AND not stale (older than 1h missed window)
                  (kind IN ('event', 'timer') 
                    AND last_sent_at IS NULL 
                    AND remind_at >= NOW() - INTERVAL '1 hour')
                  OR
                  -- Recurring: next fire is due or in future, and series not exhausted
                  (kind = 'recurring' AND remind_at <= repeat_until)
              )
              AND ($2::timestamptz IS NULL OR remind_at > $2)
            ORDER BY remind_at ASC
            LIMIT 11
        "#, user_id, after_remind_at)
        .fetch_all(&self.pool).await.map_err(crate::db_err)?;

        Ok(rows
            .into_iter()
            .map(|r| Reminder {
                kind: r.kind.into(),
                id: r.id,
                user_id: r.user_id,
                ctftime_id: r.ctftime_id,
                event_title: r.event_title,
                event_start_at: r.event_start_at,
                message: r.message,
                remind_at: r.remind_at,
                interval_secs: r.interval_secs,
                repeat_until: r.repeat_until,
                fire_count_max: r.fire_count_max,
                sent_count: r.sent_count,
                last_sent_at: r.last_sent_at,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn count_active_recurring(&self, user_id: &str) -> CtfResult<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)
            FROM reminders
            WHERE user_id = $1
              AND kind = 'recurring'
              AND remind_at <= repeat_until
            "#,
            user_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(count.unwrap_or(0))
    }

    async fn cancel(&self, id: Uuid, user_id: &str) -> CtfResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM reminders WHERE id = $1 AND user_id = $2",
            id,
            user_id
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(result.rows_affected() > 0)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
