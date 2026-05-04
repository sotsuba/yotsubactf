use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::env;
use twilight_model::id::Id;

pub fn parse_id<M>(value: &str, key: &str) -> Result<Id<M>> {
    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("{key} must be a valid u64"))?;
    Ok(Id::new(parsed))
}

pub fn parse_u16_env(key: &str) -> Option<u16> {
    env::var(key).ok().and_then(|v| v.parse().ok())
}

pub fn format_utc(time: DateTime<Utc>) -> String {
    time.format("%Y-%m-%d %H:%M UTC").to_string()
}

use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};

pub fn get_string_option<'a>(options: &'a [CommandDataOption], name: &str) -> Option<&'a str> {
    options.iter().find(|o| o.name == name).and_then(|o| {
        if let CommandOptionValue::String(ref v) = o.value {
            Some(v.as_str())
        } else {
            None
        }
    })
}

pub fn get_int_option(options: &[CommandDataOption], name: &str) -> Option<i64> {
    options.iter().find(|o| o.name == name).and_then(|o| {
        if let CommandOptionValue::Integer(v) = o.value {
            Some(v)
        } else {
            None
        }
    })
}

/// Truncate a string to a maximum number of characters (not bytes) safely.
/// If truncated, appends an ellipsis.
pub fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let mut truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        truncated.push_str("...");
    }
    truncated
}
