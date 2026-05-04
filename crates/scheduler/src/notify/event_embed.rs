use serde_json::{Value, json};
use shared::embed::COLOR_BRAND;
use shared::{CtfEmbed, CtfEvent};

const REMIND_PREFIX: &str = "remind";

pub fn build_event_notification(event: &CtfEvent) -> Value {
    let start_ts = event.start_time.timestamp();
    let end_ts = event.end_time.timestamp();
    let format = event.format.as_deref().unwrap_or("unknown");
    let weight = event
        .weight
        .map(|w| format!("{w:.2}"))
        .unwrap_or_else(|| "?".into());

    let mut embed = CtfEmbed::new(format!("🚩 {}", event.title))
        .url(event.url.clone())
        .description(format!("Starts <t:{start_ts}:R>"))
        .color(COLOR_BRAND)
        .footer("CTF Bot • new event")
        .field("⏰ Starts", format!("<t:{start_ts}:F>"), true)
        .field("🏁 Ends", format!("<t:{end_ts}:F>"), true)
        .field("📋 Format", format.to_string(), true)
        .field("⚖️ Weight", weight, true);

    if !event.social_links.is_empty() {
        let platforms: Vec<&str> = event
            .social_links
            .iter()
            .map(|l| l.platform.emoji_label())
            .collect();
        embed = embed.field("🔗 Community", platforms.join("  ·  "), false);
    }

    let mut buttons: Vec<Value> = Vec::new();

    buttons.push(json!({
        "type":  2,   // BUTTON
        "style": 5,   // LINK
        "label": "🌐 CTFTime",
        "url":   event.url,
    }));

    for link in event.social_links.iter().take(3) {
        buttons.push(json!({
            "type":  2,
            "style": 5,
            "label": link.platform.emoji_label(),
            "url":   link.url,
        }));
    }

    buttons.push(json!({
        "type":      2,    // BUTTON
        "style":     1,    // PRIMARY (blurple)
        "label":     "🔔 Remind me",
        "custom_id": format!("{REMIND_PREFIX}:{}", event.ctftime_id),
    }));

    json!({
        "embeds":     [embed.to_json()],
        "components": [{ "type": 1, "components": buttons }],
    })
}
