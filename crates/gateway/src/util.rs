use anyhow::{Context, Result};
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use twilight_model::application::interaction::application_command::CommandDataOption;

    fn unique_env_key() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after unix epoch")
            .as_nanos();
        format!("YOTSUBA_TEST_ENV_{}_{}", std::process::id(), nanos)
    }

    #[test]
    fn parse_id_parses_valid_u64() {
        let parsed = parse_id::<()>("42", "TEST_ID").expect("id should parse");
        assert_eq!(parsed.get(), 42);
    }

    #[test]
    fn parse_id_reports_context_on_invalid_input() {
        let err = parse_id::<()>("not-a-number", "TEST_ID")
            .expect_err("invalid id should fail")
            .to_string();
        assert!(err.contains("TEST_ID must be a valid u64"));
    }

    #[test]
    fn parse_u16_env_returns_some_for_valid_integer() {
        let key = unique_env_key();
        // SAFETY: test uses a unique key to avoid concurrent access with other code.
        unsafe { std::env::set_var(&key, "8080") };
        let value = parse_u16_env(&key);
        // SAFETY: key is unique to this test and cleaned up immediately.
        unsafe { std::env::remove_var(&key) };

        assert_eq!(value, Some(8080));
    }

    #[test]
    fn parse_u16_env_returns_none_for_invalid_or_missing_values() {
        let key = unique_env_key();
        // SAFETY: test uses a unique key to avoid concurrent access with other code.
        unsafe { std::env::set_var(&key, "invalid") };
        assert_eq!(parse_u16_env(&key), None);
        // SAFETY: key is unique to this test and cleaned up immediately.
        unsafe { std::env::remove_var(&key) };
        assert_eq!(parse_u16_env(&key), None);
    }

    #[test]
    fn get_string_option_returns_matching_string_value() {
        let options = vec![
            CommandDataOption {
                name: "count".into(),
                value: CommandOptionValue::Integer(5),
            },
            CommandDataOption {
                name: "name".into(),
                value: CommandOptionValue::String("yotsuba".into()),
            },
        ];

        assert_eq!(get_string_option(&options, "name"), Some("yotsuba"));
        assert_eq!(get_string_option(&options, "count"), None);
        assert_eq!(get_string_option(&options, "missing"), None);
    }

    #[test]
    fn truncate_handles_short_ascii_and_unicode_inputs() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("abcdef", 3), "abc...");
        assert_eq!(truncate("あいうえお", 3), "あいう...");
    }
}
