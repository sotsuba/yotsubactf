//! Thin wrappers over twilight's embed/component builders.
//!
//! Goals
//! ─────
//! • Single place to set the brand colour and footer copy.
//! • Fluent, chainable API that mirrors twilight's own builder style.
//! • `PagedResponse` ties an embed to its prev/next buttons AND optional
//!   per-event "Join Community" buttons so callers never have to assemble
//!   the `InteractionResponse` by hand.

use chrono::{DateTime, Utc};
use twilight_model::channel::message::MessageFlags;
use twilight_model::channel::message::component::{ActionRow, Button, ButtonStyle, Component};
use twilight_model::channel::message::embed::Embed;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};

// ── Palette ───────────────────────────────────────────────────────────────────

pub const DEFAULT_PAGE_SIZE: i64 = 5;
pub const MAX_PAGE_SIZE: i64 = 25;

// ── Embed builder ─────────────────────────────────────────────────────────────

pub struct CtfEmbed(shared::CtfEmbed);

#[allow(dead_code)]
impl CtfEmbed {
    pub fn from_shared(inner: shared::CtfEmbed) -> Self {
        Self(inner)
    }
    pub fn new(title: impl Into<String>) -> Self {
        Self(shared::CtfEmbed::new(title))
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self(shared::CtfEmbed::success(title))
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self(shared::CtfEmbed::warning(title))
    }
    pub fn error(title: impl Into<String>) -> Self {
        Self(shared::CtfEmbed::error(title))
    }

    pub fn description(self, text: impl Into<String>) -> Self {
        Self(self.0.description(text))
    }
    #[allow(dead_code)]
    pub fn url(self, url: impl Into<String>) -> Self {
        Self(self.0.url(url))
    }
    pub fn field(self, name: impl Into<String>, value: impl Into<String>, inline: bool) -> Self {
        Self(self.0.field(name, value, inline))
    }
    pub fn footer(self, text: impl Into<String>) -> Self {
        Self(self.0.footer(text))
    }
    #[allow(dead_code)]
    pub fn timestamp(self, ts: DateTime<Utc>) -> Self {
        Self(self.0.timestamp(ts))
    }
    pub fn now(self) -> Self {
        Self(self.0.now())
    }

    pub fn build(self) -> Embed {
        let d = &self.0.data;

        Embed {
            author: None,
            color: d.color,
            description: d.description.clone(),
            fields: d
                .fields
                .iter()
                .map(|f| twilight_model::channel::message::embed::EmbedField {
                    inline: f.inline,
                    name: f.name.clone(),
                    value: f.value.clone(),
                })
                .collect(),
            footer: d.footer.as_ref().map(|f| {
                twilight_model::channel::message::embed::EmbedFooter {
                    icon_url: None,
                    proxy_icon_url: None,
                    text: f.text.clone(),
                }
            }),
            image: None,
            kind: "rich".to_string(),
            provider: None,
            thumbnail: None,
            timestamp: d
                .timestamp
                .as_ref()
                .and_then(|s| twilight_model::util::Timestamp::parse(s).ok()),
            title: d.title.clone(),
            url: d.url.clone(),
            video: None,
        }
    }
}

// ── Paginated response ────────────────────────────────────────────────────────

pub struct PagedResponse {
    pub embed: Embed,
    pub nav: Option<PaginationNav>,
    pub extra_rows: Vec<Component>,
}

pub struct PaginationNav {
    pub prev_id: String,
    pub next_id: String,
    pub has_prev: bool,
    pub has_next: bool,
}

impl PaginationNav {
    pub fn into_action_row(self) -> Component {
        Component::ActionRow(ActionRow {
            components: vec![
                Component::Button(Button {
                    custom_id: Some(self.prev_id),
                    disabled: !self.has_prev,
                    emoji: None,
                    label: Some("◀ Prev".to_string()),
                    style: ButtonStyle::Secondary,
                    url: None,
                }),
                Component::Button(Button {
                    custom_id: Some(self.next_id),
                    disabled: !self.has_next,
                    emoji: None,
                    label: Some("Next ►".to_string()),
                    style: ButtonStyle::Secondary,
                    url: None,
                }),
            ],
        })
    }
}

impl PagedResponse {
    fn data(&self) -> InteractionResponseData {
        let mut components = Vec::new();
        if let Some(nav) = &self.nav {
            components.push(nav.clone().into_action_row());
        }
        components.extend(self.extra_rows.clone());

        InteractionResponseData {
            embeds: Some(vec![self.embed.clone()]),
            components: Some(components),
            ..Default::default()
        }
    }

    pub fn into_new_message(self) -> InteractionResponse {
        InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(self.data()),
        }
    }

    pub fn into_update(self) -> InteractionResponse {
        InteractionResponse {
            kind: InteractionResponseType::UpdateMessage,
            data: Some(self.data()),
        }
    }
}

pub fn paged_response(
    embed: Embed,
    nav: Option<PaginationNav>,
    extra_rows: Vec<Component>,
    update: bool,
) -> InteractionResponse {
    PagedResponse {
        embed,
        nav,
        extra_rows,
    }
    .into_response(update)
}

impl PagedResponse {
    pub fn into_response(self, update: bool) -> InteractionResponse {
        if update {
            self.into_update()
        } else {
            self.into_new_message()
        }
    }
}

impl Clone for PaginationNav {
    fn clone(&self) -> Self {
        Self {
            prev_id: self.prev_id.clone(),
            next_id: self.next_id.clone(),
            has_prev: self.has_prev,
            has_next: self.has_next,
        }
    }
}

// ── Join-community ephemeral response ─────────────────────────────────────────

/// Build an ephemeral reply listing all community platforms for one CTF event.
///
/// Each platform gets a URL button that opens the invite link directly.
pub fn join_community_response(
    event_title: &str,
    social_links: &[shared::SocialLink],
) -> InteractionResponse {
    if social_links.is_empty() {
        return ephemeral_error(&format!(
            "No community links found yet for **{event_title}**. \
             Try checking back after the next enrichment cycle."
        ));
    }

    // Build one URL button per social link.  Discord limits an action row to
    // 5 buttons, so cap at 5 (extremely rare that a CTF has more).
    let buttons: Vec<Component> = social_links
        .iter()
        .take(5)
        .map(|link| {
            Component::Button(Button {
                custom_id: None, // URL buttons must not have a custom_id
                disabled: false,
                emoji: None,
                label: Some(link.platform.emoji_label().to_string()),
                style: ButtonStyle::Link,
                url: Some(link.url.clone()),
            })
        })
        .collect();

    let embed = CtfEmbed::new(format!("Join community — {event_title}"))
        .description(format!(
            "Found **{}** community link(s). Click a button to join:",
            social_links.len()
        ))
        .now()
        .build();

    InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            embeds: Some(vec![embed]),
            components: Some(vec![Component::ActionRow(ActionRow {
                components: buttons,
            })]),
            flags: Some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    }
}

// ── Simple response helpers ───────────────────────────────────────────────────

pub fn ephemeral_embed(embed: Embed) -> InteractionResponse {
    InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            embeds: Some(vec![embed]),
            flags: Some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    }
}

#[allow(dead_code)]
pub fn embed_reply(embed: Embed) -> InteractionResponse {
    InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            embeds: Some(vec![embed]),
            ..Default::default()
        }),
    }
}

pub fn ephemeral_reply(message: impl Into<String>) -> InteractionResponse {
    InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            content: Some(message.into()),
            flags: Some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    }
}

pub fn ephemeral_error(message: &str) -> InteractionResponse {
    InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            content: Some(message.to_string()),
            flags: Some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    }
}
