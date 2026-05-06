use chrono::Utc;
use serde_json::{Value, json};
use shared::{CtfEmbed, TeamResult};

pub fn build_result_notification(result: &TeamResult, event_title: &str, team_name: &str) -> Value {
    let mut embed = CtfEmbed::new(format!("🏆 New Result for {}", team_name))
        .description(format!("**{}** result is out!", event_title))
        .color(0xF1C40F) // Gold-ish
        .footer("YotsubaCTF • results")
        .timestamp(Utc::now())
        .field("Team", team_name, true)
        .field("Event", event_title, true);

    if let Some(place) = result.place {
        let medal = match place {
            1 => "🥇 ",
            2 => "🥈 ",
            3 => "🥉 ",
            _ => "",
        };
        embed = embed.field("Place", format!("{}{} place", medal, place), true);
    }

    if let Some(score) = result.score {
        embed = embed.field("Score", format!("{:.2} pts", score), true);
    }

    if let Some(total) = result.total_teams {
        embed = embed.field("Total Teams", total.to_string(), true);
    }

    json!({ "embeds": [embed.to_json()] })
}
