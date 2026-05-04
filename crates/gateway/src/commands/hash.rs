use async_trait::async_trait;
use shared::CtfResult;
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::{CommandBuilder, StringBuilder};

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed, ephemeral_error};

pub struct HashCommand;

#[async_trait]
impl SlashCommand for HashCommand {
    fn name(&self) -> &'static str {
        "hash"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "hash",
            "Compute a cryptographic hash",
            CommandType::ChatInput,
        )
        .option(
            StringBuilder::new("type", "Hash algorithm")
                .required(true)
                .choices([
                    ("MD5", "md5"),
                    ("SHA-1", "sha1"),
                    ("SHA-256", "sha256"),
                    ("SHA-512", "sha512"),
                ])
                .build(),
        )
        .option(
            StringBuilder::new("input", "String to hash")
                .required(true)
                .build(),
        )
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        handle(ctx.options).await
    }
}

use digest::Digest;
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha256, Sha512};

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

pub async fn handle(options: &[CommandDataOption]) -> CtfResult<InteractionResponse> {
    let mut htype = String::new();
    let mut input_str = String::new();

    for option in options {
        match option.name.as_str() {
            "type" => {
                if let CommandOptionValue::String(s) = &option.value {
                    htype = s.clone();
                }
            }
            "input" => {
                if let CommandOptionValue::String(s) = &option.value {
                    input_str = s.clone();
                }
            }
            _ => {}
        }
    }

    if htype.is_empty() || input_str.is_empty() {
        return Ok(ephemeral_error("Missing required options"));
    }

    let (hash_hex, bit_len, hname) = match htype.as_str() {
        "md5" => {
            let mut hasher = Md5::new();
            hasher.update(input_str.as_bytes());
            (format!("{:x}", hasher.finalize()), 128, "MD5")
        }
        "sha1" => {
            let mut hasher = Sha1::new();
            hasher.update(input_str.as_bytes());
            (format!("{:x}", hasher.finalize()), 160, "SHA-1")
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(input_str.as_bytes());
            (format!("{:x}", hasher.finalize()), 256, "SHA-256")
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(input_str.as_bytes());
            (format!("{:x}", hasher.finalize()), 512, "SHA-512")
        }
        _ => return Ok(ephemeral_error("Unknown hash type")),
    };

    let embed = CtfEmbed::new(format!("#️⃣ {}", hname))
        .field("Input", truncate(&input_str, 200), false)
        .field("Hash", hash_hex, false)
        .field("Length", format!("{} bits", bit_len), true)
        .build();

    Ok(ephemeral_embed(embed))
}
