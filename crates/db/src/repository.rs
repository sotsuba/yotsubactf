use anyhow::Result;
use sqlx::PgPool;
use ctftime_core::CtfEvent;

pub struct CtfRepository {
    pool: PgPool,
}

impl CtfRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_event(&self, event: &CtfEvent) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            INSERT INTO ctf_events (
                ctftime_id, title, url, start_time, end_time, 
                weight, format, organiser, description
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (ctftime_id) 
            DO UPDATE SET
                title = EXCLUDED.title,
                url = EXCLUDED.url,
                start_time = EXCLUDED.start_time,
                end_time = EXCLUDED.end_time,
                weight = EXCLUDED.weight,
                format = EXCLUDED.format,
                organiser = EXCLUDED.organiser,
                description = EXCLUDED.description
            "#,
            event.ctftime_id,
            event.title,
            event.url,
            event.start_time,
            event.end_time,
            event.weight,
            event.format,
            event.organiser,
            event.description
        )
        .execute(&self.pool)
        .await?;

        // Trả về true nếu có dòng bị tác động (insert mới hoặc update thành công)
        Ok(result.rows_affected() > 0)
    }
}