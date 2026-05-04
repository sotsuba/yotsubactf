use super::validation::{MAX_PENDING_REMINDERS, MAX_REMINDER_DAYS};
use crate::embed::ephemeral_error;
use chrono::{DateTime, Duration, Utc};
use shared::{CtfResult, ReadCtfRepository, Reminder, ReminderKind, ReminderRepository};
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use uuid::Uuid;

pub async fn create_event_reminder(
    now: DateTime<Utc>,
    event_repo: &dyn ReadCtfRepository,
    reminder_repo: &dyn ReminderRepository,
    user_id: &str,
    ctftime_id: i64,
    offset_secs: i64,
) -> CtfResult<InteractionResponse> {
    let event = match event_repo.get_by_ctftime_id(ctftime_id).await? {
        Some(e) => e,
        None => return Ok(ephemeral_error("Event not found in database.")),
    };

    let remind_at = event.start_time - Duration::seconds(offset_secs);

    // 1. Time validation
    if event.end_time <= now {
        return Ok(ephemeral_error(&format!(
            "⏰ **{}** has already ended.",
            event.title
        )));
    }
    if event.start_time <= now {
        return Ok(ephemeral_error(&format!(
            "⏰ **{}** has already started <t:{}:R>. You can no longer set a reminder.",
            event.title,
            event.start_time.timestamp()
        )));
    }
    if remind_at <= now {
        return Ok(ephemeral_error(&format!(
            "Reminder would fire in the past. Try a shorter offset — the event starts <t:{}:R>.",
            event.start_time.timestamp()
        )));
    }
    if remind_at > now + Duration::days(MAX_REMINDER_DAYS) {
        return Ok(ephemeral_error(&format!(
            "Reminder time is too far in the future (max {} days).",
            MAX_REMINDER_DAYS
        )));
    }

    // 2. Duplicate check
    let pending = reminder_repo.list_pending(user_id, Some(now)).await?;
    if pending.iter().any(|r| r.ctftime_id == Some(ctftime_id)) {
        return Ok(InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseData {
                content: Some(format!(
                    "✅ You already have a reminder set for **{}**.",
                    event.title
                )),
                flags: Some(MessageFlags::EPHEMERAL),
                ..Default::default()
            }),
        });
    }

    // 3. Quota check
    if pending.len() >= MAX_PENDING_REMINDERS {
        return Ok(ephemeral_error(&format!(
            "⚠️ You've reached the maximum of **{} pending reminders**. Wait for some to fire before adding more.",
            MAX_PENDING_REMINDERS
        )));
    }

    // 4. Creation
    let reminder = Reminder {
        id: Uuid::nil(),
        user_id: user_id.to_string(),
        kind: ReminderKind::Event,
        ctftime_id: Some(ctftime_id),
        event_title: Some(event.title.clone()),
        event_start_at: Some(event.start_time),
        remind_at,
        created_at: now,
        ..Default::default()
    };

    reminder_repo.create(&reminder).await?;

    Ok(InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            content: Some(format!(
                "✅ **Reminder set!**\n**{}** starts <t:{}:F>\nYou'll be notified <t:{}:R>",
                event.title,
                event.start_time.timestamp(),
                remind_at.timestamp(),
            )),
            flags: Some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    })
}
