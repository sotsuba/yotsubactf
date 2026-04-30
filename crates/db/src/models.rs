use chrono::{DateTime, Utc};
use uuid::Uuid;
use ctftime_core::CtfEvent;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DbCtfEvent {
    pub id: Uuid,
    pub ctftime_id: i64,
    pub title: String,
    pub url: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub weight: Option<f64>,
    pub format: Option<String>,
    pub organiser: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DbCtfEvent> for CtfEvent {
    fn from(db_row: DbCtfEvent) -> Self {
        Self {
            id: Some(db_row.id),
            ctftime_id: db_row.ctftime_id,
            title: db_row.title,
            url: db_row.url,
            start_time: db_row.start_time,
            end_time: db_row.end_time,
            weight: db_row.weight,
            format: db_row.format,
            organiser: db_row.organiser,
            description: db_row.description,
        }
    }
}