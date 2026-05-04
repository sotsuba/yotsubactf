use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::CtfResult as Result;
use shared::{Writeup, WriteupRepository, WriteupSearchResult};
use sqlx::PgPool;
use uuid::Uuid;

pub struct PostgresWriteupRepository {
    pool: PgPool,
    redis: Option<redis::Client>,
}

impl PostgresWriteupRepository {
    pub fn new(pool: PgPool, redis: Option<redis::Client>) -> Self {
        Self { pool, redis }
    }
}

#[async_trait]
impl WriteupRepository for PostgresWriteupRepository {
    async fn upsert_writeup(&self, writeup: &Writeup) -> Result<bool> {
        let row = sqlx::query!(
            r#"
            INSERT INTO writeups (
                id, ctftime_id, title, url, event_id, 
                category, event_name, published_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (ctftime_id) DO UPDATE SET
                title = EXCLUDED.title,
                url = EXCLUDED.url,
                event_id = CASE WHEN writeups.event_id = 0 THEN EXCLUDED.event_id ELSE writeups.event_id END,
                category = EXCLUDED.category,
                event_name = EXCLUDED.event_name,
                published_at = EXCLUDED.published_at
            RETURNING (xmax = 0) as is_inserted
            "#,
            writeup.id,
            writeup.ctftime_id,
            writeup.title,
            writeup.url,
            writeup.event_id,
            writeup.category,
            writeup.event_name,
            writeup.published_at,
            writeup.created_at
        )
        .fetch_one(&self.pool)
        .await.map_err(crate::db_err)?;

        let is_inserted = row.is_inserted.unwrap_or_else(|| {
            tracing::error!(
                ctftime_id = writeup.ctftime_id,
                "is_inserted is NULL in upsert_writeup RETURNING clause"
            );
            false
        });

        // Invalidate Redis cache when a new writeup is inserted
        if is_inserted
            && let Some(client) = &self.redis
            && let Ok(mut conn) = client.get_multiplexed_async_connection().await
        {
            let mut keys = Vec::new();
            let mut cursor: u64 = 0;

            loop {
                let (new_cursor, page): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg("writeups:recent:*")
                    .arg("COUNT")
                    .arg(100)
                    .query_async(&mut conn)
                    .await
                    .unwrap_or((0, vec![]));

                keys.extend(page);
                cursor = new_cursor;
                if cursor == 0 {
                    break;
                }
            }

            if !keys.is_empty() {
                let _: () = redis::cmd("DEL")
                    .arg(&keys)
                    .query_async(&mut conn)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "Failed to delete keys from Redis for writeup cache invalidation");
                    });
            }
        }

        Ok(is_inserted)
    }

    async fn list_by_event(&self, event_id: i64) -> Result<Vec<Writeup>> {
        let rows = sqlx::query_as!(
            Writeup,
            r#"
            SELECT 
                id, ctftime_id, title, url, event_id, 
                category, event_name, published_at, created_at as "created_at!"
            FROM writeups
            WHERE event_id = $1
            ORDER BY published_at DESC NULLS LAST, created_at DESC
            "#,
            event_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows)
    }

    async fn list_unnotified_writeups(&self) -> Result<Vec<Writeup>> {
        let rows = sqlx::query_as!(
            Writeup,
            r#"
            SELECT 
                id, ctftime_id, title, url, event_id, 
                category, event_name, published_at, created_at as "created_at!"
            FROM writeups
            WHERE notified_at IS NULL
            ORDER BY created_at ASC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows)
    }

    async fn mark_writeup_notified(&self, id: Uuid) -> Result<()> {
        sqlx::query!("UPDATE writeups SET notified_at = NOW() WHERE id = $1", id)
            .execute(&self.pool)
            .await
            .map_err(crate::db_err)?;
        Ok(())
    }

    async fn search_writeups(
        &self,
        query: &str,
        category: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<WriteupSearchResult>> {
        if query.trim().is_empty() {
            if let Some(cat) = category {
                let rows = sqlx::query_as!(
                    Writeup,
                    r#"
                    SELECT 
                        id, ctftime_id, title, url, event_id, 
                        category, event_name, published_at, created_at as "created_at!"
                    FROM writeups
                    WHERE category = $1
                    ORDER BY published_at DESC NULLS LAST, created_at DESC
                    LIMIT $2
                    OFFSET $3
                    "#,
                    cat,
                    limit,
                    offset,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(crate::db_err)?;

                return Ok(rows
                    .into_iter()
                    .map(|w| WriteupSearchResult {
                        rank: 0.0,
                        writeup: w,
                    })
                    .collect());
            } else {
                let writeups = self.list_recent(limit, offset).await?;
                return Ok(writeups
                    .into_iter()
                    .map(|w| WriteupSearchResult {
                        rank: 0.0,
                        writeup: w,
                    })
                    .collect());
            }
        }

        let rows = sqlx::query!(
            r#"
            SELECT
                id, ctftime_id, title, url, event_id,
                category, event_name, published_at,
                created_at as "created_at!",
                ts_rank(search_vector, plainto_tsquery('english', $1)) as "rank!"
            FROM writeups
            WHERE
                search_vector @@ plainto_tsquery('english', $1)
                AND ($2::text IS NULL OR category = $2)
            ORDER BY "rank!" DESC, published_at DESC NULLS LAST
            LIMIT $3
            OFFSET $4
            "#,
            query,
            category,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows
            .into_iter()
            .map(|r| WriteupSearchResult {
                rank: r.rank,
                writeup: Writeup {
                    id: r.id,
                    ctftime_id: r.ctftime_id,
                    title: r.title,
                    url: r.url,
                    event_id: r.event_id,
                    category: r.category,
                    event_name: r.event_name,
                    published_at: r.published_at,
                    created_at: r.created_at,
                },
            })
            .collect())
    }

    async fn list_recent(&self, limit: i64, offset: i64) -> Result<Vec<Writeup>> {
        // Try Redis cache first
        let cache_key = format!("writeups:recent:{limit }:o{offset}");
        if let Some(client) = &self.redis
            && let Ok(mut conn) = client.get_multiplexed_async_connection().await
        {
            let cached: Option<String> = redis::cmd("GET")
                .arg(&cache_key)
                .query_async(&mut conn)
                .await
                .ok();

            if let Some(json) = cached
                && let Ok(writeups) = serde_json::from_str::<Vec<Writeup>>(&json)
            {
                return Ok(writeups);
            }
        }

        let writeups = sqlx::query_as!(
            Writeup,
            r#"
            SELECT 
                id, ctftime_id, title, url, event_id, 
                category, event_name, published_at, created_at as "created_at!"
            FROM writeups
            ORDER BY published_at DESC NULLS LAST, created_at DESC
            LIMIT $1
            OFFSET $2
            "#,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        // Save to cache
        if let Some(client) = &self.redis
            && let Ok(mut conn) = client.get_multiplexed_async_connection().await
            && let Ok(json) = serde_json::to_string(&writeups)
        {
            let _: () = redis::cmd("SETEX")
                .arg(&cache_key)
                .arg(600) // 10 minutes
                .arg(json)
                .query_async(&mut conn)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to save writeups to Redis cache");
                });
        }

        Ok(writeups)
    }

    async fn list_by_event_name(
        &self,
        name: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Writeup>> {
        let rows = sqlx::query_as!(
            Writeup,
            r#"
            SELECT 
                id, ctftime_id, title, url, event_id, 
                category, event_name, published_at, created_at as "created_at!"
            FROM writeups
            WHERE event_name ILIKE $1 OR title ILIKE $1
            ORDER BY published_at DESC NULLS LAST, created_at DESC
            LIMIT $2
            OFFSET $3
            "#,
            format!("%{}%", name),
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows)
    }

    async fn list_by_event_ids(
        &self,
        event_ids: &[i64],
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Writeup>> {
        let rows = sqlx::query_as!(
            Writeup,
            r#"
            SELECT 
                id, ctftime_id, title, url, event_id, 
                category, event_name, published_at, created_at as "created_at!"
            FROM writeups
            WHERE event_id = ANY($1::bigint[])
            ORDER BY published_at DESC NULLS LAST, created_at DESC
            LIMIT $2
            OFFSET $3
            "#,
            event_ids,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows)
    }

    async fn autocomplete_event_name(&self, prefix: &str, limit: i64) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT event_name
            FROM writeups
            WHERE event_name ILIKE $1 AND event_name IS NOT NULL
            ORDER BY event_name
            LIMIT $2
            "#,
            format!("%{}%", prefix),
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows.into_iter().filter_map(|r| r.event_name).collect())
    }

    async fn list_top_writeups_since(
        &self,
        since: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<Writeup>> {
        let rows = sqlx::query_as!(
            Writeup,
            r#"
            SELECT DISTINCT ON (category)
                id, ctftime_id, title, url, event_id, 
                category, event_name, published_at, created_at as "created_at!"
            FROM writeups
            WHERE published_at >= $1 OR (published_at IS NULL AND created_at >= $1)
            ORDER BY category, published_at DESC NULLS LAST
            LIMIT $2
            "#,
            since,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
