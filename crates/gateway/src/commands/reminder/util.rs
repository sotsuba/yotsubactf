use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::{InteractionResponse, InteractionResponseType};
use twilight_util::builder::InteractionResponseDataBuilder;

pub fn parse_options(
    options: &[CommandDataOption],
) -> std::collections::HashMap<&str, &CommandOptionValue> {
    options
        .iter()
        .map(|o| (o.name.as_str(), &o.value))
        .collect()
}

pub fn opt_int(
    opts: &std::collections::HashMap<&str, &CommandOptionValue>,
    name: &str,
) -> Option<i64> {
    opts.get(name).and_then(|v| {
        if let CommandOptionValue::Integer(i) = v {
            Some(*i)
        } else {
            None
        }
    })
}

pub fn opt_str(
    opts: &std::collections::HashMap<&str, &CommandOptionValue>,
    name: &str,
) -> Option<String> {
    opts.get(name).and_then(|v| {
        if let CommandOptionValue::String(s) = v {
            Some(s.clone())
        } else {
            None
        }
    })
}

pub fn opt_int_or_zero(
    opts: &std::collections::HashMap<&str, &CommandOptionValue>,
    name: &str,
) -> i64 {
    opt_int(opts, name).unwrap_or(0)
}

#[allow(dead_code)]
pub async fn reply_ephemeral(
    http: &twilight_http::Client,
    interaction: &Interaction,
    message: &str,
) -> shared::CtfResult<()> {
    let response = InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(
            InteractionResponseDataBuilder::new()
                .content(message)
                .flags(twilight_model::channel::message::MessageFlags::EPHEMERAL)
                .build(),
        ),
    };
    http.interaction(interaction.application_id)
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|e| shared::CtfError::ExternalApi {
            status: 0,
            message: e.to_string(),
        })?;
    Ok(())
}
