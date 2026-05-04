use serde_json::{Value, json};
use shared::{CtfEmbed, Writeup};

pub fn build_writeup_notification(writeup: &Writeup) -> Value {
    let mut embed = CtfEmbed::new(format!("📝 New Writeup: {}", writeup.title))
        .url(writeup.url.clone())
        .color(0x3498DB) // Blue
        .footer("CTF Bot • writeups")
        .timestamp(writeup.published_at.unwrap_or(writeup.created_at));

    if let Some(category) = &writeup.category {
        embed = embed.field("📋 Category", format!("`{}`", category), true);
    }

    if let Some(event_name) = &writeup.event_name {
        embed = embed.field("🚩 Event", event_name, true);
    }

    json!({
        "embeds": [embed.to_json()],
        "components": [{
            "type": 1,
            "components": [{
                "type": 2,
                "style": 5,
                "label": "Read Writeup",
                "url": writeup.url.clone()
            }]
        }]
    })
}
