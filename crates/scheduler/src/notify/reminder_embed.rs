use serde_json::{Value, json};
use shared::{Reminder, ReminderKind};

pub fn build_reminder_dm(reminder: &Reminder) -> Value {
    match reminder.kind {
        ReminderKind::Event => json!({
            "embeds": [{
                "color": 0x5865F2,  // Discord blurple
                "title": "🔔 Event Reminder",
                "description": format!(
                    "**{}** is starting soon!\n\n\
                     **Starts:** <t:{}:F>\n\
                     **CTFTime:** https://ctftime.org/event/{}",
                    reminder.event_title.as_deref().unwrap_or("Unknown"),
                    reminder.event_start_at.map(|t| t.timestamp()).unwrap_or(0),
                    reminder.ctftime_id.unwrap_or(0),
                ),
                "footer": { "text": "Good luck! 🚩" }
            }]
        }),

        ReminderKind::Timer => json!({
            "embeds": [{
                "color": 0xFEE75C,  // yellow
                "title": "⏰ Timer",
                "description": reminder.message.as_deref().unwrap_or("Your timer has fired."),
                "footer": {
                    "text": format!("Set <t:{}:R>", reminder.created_at.timestamp())
                }
            }]
        }),

        ReminderKind::Recurring => json!({
            "embeds": [{
                "color": 0x57F287,  // green
                "title": "🔁 Recurring Reminder",
                "description": reminder.message.as_deref().unwrap_or("Recurring reminder."),
                "fields": [
                    {
                        "name": "Fire",
                        "value": format!(
                            "#{} of {}",
                            reminder.sent_count + 1,
                            reminder.fire_count_max.unwrap_or(0)
                        ),
                        "inline": true
                    },
                    {
                        "name": "Until",
                        "value": format!(
                            "<t:{}:F>",
                            reminder.repeat_until.map(|t| t.timestamp()).unwrap_or(0)
                        ),
                        "inline": true
                    }
                ],
                "footer": {
                    "text": format!("Every {}", format_interval(reminder.interval_secs.unwrap_or(0)))
                }
            }]
        }),
    }
}

fn format_interval(secs: i64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    [
        (d > 0).then(|| format!("{d}d")),
        (h > 0).then(|| format!("{h}h")),
        (m > 0).then(|| format!("{m}m")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
}
