use crate::state::AppState;
use chrono::{DateTime, TimeZone, Utc};
use shared::CtfResult;
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::Interaction;
use twilight_model::channel::message::{
    MessageFlags,
    component::{ActionRow, Button, ButtonStyle, Component},
};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};

pub async fn handle(
    _http: &HttpClient,
    interaction: &Interaction,
    state: &AppState,
    cursor: Option<DateTime<Utc>>,
    response_type: InteractionResponseType,
) -> CtfResult<InteractionResponse> {
    let user_id = interaction
        .author_id()
        .ok_or_else(|| shared::CtfError::InvalidInput("Cannot identify user".into()))?
        .to_string();

    let mut reminders = state.reminders.list_pending(&user_id, cursor).await?;

    if reminders.is_empty() && cursor.is_none() {
        return Ok(crate::embed::ephemeral_reply(
            "You have no active reminders.\n\
             Use `/reminder set event`, `/reminder set timer`, or `/reminder set recurring` to create one.",
        ));
    }

    let has_next = reminders.len() == 11;
    if has_next {
        reminders.pop();
    }

    let lines: Vec<String> = reminders
        .iter()
        .enumerate()
        .map(|(i, r)| format!("**{}** {}", i + 1, r.list_label()))
        .collect();

    let content = format!(
        "**Your reminders ({} active)**\n\n{}\n\n\
         Use `/reminder cancel` with the number to remove one (use `/reminder list` to find the number).",
        reminders.len(), // This len is just for the current page, which might be confusing.
        lines.join("\n\n"),
    );

    let mut response_data = InteractionResponseData {
        content: Some(content),
        flags: Some(MessageFlags::EPHEMERAL),
        ..Default::default()
    };

    if has_next && let Some(last) = reminders.last() {
        let next_cursor = last.remind_at.timestamp();
        response_data.components = Some(vec![Component::ActionRow(ActionRow {
            components: vec![Component::Button(Button {
                custom_id: Some(format!("reminder_list:{}", next_cursor)),
                label: Some("Next →".to_string()),
                style: ButtonStyle::Primary,
                disabled: false,
                emoji: None,
                id: None,
                sku_id: None,
                url: None,
            })],
            id: None,
        })]);
    }

    Ok(InteractionResponse {
        kind: response_type,
        data: Some(response_data),
    })
}

pub async fn handle_component(
    http: &HttpClient,
    interaction: &Interaction,
    state: &AppState,
    cursor_str: &str,
) -> CtfResult<InteractionResponse> {
    let cursor = cursor_str
        .parse::<i64>()
        .ok()
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        .filter(|t| *t > Utc::now() - chrono::Duration::days(1));

    handle(
        http,
        interaction,
        state,
        cursor,
        InteractionResponseType::UpdateMessage,
    )
    .await
}
