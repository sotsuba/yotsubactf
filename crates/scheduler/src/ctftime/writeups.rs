use chrono::{DateTime, Utc};
use regex::Regex;
use reqwest::Client;
use rss::Channel;
use shared::Writeup;
use shared::{CtfError, CtfResult};
use std::sync::OnceLock;
use uuid::Uuid;

const KEYWORDS: &[(&str, &str)] = &[
    ("sql", "web"),
    ("xss", "web"),
    ("rce", "web"),
    ("upload", "web"),
    ("injection", "web"),
    ("rsa", "crypto"),
    ("aes", "crypto"),
    ("cipher", "crypto"),
    ("hash", "crypto"),
    ("ecc", "crypto"),
    ("overflow", "pwn"),
    ("rop", "pwn"),
    ("heap", "pwn"),
    ("stack", "pwn"),
    ("format string", "pwn"),
    ("apk", "mobile"),
    ("android", "mobile"),
    ("smali", "mobile"),
    ("memory dump", "forensics"),
    ("wireshark", "forensics"),
    ("pcap", "forensics"),
    ("disk image", "forensics"),
    ("xor", "crypto"),
];

pub async fn fetch_recent_writeups(client: &Client) -> CtfResult<Vec<Writeup>> {
    let url = "https://ctftime.org/writeups/rss/";
    let resp = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .send().await.map_err(|e| {
        if e.is_timeout() {
            CtfError::Timeout
        } else {
            CtfError::ExternalApi { status: 0, message: format!("HTTP request failed: {e}") }
        }
    })?;

    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(CtfError::RateLimit { retry_after: None });
    }

    if !status.is_success() {
        return Err(CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("External host returned error: {}", status),
        });
    }

    let content = resp.bytes().await.map_err(|e| CtfError::ExternalApi {
        status: status.as_u16(),
        message: format!("Failed to read response body: {e}"),
    })?;
    let channel = Channel::read_from(&content[..]).map_err(|e| CtfError::ExternalApi {
        status: status.as_u16(),
        message: format!("Failed to parse RSS: {e}"),
    })?;

    let mut writeups = Vec::new();
    for item in channel.items() {
        let title = item.title().unwrap_or("No Title").to_string();
        let url = item.link().unwrap_or("").to_string();

        // RSS link is usually: https://ctftime.org/writeup/1234
        let ctftime_id = url
            .trim_end_matches('/')
            .split('/')
            .last()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        if ctftime_id == 0 {
            continue;
        }

        let category = extract_category_from_title(&title);
        let event_name = item
            .description()
            .and_then(extract_event_name_from_description)
            .or_else(|| extract_event_name_from_title(&title));

        let published_at = item
            .pub_date()
            .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
            .map(|d| d.with_timezone(&Utc));

        writeups.push(Writeup {
            id: Uuid::new_v4(),
            ctftime_id,
            title,
            url,
            event_id: 0, // Placeholder, resolved in the task layer
            category,
            event_name,
            published_at,
            created_at: Utc::now(),
        });
    }

    Ok(writeups)
}

fn extract_category_from_title(title: &str) -> Option<String> {
    // 1. Try bracket extraction first (e.g. "[web] My Challenge")
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\[([A-Za-z][A-Za-z0-9 _-]{1,20})\]").unwrap());

    let raw = re
        .captures(title)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_lowercase());

    if let Some(cat) = raw {
        return Some(standardize_category(&cat));
    }

    // 2. Fallback: Keyword matching in the title
    let lower = title.to_lowercase();

    for (pattern, target) in KEYWORDS {
        if lower.contains(pattern) {
            return Some(target.to_string());
        }
    }

    None
}

fn standardize_category(raw: &str) -> String {
    const CATEGORY_MAP: &[(&str, &str)] = &[
        ("web", "web"),
        ("web-exploitation", "web"),
        ("web explo", "web"),
        ("crypto", "crypto"),
        ("cryptography", "crypto"),
        ("pwn", "pwn"),
        ("exploit", "pwn"),
        ("binary", "pwn"),
        ("exp", "pwn"),
        ("rev", "rev"),
        ("reverse", "rev"),
        ("reversing", "rev"),
        ("binary analysis", "rev"),
        ("forensic", "forensics"),
        ("forensics", "forensics"),
        ("dfir", "forensics"),
        ("misc", "misc"),
        ("osint", "osint"),
        ("stego", "stego"),
        ("steganography", "stego"),
        ("mobile", "mobile"),
        ("android", "mobile"),
        ("ios", "mobile"),
        ("cloud", "cloud"),
        ("hardware", "hardware"),
        ("iot", "hardware"),
    ];

    for (pattern, target) in CATEGORY_MAP {
        if raw == *pattern || raw.contains(pattern) {
            return target.to_string();
        }
    }
    raw.to_string()
}

fn extract_event_name_from_description(desc: &str) -> Option<String> {
    // CTFtime description often has "CTF: <name>"
    desc.lines()
        .find(|l| l.starts_with("CTF:"))
        .map(|l| l.trim_start_matches("CTF:").trim().to_string())
}

fn extract_event_name_from_title(title: &str) -> Option<String> {
    // If title is like "TRX CTF 2026 quantum-cipher team rawpayload"
    // We can try to take everything before the challenge name if it's a known pattern.
    // Or just look for "CTF" and the year.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^(.*? CTF \d{4})").unwrap());

    re.captures(title)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}
