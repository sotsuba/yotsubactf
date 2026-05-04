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

pub struct CipherCommand;

#[async_trait]
impl SlashCommand for CipherCommand {
    fn name(&self) -> &'static str {
        "cipher"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new(
            "cipher",
            "Encode or decode a string",
            CommandType::ChatInput,
        )
        .option(
            StringBuilder::new("type", "Cipher / encoding type")
                .required(true)
                .choices([
                    ("Base64", "base64"),
                    ("Base32", "base32"),
                    ("Hex", "hex"),
                    ("URL", "url"),
                    ("ROT13", "rot13"),
                    ("Binary", "binary"),
                    ("Morse", "morse"),
                    ("Atbash", "atbash"),
                ])
                .build(),
        )
        .option(
            StringBuilder::new("mode", "Encode or decode")
                .required(true)
                .choices([("Encode", "encode"), ("Decode", "decode")])
                .build(),
        )
        .option(
            StringBuilder::new("input", "String to process")
                .required(true)
                .build(),
        )
        .build()
    }
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        handle(ctx.options).await
    }
}

use std::collections::HashMap;
use std::sync::OnceLock;

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

static MORSE_ENCODE: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
static MORSE_DECODE: OnceLock<HashMap<&'static str, char>> = OnceLock::new();

fn morse_encode() -> &'static HashMap<char, &'static str> {
    MORSE_ENCODE.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert('A', ".-");
        m.insert('B', "-...");
        m.insert('C', "-.-.");
        m.insert('D', "-..");
        m.insert('E', ".");
        m.insert('F', "..-.");
        m.insert('G', "--.");
        m.insert('H', "....");
        m.insert('I', "..");
        m.insert('J', ".---");
        m.insert('K', "-.-");
        m.insert('L', ".-..");
        m.insert('M', "--");
        m.insert('N', "-.");
        m.insert('O', "---");
        m.insert('P', ".--.");
        m.insert('Q', "--.-");
        m.insert('R', ".-.");
        m.insert('S', "...");
        m.insert('T', "-");
        m.insert('U', "..-");
        m.insert('V', "...-");
        m.insert('W', ".--");
        m.insert('X', "-..-");
        m.insert('Y', "-.--");
        m.insert('Z', "--..");
        m.insert('0', "-----");
        m.insert('1', ".----");
        m.insert('2', "..---");
        m.insert('3', "...--");
        m.insert('4', "....-");
        m.insert('5', ".....");
        m.insert('6', "-....");
        m.insert('7', "--...");
        m.insert('8', "---..");
        m.insert('9', "----.");
        m
    })
}

fn morse_decode() -> &'static HashMap<&'static str, char> {
    MORSE_DECODE.get_or_init(|| morse_encode().iter().map(|(&k, &v)| (v, k)).collect())
}

fn encode_base64(input: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD.encode(input)
}

fn decode_base64(input: &str) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose};
    match general_purpose::STANDARD.decode(input) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err("Base64 decode yielded invalid UTF-8".to_string()),
        },
        Err(e) => Err(format!("Base64 decode error: {}", e)),
    }
}

fn encode_base32(input: &str) -> String {
    data_encoding::BASE32.encode(input.as_bytes())
}

fn decode_base32(input: &str) -> Result<String, String> {
    match data_encoding::BASE32.decode(input.as_bytes()) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err("Base32 decode yielded invalid UTF-8".to_string()),
        },
        Err(e) => Err(format!("Base32 decode error: {}", e)),
    }
}

fn encode_hex(input: &str) -> String {
    hex::encode(input)
}

fn decode_hex(input: &str) -> Result<String, String> {
    match hex::decode(input) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err("Hex decode yielded invalid UTF-8".to_string()),
        },
        Err(e) => Err(format!("Hex decode error: {}", e)),
    }
}

fn encode_url(input: &str) -> String {
    urlencoding::encode(input).to_string()
}

fn decode_url(input: &str) -> Result<String, String> {
    match urlencoding::decode(input) {
        Ok(s) => Ok(s.to_string()),
        Err(e) => Err(format!("URL decode error: {}", e)),
    }
}

fn rot13(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'a'..='m' | 'A'..='M' => ((c as u8) + 13) as char,
            'n'..='z' | 'N'..='Z' => ((c as u8) - 13) as char,
            _ => c,
        })
        .collect()
}

fn encode_binary(input: &str) -> String {
    input
        .as_bytes()
        .iter()
        .map(|b| format!("{:08b}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_binary(input: &str) -> Result<String, String> {
    let mut bytes = Vec::new();
    for part in input.split_whitespace() {
        match u8::from_str_radix(part, 2) {
            Ok(b) => bytes.push(b),
            Err(_) => return Err(format!("Invalid binary part: {}", part)),
        }
    }
    match String::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(_) => Err("Binary decode yielded invalid UTF-8".to_string()),
    }
}

fn encode_morse(input: &str) -> String {
    let mut words = Vec::new();
    for word in input.to_uppercase().split_whitespace() {
        let mut morse_chars = Vec::new();
        for c in word.chars() {
            if let Some(&m) = morse_encode().get(&c) {
                morse_chars.push(m);
            }
        }
        words.push(morse_chars.join(" "));
    }
    words.join(" / ")
}

fn decode_morse(input: &str) -> Result<String, String> {
    let mut text = String::new();
    for word in input.split('/') {
        for m in word.split_whitespace() {
            if let Some(&c) = morse_decode().get(m) {
                text.push(c);
            } else {
                return Err(format!("Invalid morse sequence: {}", m));
            }
        }
        text.push(' ');
    }
    Ok(text.trim().to_string())
}

fn atbash(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'a'..='z' => ('z' as u8 - (c as u8 - 'a' as u8)) as char,
            'A'..='Z' => ('Z' as u8 - (c as u8 - 'A' as u8)) as char,
            _ => c,
        })
        .collect()
}

pub async fn handle(options: &[CommandDataOption]) -> CtfResult<InteractionResponse> {
    let mut ctype = String::new();
    let mut mode = String::new();
    let mut input_str = String::new();

    for option in options {
        match option.name.as_str() {
            "type" => {
                if let CommandOptionValue::String(s) = &option.value {
                    ctype = s.clone();
                }
            }
            "mode" => {
                if let CommandOptionValue::String(s) = &option.value {
                    mode = s.clone();
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

    if ctype.is_empty() || mode.is_empty() || input_str.is_empty() {
        return Ok(ephemeral_error("Missing required options"));
    }

    let result = match (ctype.as_str(), mode.as_str()) {
        ("base64", "encode") => Ok(encode_base64(&input_str)),
        ("base64", "decode") => decode_base64(&input_str),
        ("base32", "encode") => Ok(encode_base32(&input_str)),
        ("base32", "decode") => decode_base32(&input_str),
        ("hex", "encode") => Ok(encode_hex(&input_str)),
        ("hex", "decode") => decode_hex(&input_str),
        ("url", "encode") => Ok(encode_url(&input_str)),
        ("url", "decode") => decode_url(&input_str),
        ("rot13", _) => Ok(rot13(&input_str)),
        ("binary", "encode") => Ok(encode_binary(&input_str)),
        ("binary", "decode") => decode_binary(&input_str),
        ("morse", "encode") => Ok(encode_morse(&input_str)),
        ("morse", "decode") => decode_morse(&input_str),
        ("atbash", _) => Ok(atbash(&input_str)),
        _ => Err("Unknown cipher type or mode".to_string()),
    };

    match result {
        Ok(mut output) => {
            if output.len() > 1900 {
                output.truncate(1900);
                output.push_str("\n\n*(output bị cắt do quá dài)*");
            }

            let type_title = match ctype.as_str() {
                "base64" => "Base64",
                "base32" => "Base32",
                "hex" => "Hex",
                "url" => "URL",
                "rot13" => "ROT13",
                "binary" => "Binary",
                "morse" => "Morse",
                "atbash" => "Atbash",
                _ => "Unknown",
            };

            let mode_title = match mode.as_str() {
                "encode" => "Encode",
                "decode" => "Decode",
                _ => "Unknown",
            };

            let embed = CtfEmbed::success(format!("🔐 {} — {}", type_title, mode_title))
                .field("Input", truncate(&input_str, 200), false)
                .field("Output", output, false)
                .build();
            Ok(ephemeral_embed(embed))
        }
        Err(e) => {
            let type_title = ctype.to_uppercase();
            let mode_title = mode.to_uppercase();
            let embed = CtfEmbed::error(format!("🔐 {} — {}", type_title, mode_title))
                .field("Input", truncate(&input_str, 200), false)
                .field("Error", e, false)
                .build();
            Ok(ephemeral_embed(embed))
        }
    }
}
