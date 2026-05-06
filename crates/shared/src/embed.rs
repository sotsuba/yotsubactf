use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const COLOR_BRAND: u32 = 0x2F_6F_ED;
pub const COLOR_SUCCESS: u32 = 0x2E_CC_71;
pub const COLOR_WARNING: u32 = 0xE6_7E_22;
pub const COLOR_ERROR: u32 = 0xE7_4C_3C;
pub const FOOTER_TEXT: &str = "YotsubaCTF";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EmbedData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<EmbedFooter>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EmbedField>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmbedFooter {
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CtfEmbed {
    pub data: EmbedData,
}

impl CtfEmbed {
    fn with_color(title: impl Into<String>, color: u32) -> Self {
        Self {
            data: EmbedData {
                title: Some(title.into()),
                color: Some(color),
                ..Default::default()
            },
        }
    }

    pub fn new(title: impl Into<String>) -> Self {
        Self::with_color(title, COLOR_BRAND)
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self::with_color(title, COLOR_SUCCESS)
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self::with_color(title, COLOR_WARNING)
    }
    pub fn error(title: impl Into<String>) -> Self {
        Self::with_color(title, COLOR_ERROR)
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.data.title = Some(title.into());
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.data.description = Some(text.into());
        self
    }

    pub fn color(mut self, color: u32) -> Self {
        self.data.color = Some(color);
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.data.url = Some(url.into());
        self
    }

    pub fn field(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        inline: bool,
    ) -> Self {
        self.data.fields.push(EmbedField {
            name: name.into(),
            value: value.into(),
            inline,
        });
        self
    }

    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.data.footer = Some(EmbedFooter { text: text.into() });
        self
    }

    pub fn timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.data.timestamp = Some(ts.to_rfc3339());
        self
    }

    pub fn now(self) -> Self {
        self.footer(FOOTER_TEXT).timestamp(Utc::now())
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.data).unwrap_or_default()
    }
}
