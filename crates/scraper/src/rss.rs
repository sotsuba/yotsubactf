use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::Client;
use rss::{extension::Extension, Channel, Item};
use serde_json::from_str;

use crate::models::{RawCtftimeEvent, RawOrganizer};

const UPCOMMING_CTF_RSS_URL: &'static str = "https://ctftime.org/event/list/upcoming/rss";

pub async fn fetch_upcoming(client: &Client) -> Result<Vec<RawCtftimeEvent>> {
    let resp = client.get(UPCOMMING_CTF_RSS_URL).send().await?.bytes().await?;
    let channel = Channel::read_from(&resp[..])?;

    Ok(channel.items().iter().map(|item| {
        let start_date = extension_value(item, "start_date")
            .and_then(|value| convert_ctftime_datetime(&value))
            .unwrap_or_default();
        let finish_date = extension_value(item, "finish_date")
            .and_then(|value| convert_ctftime_datetime(&value))
            .unwrap_or_default();
        let weight = extension_value(item, "weight")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0);
        let format = extension_value(item, "format_text").unwrap_or_default();
        let organizers = extension_value(item, "organizers")
            .and_then(|value| from_str::<Vec<RawOrganizer>>(value.trim()).ok())
            .unwrap_or_default();
        let url = extension_value(item, "url")
            .or_else(|| item.link().map(|value| value.to_string()))
            .unwrap_or_default();
        let guid = item
            .guid()
            .map(|value| value.value().to_string())
            .unwrap_or_default();

        RawCtftimeEvent {
            ctftime_id:       extract_ctftime_id(&guid).unwrap_or(0),
            title:            item.title().unwrap_or_default().to_string(),
            url,
            description:      item.description().unwrap_or_default().to_string(),
            start:            start_date,
            finish:           finish_date,
            weight,
            format,
            organizers,
        }
    }).collect())
}

fn extension_value(item: &Item, key: &str) -> Option<String> {
    item
        .extensions()
        .iter()
        .find_map(|(_, values)| values.get(key).and_then(first_extension_value))
}

fn first_extension_value(extensions: &Vec<Extension>) -> Option<String> {
    extensions.first().and_then(|ext| ext.value.clone())
}

fn convert_ctftime_datetime(value: &str) -> Option<String> {
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    let datetime = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
    Some(datetime.to_rfc3339())
}

fn extract_ctftime_id(id: &str) -> Option<i64> {
    let digits: String = id.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.chars().rev().collect::<String>().parse().ok()
    }
}
