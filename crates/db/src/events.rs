use async_trait::async_trait;
use chrono::{DateTime, Utc};
use metrics;
use redis::AsyncCommands;
use serde_json::Value as JsonValue;
use shared::CtfResult as Result;
use sqlx::{PgPool, QueryBuilder};
use uuid::Uuid;

use shared::{
    CompletedFilter, CtfEvent, PaginatedEvents, ReadCtfRepository, SocialLink, UpcomingFilter,
    UpsertStatus, WriteCtfRepository,
};

#[derive(Debug, sqlx::FromRow)]
struct DbCtfEvent {
    id: Uuid,
    ctftime_id: i64,
    title: String,
    url: String,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    weight: Option<f64>,
    format: Option<String>,
    organiser: Option<String>,
    description: Option<String>,
    social_links: JsonValue,
    #[allow(dead_code)]
    updated_at: DateTime<Utc>,
    is_onsite: bool,
    enriched_at: Option<DateTime<Utc>>,
    notified_at: Option<DateTime<Utc>>,
    total_count: Option<i64>,
}

impl From<DbCtfEvent> for CtfEvent {
    fn from(row: DbCtfEvent) -> Self {
        let social_links: Vec<SocialLink> = serde_json::from_value(row.social_links).unwrap_or_else(|e| {
            tracing::warn!(ctftime_id = row.ctftime_id, error = %e, "Failed to parse social_links for event; using empty list");
            vec![]
        });
        Self {
            id: Some(row.id),
            ctftime_id: row.ctftime_id,
            title: row.title,
            url: row.url,
            start_time: row.start_time,
            end_time: row.end_time,
            weight: row.weight,
            format: row.format,
            organiser: row.organiser,
            description: row.description,
            social_links,
            is_onsite: row.is_onsite,
            enriched_at: row.enriched_at,
            notified_at: row.notified_at,
        }
    }
}

pub struct PostgresCtfRepository {
    pool: PgPool,
    redis: Option<redis::Client>,
}

impl PostgresCtfRepository {
    pub fn new(pool: PgPool, redis: Option<redis::Client>) -> Self {
        Self { pool, redis }
    }
}

#[async_trait]
impl ReadCtfRepository for PostgresCtfRepository {
    async fn list_upcoming(
        &self,
        limit: i64,
        offset: i64,
        filter: &UpcomingFilter,
    ) -> Result<PaginatedEvents> {
        if let Some(ref client) = self.redis
            && let Ok(mut conn) = client.get_multiplexed_async_connection().await
        {
            // Generate a simple hash/key for the filter.
            let filter_key = serde_json::to_string(filter).unwrap_or_default();
            let cache_key = format!("upcoming:{filter_key}:o{offset}:l{limit}");

            if let Ok(cached) = conn.get::<_, String>(&cache_key).await
                && let Ok(res) = serde_json::from_str::<PaginatedEvents>(&cached)
            {
                metrics::counter!(shared::metrics::DB_REDIS_HITS_TOTAL, "repo" => "ctf_events", "op" => "list_upcoming").increment(1);
                return Ok(res);
            }
            metrics::counter!(shared::metrics::DB_REDIS_MISSES_TOTAL, "repo" => "ctf_events", "op" => "list_upcoming").increment(1);
        }

        // Build the query dynamically using QueryBuilder so parameter numbering
        // is handled automatically — no risk of $p counter drift.
        let mut qb = QueryBuilder::new(
            "SELECT id, ctftime_id, title, url, start_time, end_time, \
             weight, format, organiser, description, \
             social_links, created_at, updated_at, is_onsite, \
             enriched_at, notified_at, \
             COUNT(*) OVER() as total_count \
             FROM ctf_events WHERE end_time >= ",
        );
        qb.push_bind(Utc::now());

        if let Some(ref fmt) = filter.format {
            qb.push(" AND format ILIKE ").push_bind(fmt.clone());
        }
        if let Some(w) = filter.min_weight {
            qb.push(" AND weight >= ").push_bind(w);
        }
        if let Some(w) = filter.max_weight {
            qb.push(" AND weight <= ").push_bind(w);
        }
        if let Some(onsite) = filter.onsite {
            qb.push(" AND is_onsite = ").push_bind(onsite);
        }

        let order_by = if filter.sort_by.as_deref() == Some("weight") {
            " ORDER BY weight DESC, start_time ASC "
        } else {
            " ORDER BY start_time ASC "
        };
        qb.push(order_by);

        qb.push(" LIMIT ").push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);

        let rows = qb
            .build_query_as::<DbCtfEvent>()
            .fetch_all(&self.pool)
            .await
            .map_err(crate::db_err)?;

        let total_count = rows.first().and_then(|r| r.total_count).unwrap_or(0);
        let events: Vec<CtfEvent> = rows.into_iter().map(CtfEvent::from).collect();
        let res = PaginatedEvents {
            events,
            total_count,
        };

        if let Some(ref client) = self.redis
            && let Ok(mut conn) = client.get_multiplexed_async_connection().await
        {
            let filter_key = serde_json::to_string(filter).unwrap_or_default();
            let cache_key = format!("upcoming:{filter_key}:o{offset}:l{limit}");
            if let Ok(serialized) = serde_json::to_string(&res) {
                let _ = conn.set_ex::<_, _, ()>(&cache_key, serialized, 300).await;
            } else {
                tracing::warn!(?res, "Failed to serialize PaginatedEvents for cache");
            }
        }

        Ok(res)
    }

    async fn get_by_ctftime_id(&self, ctftime_id: i64) -> Result<Option<CtfEvent>> {
        let row = sqlx::query_as::<_, DbCtfEvent>(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at,
                   NULL as total_count
            FROM ctf_events
            WHERE ctftime_id = $1
            "#,
        )
        .bind(ctftime_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(row.map(CtfEvent::from))
    }

    async fn list_current(&self, limit: i64, offset: i64) -> Result<PaginatedEvents> {
        let rows = sqlx::query_as::<_, DbCtfEvent>(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at,
                   COUNT(*) OVER() as total_count
            FROM ctf_events
            WHERE NOW() BETWEEN start_time AND end_time
            ORDER BY end_time ASC
            LIMIT $1
            OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        let total_count = rows.first().and_then(|r| r.total_count).unwrap_or(0);
        let events = rows.into_iter().map(CtfEvent::from).collect();
        Ok(PaginatedEvents {
            events,
            total_count,
        })
    }

    async fn get_by_title_fuzzy(&self, query: &str) -> Result<Option<CtfEvent>> {
        // If query is numeric, prioritize ctftime_id match
        if let Ok(id) = query.parse::<i64>()
            && let Some(event) = self.get_by_ctftime_id(id).await?
            && event.end_time > Utc::now()
        {
            return Ok(Some(event));
        }

        let pattern = format!("%{query}%");

        let row = sqlx::query_as::<_, DbCtfEvent>(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at,
                   NULL as total_count
            FROM ctf_events
            WHERE title ILIKE $1
              AND end_time > NOW()
            ORDER BY ABS(EXTRACT(EPOCH FROM (start_time - NOW()))) ASC
            LIMIT 1
            "#,
        )
        .bind(pattern)
        .fetch_optional(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(row.map(CtfEvent::from))
    }

    async fn list_recently_ended(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CtfEvent>> {
        let rows = sqlx::query_as::<_, DbCtfEvent>(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at,
                   NULL as total_count
            FROM ctf_events
            WHERE end_time BETWEEN $1 AND $2
            ORDER BY end_time DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows.into_iter().map(CtfEvent::from).collect())
    }

    async fn list_completed(
        &self,
        limit: i64,
        offset: i64,
        filter: &CompletedFilter,
    ) -> Result<PaginatedEvents> {
        let mut qb = QueryBuilder::new(
            "SELECT id, ctftime_id, title, url, start_time, end_time, \
             weight, format, organiser, description, \
             social_links, created_at, updated_at, is_onsite, \
             enriched_at, notified_at, \
             COUNT(*) OVER() as total_count \
             FROM ctf_events WHERE end_time < ",
        );
        qb.push_bind(Utc::now());

        if let Some(ref fmt) = filter.format {
            qb.push(" AND format ILIKE ");
            qb.push_bind(fmt);
        }
        if let Some(min_w) = filter.min_weight {
            qb.push(" AND weight >= ");
            qb.push_bind(min_w);
        }

        qb.push(" ORDER BY end_time DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let rows = qb
            .build_query_as::<DbCtfEvent>()
            .fetch_all(&self.pool)
            .await
            .map_err(crate::db_err)?;

        let total_count = rows.first().and_then(|r| r.total_count).unwrap_or(0);
        let events = rows.into_iter().map(CtfEvent::from).collect();
        Ok(PaginatedEvents {
            events,
            total_count,
        })
    }

    async fn get_all_by_title_fuzzy(&self, query: &str) -> Result<Option<CtfEvent>> {
        // If query is numeric, prioritize ctftime_id match
        if let Ok(id) = query.parse::<i64>()
            && let Some(event) = self.get_by_ctftime_id(id).await?
        {
            return Ok(Some(event));
        }

        let pattern = format!("%{query}%");

        let row = sqlx::query_as::<_, DbCtfEvent>(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at,
                   NULL as total_count
            FROM ctf_events
            WHERE title ILIKE $1
            ORDER BY ABS(EXTRACT(EPOCH FROM (start_time - NOW()))) ASC
            LIMIT 1
            "#,
        )
        .bind(pattern)
        .fetch_optional(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(row.map(CtfEvent::from))
    }

    async fn get_all_by_title_fuzzy_with_score(
        &self,
        query: &str,
        min_similarity: f32,
    ) -> Result<Option<(CtfEvent, f32)>> {
        let row = sqlx::query!(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at,
                   word_similarity(title, $1) as score
            FROM ctf_events
            WHERE title % $1 AND word_similarity(title, $1) >= $2
            ORDER BY score DESC, ABS(EXTRACT(EPOCH FROM (start_time - NOW()))) ASC
            LIMIT 1
            "#,
            query,
            min_similarity as f64
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(row.map(|r| {
            let social_links: Vec<SocialLink> = serde_json::from_value(r.social_links.clone()).unwrap_or_else(|e| {
                tracing::warn!(ctftime_id = r.ctftime_id, error = %e, "Failed to parse social_links for fuzzy-matched event; using empty list");
                vec![]
            });
            (CtfEvent {
                id: Some(r.id),
                ctftime_id: r.ctftime_id,
                title: r.title,
                url: r.url,
                start_time: r.start_time,
                end_time: r.end_time,
                weight: r.weight,
                format: r.format,
                organiser: r.organiser,
                description: r.description,
                social_links,
                is_onsite: r.is_onsite,
                enriched_at: r.enriched_at,
                notified_at: r.notified_at,
            }, r.score.unwrap_or(0.0))
        }))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl WriteCtfRepository for PostgresCtfRepository {
    async fn upsert_event(&self, event: &CtfEvent) -> Result<UpsertStatus> {
        use sqlx::Row;
        let row = sqlx::query(
            r#"
            INSERT INTO ctf_events
                (ctftime_id, title, url, start_time, end_time,
                 weight, format, organiser, description, social_links, is_onsite)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (ctftime_id) DO UPDATE SET
                title        = EXCLUDED.title,
                url          = EXCLUDED.url,
                start_time   = EXCLUDED.start_time,
                end_time     = EXCLUDED.end_time,
                weight       = EXCLUDED.weight,
                format       = EXCLUDED.format,
                organiser    = EXCLUDED.organiser,
                description  = EXCLUDED.description,
                social_links = EXCLUDED.social_links,
                is_onsite    = EXCLUDED.is_onsite,
                updated_at   = NOW()
            WHERE
                ctf_events.title IS DISTINCT FROM EXCLUDED.title OR
                ctf_events.url IS DISTINCT FROM EXCLUDED.url OR
                ctf_events.start_time IS DISTINCT FROM EXCLUDED.start_time OR
                ctf_events.end_time IS DISTINCT FROM EXCLUDED.end_time OR
                ctf_events.weight IS DISTINCT FROM EXCLUDED.weight OR
                ctf_events.format IS DISTINCT FROM EXCLUDED.format OR
                ctf_events.organiser IS DISTINCT FROM EXCLUDED.organiser OR
                ctf_events.description IS DISTINCT FROM EXCLUDED.description OR
                ctf_events.social_links IS DISTINCT FROM EXCLUDED.social_links OR
                ctf_events.is_onsite IS DISTINCT FROM EXCLUDED.is_onsite
            RETURNING (xmax = 0) AS inserted
            "#,
        )
        .bind(event.ctftime_id)
        .bind(&event.title)
        .bind(&event.url)
        .bind(event.start_time)
        .bind(event.end_time)
        .bind(event.weight)
        .bind(&event.format)
        .bind(&event.organiser)
        .bind(&event.description)
        .bind(serde_json::to_value(&event.social_links).unwrap_or_else(|e| {
            tracing::warn!(ctftime_id = event.ctftime_id, error = %e, "Failed to serialize social_links for upsert; using empty list");
            serde_json::Value::Array(vec![])
        }))
        .bind(event.is_onsite)
        .fetch_optional(&self.pool)
        .await.map_err(crate::db_err)?;

        match row {
            None => Ok(UpsertStatus::Unchanged),
            Some(row) => {
                let inserted: bool = row.try_get("inserted").map_err(crate::db_err)?;
                if inserted {
                    Ok(UpsertStatus::Inserted)
                } else {
                    Ok(UpsertStatus::Updated)
                }
            }
        }
    }

    async fn invalidate_upcoming_cache(&self) -> Result<()> {
        if let Some(ref client) = self.redis {
            let mut conn = client
                .get_multiplexed_async_connection()
                .await
                .map_err(crate::redis_err)?;

            let mut cursor: u64 = 0;
            let mut to_delete = Vec::new();

            loop {
                let (next_cursor, page): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg("upcoming:*")
                    .arg("COUNT")
                    .arg(100)
                    .query_async(&mut conn)
                    .await
                    .map_err(crate::redis_err)?;

                to_delete.extend(page);
                cursor = next_cursor;
                if cursor == 0 {
                    break;
                }
            }

            if !to_delete.is_empty() {
                let _: () = redis::cmd("DEL")
                    .arg(&to_delete)
                    .query_async(&mut conn)
                    .await
                    .map_err(crate::redis_err)?;

                tracing::debug!(count = to_delete.len(), "Invalidated upcoming cache keys");
            }
        }
        Ok(())
    }

    async fn list_unenriched_events(&self, limit: i64) -> Result<Vec<CtfEvent>> {
        let rows = sqlx::query_as::<_, DbCtfEvent>(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at, NULL as total_count
            FROM ctf_events
            WHERE enriched_at IS NULL
            ORDER BY created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows.into_iter().map(CtfEvent::from).collect())
    }

    async fn mark_event_enriched(&self, id: Uuid, description: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE ctf_events SET description = $1, enriched_at = NOW() WHERE id = $2",
            description,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }

    async fn list_unnotified_events(&self) -> Result<Vec<CtfEvent>> {
        let rows = sqlx::query_as::<_, DbCtfEvent>(
            r#"
            SELECT id, ctftime_id, title, url, start_time, end_time,
                   weight, format, organiser, description,
                   social_links, created_at, updated_at, is_onsite,
                   enriched_at, notified_at, NULL as total_count
            FROM ctf_events
            WHERE notified_at IS NULL AND (enriched_at IS NOT NULL OR description IS NULL OR description = '')
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows.into_iter().map(CtfEvent::from).collect())
    }

    async fn mark_event_notified(&self, id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE ctf_events SET notified_at = NOW() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }
}
