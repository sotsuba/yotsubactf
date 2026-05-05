use twilight_model::application::command::CommandOption;
use twilight_util::builder::command::{IntegerBuilder, StringBuilder};

pub fn message_option() -> CommandOption {
    StringBuilder::new("message", "Message to remind you with")
        .max_length(200)
        .build()
}

pub fn days_option(description: &str, max: i64) -> CommandOption {
    IntegerBuilder::new("days", description)
        .min_value(0)
        .max_value(max)
        .build()
}

pub fn hours_option(description: &str) -> CommandOption {
    IntegerBuilder::new("hours", description)
        .min_value(0)
        .max_value(23)
        .build()
}

pub fn minutes_option(description: &str) -> CommandOption {
    IntegerBuilder::new("minutes", description)
        .min_value(0)
        .max_value(59)
        .build()
}