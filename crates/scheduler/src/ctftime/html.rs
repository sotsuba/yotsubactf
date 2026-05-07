//! HTML scraping helpers.
//!
//! Two responsibilities:
//! 1. `fetch_event_patch` — fetches the CTFTime event page and extracts:
//! - Fallback title / description / format / weight when the REST API
//!   returns blank fields.
//! - Social/community invite links found on that page.
//!
//! This is the **single** CTFTime fetch per enrichment cycle; callers must
//! not separately call the old `fetch_social_links` (removed).
//!
//! 2. `fetch_external_social_links` — fetches only the CTF's own website
//!    (one level deep) for additional invite links.
//!
//! Performance notes
//! ─────────────────
//! • CSS selectors are compiled once and cached via `OnceLock` to avoid
//!   re-parsing the same selector string for every event.
//! • All outbound fetches carry a hard `HTML_FETCH_TIMEOUT` so a slow
//!   external server cannot stall a scrape cycle.
//! • The User-Agent is consistent with the rest of the bot and includes a
//!   contact address, which is CTFTime's documented requirement.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use html_scraper::{Html, Selector};
use reqwest_middleware::ClientWithMiddleware as Client;
use shared::{CtfError, CtfResult};

use shared::{SocialLink, SocialPlatform};

use crate::ctftime::models::HtmlEventPatch;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Hard timeout for every outbound HTML fetch.
///
/// Prevents a slow or stalled external server from blocking a scrape cycle.
/// CTFTime responses are typically <200 ms; external CTF sites are given
/// generous headroom.
const HTML_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

// ── Cached selectors ──────────────────────────────────────────────────────────
//
// `Selector::parse` compiles a CSS selector string into an internal
// representation. Doing that inside a hot path (once per document, per event)
// is unnecessary work. `OnceLock` initialises each selector on first use and
// reuses it for the lifetime of the process.

fn sel_a_href() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("a[href]").expect("valid CSS selector"))
}

fn sel_meta() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("meta").expect("valid CSS selector"))
}

fn sel_headings() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("h1, h2").expect("valid CSS selector"))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Fetch the CTFTime event page and extract patch data + on-page social links.
///
/// This is the **only** place the CTFTime HTML is fetched per enrichment cycle.
/// Callers should merge the returned [`HtmlEventPatch`] via
/// [`EnrichedEvent::apply_patch`], which fills blank API fields and merges
/// social links in a single operation.
pub async fn fetch_event_patch(client: &Client, ctftime_id: i64) -> CtfResult<HtmlEventPatch> {
    let url = format!("https://ctftime.org/event/{ctftime_id}");
    let html = fetch_html(client, &url).await?;
    let document = Html::parse_document(&html);

    let title = extract_meta(&document, "og:title").or_else(|| extract_heading(&document));
    let description = extract_meta(&document, "og:description");
    let format = find_value_from_text_nodes(&document, "Format:");
    let weight = find_value_from_text_nodes(&document, "Rating weight:")
        .and_then(|v| v.split_whitespace().next().and_then(|p| p.parse().ok()));
    let is_onsite =
        find_value_from_text_nodes(&document, "Onsite:").map(|v| v.to_lowercase() == "yes");

    let mut found: HashMap<String, SocialLink> = HashMap::new();
    extract_social_links_from_document(&document, &mut found);

    Ok(HtmlEventPatch {
        title,
        description,
        format,
        weight,
        is_onsite,
        social_links: found.into_values().collect(),
    })
}

/// Fetch social/community invite links from the CTF's own website.
///
/// This is a **separate** fetch from the CTFTime page. Call it after
/// [`fetch_event_patch`] to collect links that may not appear on CTFTime.
///
/// Returns an empty vec (and logs nothing) if the URL is absent, looks
/// unsafe, or the fetch fails — this is a best-effort enrichment.
pub async fn fetch_external_social_links(client: &Client, ctf_url: &str) -> Vec<SocialLink> {
    if ctf_url.is_empty() || !is_safe_external_url(ctf_url) {
        return vec![];
    }

    let mut found: HashMap<String, SocialLink> = HashMap::new();
    if let Ok(html) = fetch_html(client, ctf_url).await {
        let doc = Html::parse_document(&html);
        extract_social_links_from_document(&doc, &mut found);
    }

    let mut links: Vec<SocialLink> = found.into_values().collect();
    links.sort_by(|a, b| a.platform.cmp(&b.platform));
    links
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn fetch_html(client: &Client, url: &str) -> CtfResult<String> {
    let resp = client
        .get(url)
        .timeout(HTML_FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                CtfError::Timeout
            } else {
                CtfError::ExternalApi {
                    status: 0,
                    message: format!("HTTP request failed: {e}"),
                }
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

    if let Some(content_type) = resp.headers().get(reqwest::header::CONTENT_TYPE)
        && let Ok(ct) = content_type.to_str()
    {
        let ct = ct.to_lowercase();
        if !ct.contains("text/html") && !ct.contains("application/xhtml+xml") {
            return Err(CtfError::ExternalApi {
                status: status.as_u16(),
                message: format!("Non-HTML content type: {}", ct),
            });
        }
    }

    resp.text().await.map_err(|e| CtfError::ExternalApi {
        status: status.as_u16(),
        message: format!("Failed to read response body: {e}"),
    })
}

/// Walk every `<a href>` and bare text node, classify each URL, and insert
/// confirmed social links into `found` (keyed by URL to deduplicate).
fn extract_social_links_from_document(document: &Html, found: &mut HashMap<String, SocialLink>) {
    // ── <a href="..."> anchors ────────────────────────────────────────────
    for el in document.select(sel_a_href()) {
        if let Some(href) = el.value().attr("href")
            && let Some(link) = classify_url(href)
        {
            found.entry(link.url.clone()).or_insert(link);
        }
    }

    // ── Bare text nodes (links pasted as plain text, not wrapped in <a>) ──
    for text in document.root_element().text() {
        for token in text.split_whitespace() {
            let token = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.');
            if let Some(link) = classify_url(token) {
                found.entry(link.url.clone()).or_insert(link);
            }
        }
    }
}

/// Map a raw URL/token to a [`SocialLink`], or `None` if no platform matches.
fn classify_url(raw: &str) -> Option<SocialLink> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // Normalise protocol-relative URLs and bare invite hostnames.
    let normalised = if raw.starts_with("//") {
        format!("https:{raw}")
    } else if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else if raw.starts_with("discord.gg/")
        || raw.starts_with("t.me/")
        || raw.starts_with("telegram.me/")
    {
        format!("https://{raw}")
    } else {
        return None;
    };

    let lower = normalised.to_lowercase();

    // ── Discord ────────────────────────────────────────────────────────────
    if lower.contains("discord.gg/") || lower.contains("discord.com/invite/") {
        let path = extract_path(&normalised);
        if path == "/" || path.is_empty() {
            return None;
        }
        return Some(SocialLink {
            platform: SocialPlatform::Discord,
            url: normalised,
        });
    }

    // ── Telegram ───────────────────────────────────────────────────────────
    if lower.contains("t.me/") || lower.contains("telegram.me/") {
        if lower.contains("/share") {
            return None;
        }
        return Some(SocialLink {
            platform: SocialPlatform::Telegram,
            url: normalised,
        });
    }

    // ── Slack ──────────────────────────────────────────────────────────────
    if lower.contains(".slack.com") || lower.contains("slack.com/join") {
        return Some(SocialLink {
            platform: SocialPlatform::Slack,
            url: normalised,
        });
    }

    // ── Matrix ─────────────────────────────────────────────────────────────
    if lower.contains("matrix.to/#/") {
        return Some(SocialLink {
            platform: SocialPlatform::Matrix,
            url: normalised,
        });
    }

    // ── IRC ────────────────────────────────────────────────────────────────
    if lower.starts_with("irc://") || lower.starts_with("ircs://") {
        return Some(SocialLink {
            platform: SocialPlatform::Irc,
            url: normalised,
        });
    }

    None
}

/// Return `true` if `url` is safe to fetch as an external CTF site.
///
/// Rejects non-HTTP URLs, loops back to CTFTime, known binary media
/// extensions, and most importantly: private/internal IP addresses and hostnames.
fn is_safe_external_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };

    // Only allow HTTP/HTTPS
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }

    // Use parsed host representation to handle IPv4, IPv6 (without brackets), and Domains.
    match parsed.host() {
        Some(url::Host::Ipv4(v4)) => {
            if v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
            {
                return false;
            }
        }
        Some(url::Host::Ipv6(v6)) => {
            // IPv6 check: loopback, unspecified, multicast, and ULA (Unique Local Address)
            if v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
            {
                return false;
            }
        }
        Some(url::Host::Domain(host)) => {
            let lower_host = host.to_lowercase();
            // Block obvious internal hostnames and CTFTime loopback
            if matches!(
                lower_host.as_str(),
                "localhost" | "metadata.google.internal"
            ) || lower_host.contains("ctftime.org")
            {
                return false;
            }
        }
        None => return false,
    }

    // Retain binary extension filtering
    let lower_path = parsed.path().to_lowercase();
    let skip_exts = [
        ".pdf", ".zip", ".tar", ".gz", ".png", ".jpg", ".svg", ".exe",
    ];
    if skip_exts.iter().any(|ext| lower_path.ends_with(ext)) {
        return false;
    }

    true
}

fn extract_path(url: &str) -> &str {
    url.find("://")
        .and_then(|i| url[i + 3..].find('/').map(|j| &url[i + 3 + j..]))
        .unwrap_or("")
}

// ── Text-extraction helpers ───────────────────────────────────────────────────

fn extract_meta(document: &Html, key: &str) -> Option<String> {
    for element in document.select(sel_meta()) {
        let value = element.value();
        let matches = value.attr("property").map(|a| a == key).unwrap_or(false)
            || value.attr("name").map(|a| a == key).unwrap_or(false);
        if matches && let Some(content) = value.attr("content") {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_heading(document: &Html) -> Option<String> {
    document.select(sel_headings()).find_map(|el| {
        let text = el.text().collect::<String>();
        let text = text.trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    })
}

fn find_value_from_text_nodes(document: &Html, label: &str) -> Option<String> {
    document
        .root_element()
        .text()
        .find_map(|text| extract_after_label(text.trim(), label))
}

fn extract_after_label(text: &str, label: &str) -> Option<String> {
    let idx = text.find(label)?;
    let value = text[idx + label.len()..].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_external_url() {
        // Safe URLs
        assert!(is_safe_external_url("https://ctf.example.com"));
        assert!(is_safe_external_url("http://another-ctf.org/path?query=1"));

        // Unsafe schemes
        assert!(!is_safe_external_url("ftp://ctf.com"));
        assert!(!is_safe_external_url("file:///etc/passwd"));

        // CTFTime loopback
        assert!(!is_safe_external_url("https://ctftime.org/event/1"));
        assert!(!is_safe_external_url("http://sub.ctftime.org"));

        // Internal hostnames
        assert!(!is_safe_external_url("http://localhost"));
        assert!(!is_safe_external_url("http://localhost:8080"));
        assert!(!is_safe_external_url("http://metadata.google.internal"));

        // Private/Reserved IPv4
        assert!(!is_safe_external_url("http://127.0.0.1"));
        assert!(!is_safe_external_url("http://192.168.1.1"));
        assert!(!is_safe_external_url("http://10.0.0.1"));
        assert!(!is_safe_external_url("http://172.16.0.1"));
        assert!(!is_safe_external_url("http://169.254.169.254"));
        assert!(!is_safe_external_url("http://0.0.0.0"));
        assert!(!is_safe_external_url("http://255.255.255.255"));

        // Private/Reserved IPv6
        assert!(!is_safe_external_url("http://[::1]"));
        assert!(!is_safe_external_url("http://[::]"));
        assert!(!is_safe_external_url("http://[fd00::1]")); // ULA

        // Binary extensions
        assert!(!is_safe_external_url("https://ctf.com/rules.pdf"));
        assert!(!is_safe_external_url("https://ctf.com/assets.zip"));
    }
}
