use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::CtfResult as Result;
use sqlx::PgPool;
use uuid::Uuid;

use shared::{GuildRepository, Subscription};

#[derive(Debug, sqlx::FromRow)]
struct DbSubscription {
    id: Uuid,
    guild_id: String,
    channel_id: String,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
    #[allow(dead_code)]
    deleted_at: Option<DateTime<Utc>>,
}

impl From<DbSubscription> for Subscription {
    fn from(row: DbSubscription) -> Self {
        Self {
            id: row.id,
            guild_id: row.guild_id,
            channel_id: row.channel_id,
        }
    }
}

pub struct PostgresGuildRepository {
    pub pool: PgPool,
}

impl PostgresGuildRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GuildRepository for PostgresGuildRepository {
    async fn upsert_guild(&self, guild_id: &str) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO guilds (guild_id) VALUES ($1)
               ON CONFLICT (guild_id) DO UPDATE SET updated_at = NOW()"#,
            guild_id
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }

    async fn subscribe(&self, guild_id: &str, channel_id: &str) -> Result<Subscription> {
        // Everything in a single transaction so no window exists where the guild
        // row is missing or two concurrent /subscribe calls create duplicate rows.
        let mut tx = self.pool.begin().await.map_err(crate::db_err)?;

        // ── 1. Ensure the guild record exists ──────────────────────────────────
        sqlx::query!(
            r#"INSERT INTO guilds (guild_id) VALUES ($1)
               ON CONFLICT (guild_id) DO UPDATE SET updated_at = NOW()"#,
            guild_id
        )
        .execute(&mut *tx)
        .await
        .map_err(crate::db_err)?;

        // ── 2. Upsert the subscription atomically ──────────────────────────────
        //
        // `subscriptions_one_active_per_guild` is a partial unique index on
        // `(guild_id) WHERE deleted_at IS NULL`, so PostgreSQL can resolve the
        // conflict in a single statement.  If an active row already exists we
        // just update its channel_id in place — no separate soft-delete needed,
        // no second INSERT, and no gap between the two operations.
        let row = sqlx::query_as::<_, DbSubscription>(
            r#"
            INSERT INTO subscriptions (guild_id, channel_id)
            VALUES ($1, $2)
            ON CONFLICT (guild_id) WHERE deleted_at IS NULL
            DO UPDATE SET channel_id = EXCLUDED.channel_id
            RETURNING id, guild_id, channel_id, created_at, deleted_at
            "#,
        )
        .bind(guild_id)
        .bind(channel_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::db_err)?;

        tx.commit().await.map_err(crate::db_err)?;
        Ok(Subscription::from(row))
    }

    async fn unsubscribe(&self, guild_id: &str) -> Result<bool> {
        let result = sqlx::query!(
            r#"UPDATE subscriptions SET deleted_at = NOW()
               WHERE guild_id = $1 AND deleted_at IS NULL"#,
            guild_id
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_active_subscription(&self, guild_id: &str) -> Result<Option<Subscription>> {
        let row = sqlx::query_as::<_, DbSubscription>(
            r#"SELECT id, guild_id, channel_id, created_at, deleted_at
               FROM subscriptions WHERE guild_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(guild_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(row.map(Subscription::from))
    }

    async fn list_active_subscriptions(&self) -> Result<Vec<Subscription>> {
        let rows = sqlx::query_as::<_, DbSubscription>(
            r#"SELECT id, guild_id, channel_id, created_at, deleted_at
               FROM subscriptions WHERE deleted_at IS NULL ORDER BY created_at ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(rows.into_iter().map(Subscription::from).collect())
    }

    async fn set_digest(
        &self,
        guild_id: &str,
        enabled: bool,
        channel_id: Option<&str>,
        day_utc: i16,
    ) -> Result<()> {
        // We do not auto-create a subscription for digest if one doesn't exist,
        // because the user must use /subscribe first to register the guild.
        // Actually, if we want /digest enable to work without /subscribe, we should probably upsert the subscription row.
        // But for now, we just update. The command handler will handle returning an error if not subscribed.
        sqlx::query!(
            r#"UPDATE subscriptions SET digest_enabled = $1, digest_channel_id = $2, digest_day_utc = $3 WHERE guild_id = $4 AND deleted_at IS NULL"#,
            enabled,
            channel_id,
            day_utc,
            guild_id
        )
        .execute(&self.pool)
        .await.map_err(crate::db_err)?;
        Ok(())
    }

    async fn get_digest(&self, guild_id: &str) -> Result<Option<shared::models::DigestConfig>> {
        let row = sqlx::query!(
            r#"SELECT digest_enabled, digest_channel_id, digest_day_utc FROM subscriptions WHERE guild_id = $1 AND deleted_at IS NULL"#,
            guild_id
        )
        .fetch_optional(&self.pool)
        .await.map_err(crate::db_err)?;

        Ok(row.map(|r| shared::models::DigestConfig {
            enabled: r.digest_enabled,
            channel_id: r.digest_channel_id,
            day_utc: r.digest_day_utc,
        }))
    }

    async fn list_digest_guilds_for_day(
        &self,
        day_of_week: i16,
    ) -> Result<Vec<shared::models::DigestTarget>> {
        use sqlx::Row;
        let rows = sqlx::query(
            r#"
            SELECT guild_id, digest_channel_id 
            FROM subscriptions 
            WHERE deleted_at IS NULL 
              AND digest_enabled = true 
              AND digest_day_utc = $1 
              AND digest_channel_id IS NOT NULL
              AND (last_digest_sent_at IS NULL OR last_digest_sent_at < CURRENT_DATE)
            "#,
        )
        .bind(day_of_week)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        let mut targets = Vec::with_capacity(rows.len());
        for row in rows {
            targets.push(shared::models::DigestTarget {
                guild_id: row.try_get("guild_id").map_err(crate::db_err)?,
                channel_id: row
                    .try_get::<String, _>("digest_channel_id")
                    .map_err(crate::db_err)?,
            });
        }
        Ok(targets)
    }

    async fn mark_digest_sent(&self, guild_id: &str) -> Result<()> {
        sqlx::query(
            r#"UPDATE subscriptions SET last_digest_sent_at = CURRENT_DATE 
               FROM guilds 
               WHERE subscriptions.guild_id = guilds.guild_id 
                 AND guilds.guild_id = $1 
                 AND subscriptions.deleted_at IS NULL"#,
        )
        .bind(guild_id)
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }

    async fn list_guilds_tracking_event(&self, event_id: i64) -> Result<Vec<Subscription>> {
        let rows = sqlx::query_as::<_, DbSubscription>(
            r#"
            SELECT DISTINCT s.id, s.guild_id, s.channel_id, s.created_at, s.deleted_at
            FROM subscriptions s
            JOIN guilds g ON s.guild_id = g.guild_id
            JOIN tracked_teams tt ON s.guild_id = tt.guild_id
            JOIN team_results tr ON tt.ctftime_team_id = tr.ctftime_team_id
            WHERE s.deleted_at IS NULL
              AND g.notify_writeups = true
              AND tr.ctf_event_id = $1
            "#,
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows.into_iter().map(Subscription::from).collect())
    }

    async fn set_writeup_notify(&self, guild_id: &str, enabled: bool) -> Result<()> {
        sqlx::query!(
            "UPDATE guilds SET notify_writeups = $1 WHERE guild_id = $2",
            enabled,
            guild_id
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }

    async fn list_writeup_opt_in_guilds(&self) -> Result<Vec<Subscription>> {
        let rows = sqlx::query_as::<_, DbSubscription>(
            r#"
            SELECT s.id, s.guild_id, s.channel_id, s.created_at, s.deleted_at
            FROM subscriptions s
            JOIN guilds g ON s.guild_id = g.guild_id
            WHERE s.deleted_at IS NULL AND g.notify_writeups = true
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows.into_iter().map(Subscription::from).collect())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn check_health(&self) -> bool {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await.is_ok()
    }

    async fn count_subscribed_guilds(&self) -> Result<i64> {
        let row =
            sqlx::query!("SELECT COUNT(*) as count FROM subscriptions WHERE deleted_at IS NULL")
                .fetch_one(&self.pool)
                .await
                .map_err(crate::db_err)?;
        Ok(row.count.unwrap_or(0))
    }
}
