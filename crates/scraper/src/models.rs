use chrono::{DateTime, Utc};
use serde::Deserialize;
use ctftime_core::CtfEvent;

#[derive(Debug, Deserialize)]
pub struct RawCtftimeEvent {
    #[serde(rename = "id")]
    pub ctftime_id: i64,
    pub title: String,
    pub url: String,
    pub description: String, 
    pub start: String,
    pub finish: String,
    pub weight: f64,
    pub format: String,
    pub organizers: Vec<RawOrganizer>,
}

#[derive(Debug, Deserialize)]
pub struct RawOrganizer {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct HtmlEventPatch {
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub format: Option<String>,
    pub weight: Option<f64>,
}

impl RawCtftimeEvent {
    pub fn merge_missing(&mut self, other: RawCtftimeEvent) {
        if is_blank(&self.title) {
            self.title = other.title;
        }
        if is_blank(&self.url) {
            self.url = other.url;
        }
        if is_blank(&self.description) {
            self.description = other.description;
        }
        if is_blank(&self.start) {
            self.start = other.start;
        }
        if is_blank(&self.finish) {
            self.finish = other.finish;
        }
        if self.weight <= 0.0 && other.weight > 0.0 {
            self.weight = other.weight;
        }
        if is_blank(&self.format) {
            self.format = other.format;
        }
        if self.organizers.is_empty() {
            self.organizers = other.organizers;
        }
    }

    pub fn apply_patch(&mut self, patch: HtmlEventPatch) {
        if is_blank(&self.title) {
            if let Some(title) = patch.title {
                self.title = title;
            }
        }
        if is_blank(&self.url) {
            if let Some(url) = patch.url {
                self.url = url;
            }
        }
        if is_blank(&self.description) {
            if let Some(description) = patch.description {
                self.description = description;
            }
        }
        if is_blank(&self.format) {
            if let Some(format) = patch.format {
                self.format = format;
            }
        }
        if self.weight <= 0.0 {
            if let Some(weight) = patch.weight {
                self.weight = weight;
            }
        }
    }
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

impl TryFrom<RawCtftimeEvent> for CtfEvent {
    type Error = chrono::ParseError;

    fn try_from(raw: RawCtftimeEvent) -> Result<Self, Self::Error> {
        let start_time = raw.start.parse::<DateTime<Utc>>()?;
        let end_time = raw.finish.parse::<DateTime<Utc>>()?;
        
        let organiser = raw.organizers.first().map(|o| o.name.clone());
        
        Ok(Self {
            id: None, // New data, not yet in DB
            ctftime_id: raw.ctftime_id,
            title: raw.title,
            url: raw.url,
            start_time,
            end_time,
            weight: Some(raw.weight),
            format: Some(raw.format),
            organiser,
            description: Some(raw.description), 
        })
    }
}