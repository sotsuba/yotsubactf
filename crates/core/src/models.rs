use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}