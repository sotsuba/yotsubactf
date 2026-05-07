use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::warn;

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const MAX_SUMMARY_CHARS: usize = 400;
const MAX_DESCRIPTION_CHARS: usize = 320;
const MAX_DIGEST_CHARS: usize = 500;
const MAX_INPUT_CHARS: usize = 6000;
const ADAPTIVE_SUCCESS_DECREASE_MS: u64 = 100;

#[derive(Debug)]
struct GatedState {
    next_allowed: Instant,
    min_interval: Duration,
    max_interval: Duration,
    current_interval: Duration,
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    http: reqwest_middleware::ClientWithMiddleware,
    api_key: String,
    model: String,
    timeout: Duration,
    digest_timeout: Duration,
    gated_state: Arc<Mutex<GatedState>>,
}

impl GeminiClient {
    pub fn new(
        http: reqwest_middleware::ClientWithMiddleware,
        api_key: String,
        model: String,
        timeout: Duration,
        digest_timeout: Duration,
        gated_min_interval: Duration,
        gated_max_interval: Duration,
    ) -> Self {
        Self {
            http,
            api_key,
            model,
            timeout,
            digest_timeout,
            gated_state: Arc::new(Mutex::new(GatedState {
                next_allowed: Instant::now(),
                min_interval: gated_min_interval,
                max_interval: gated_max_interval,
                current_interval: gated_min_interval,
            })),
        }
    }

    pub async fn classify_category(&self, title: &str) -> Option<String> {
        let prompt = format!(
            "Return exactly one category from this list: web, crypto, pwn, rev, forensics, stego, osint, mobile, cloud, hardware, misc.\n\nTitle: {title}\n\nOnly return the category (lowercase)."
        );
        let raw = self
            .request_text("category", &prompt, self.timeout, 12, 0.2)
            .await?;
        let cleaned = normalize_line(&raw);
        if cleaned.is_empty() {
            return None;
        }
        Some(cleaned)
    }

    pub async fn summarize_writeup(&self, title: &str, content: &str) -> Option<String> {
        let content = truncate_chars(content, MAX_INPUT_CHARS);
        let prompt = format!(
            "Write a concise 2-3 line summary in English. Each line must start with '- '.\n\
             Keep the total under {MAX_SUMMARY_CHARS} characters.\n\
             Use only facts from the content; no extra commentary.\n\n\
             Title: {title}\n\nContent:\n{content}\n\nSummary:\n",
        );

        let raw = self
            .request_text("writeup_summary", &prompt, self.timeout, 160, 0.2)
            .await?;
        let normalized = normalize_bullets(&raw, MAX_SUMMARY_CHARS);
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    }

    pub async fn clean_event_description(&self, title: &str, description: &str) -> Option<String> {
        let description = truncate_chars(description, MAX_INPUT_CHARS);
        let prompt = format!(
            "Rewrite the description into 1-2 short English sentences.\n\
             Remove HTML, boilerplate, and noise.\n\
             Preserve key notes like beginner-friendly, hardware requirements, or prize pool if present.\n\
             If there is nothing useful, return an empty string.\n\n\
             Title: {title}\n\nDescription:\n{description}\n\nCleaned description:",
        );

        let raw = self
            .request_text("event_description", &prompt, self.timeout, 120, 0.2)
            .await?;
        let cleaned = normalize_sentence(&raw, MAX_DESCRIPTION_CHARS);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }

    pub async fn digest_narrative(&self, data: &str) -> Option<String> {
        let data = truncate_chars(data, MAX_INPUT_CHARS);
        let prompt = format!(
            "Write a short weekly CTF digest in English, 2-3 sentences.\n\
             No markdown, no bullet points. Keep under {MAX_DIGEST_CHARS} characters.\n\
             Highlight notable events (high weight, prestigious, beginner-friendly) and timing.\n\n\
             Data:\n{data}\n\nDigest:",
        );

        let raw = self
            .request_text("digest", &prompt, self.digest_timeout, 180, 0.3)
            .await?;
        let cleaned = normalize_sentence(&raw, MAX_DIGEST_CHARS);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }

    async fn request_text(
        &self,
        op: &str,
        prompt: &str,
        timeout: Duration,
        max_tokens: u32,
        temperature: f32,
    ) -> Option<String> {
        let gated = matches!(op, "category" | "event_description");
        if gated {
            self.throttle_gated_ops().await;
        }
        let url = format!(
            "{}/{model}:generateContent",
            GEMINI_API_BASE,
            model = self.model
        );

        let payload = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": max_tokens
            }
        });

        let start = std::time::Instant::now();
        metrics::counter!(shared::metrics::SCHEDULER_LLM_REQUESTS_TOTAL, "op" => op.to_string())
            .increment(1);

        let response = tokio::time::timeout(timeout, async {
            let resp = self
                .http
                .post(&url)
                .header("x-goog-api-key", &self.api_key)
                .json(&payload)
                .send()
                .await
                .map_err(|e| format!("request failed: {e}"))?;

            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && gated {
                self.on_rate_limited().await;
            }

            let resp = resp
                .error_for_status()
                .map_err(|e| format!("unexpected status: {e}"))?;

            resp.json::<GeminiResponse>()
                .await
                .map_err(|e| format!("invalid response: {e}"))
        })
        .await;

        let elapsed = start.elapsed();
        metrics::histogram!(shared::metrics::SCHEDULER_LLM_LATENCY, "op" => op.to_string())
            .record(elapsed.as_secs_f64());

        match response {
            Ok(Ok(body)) => {
                if gated {
                    self.on_success().await;
                }
                let text = body
                    .candidates
                    .as_ref()
                    .and_then(|c| c.first())
                    .and_then(|c| c.content.as_ref())
                    .and_then(|c| c.parts.as_ref())
                    .and_then(|p| p.first())
                    .and_then(|p| p.text.as_ref())
                    .map(|t| t.trim().to_string());

                if text.as_deref().unwrap_or("").is_empty() {
                    metrics::counter!(shared::metrics::SCHEDULER_LLM_FAILURE_TOTAL, "op" => op.to_string(), "reason" => "empty")
                        .increment(1);
                    warn!(op, "Gemini returned empty response");
                    return None;
                }

                Some(text.unwrap_or_default())
            }
            Ok(Err(err)) => {
                let reason = if err.contains("429") {
                    "rate_limit"
                } else {
                    "error"
                };
                metrics::counter!(shared::metrics::SCHEDULER_LLM_FAILURE_TOTAL, "op" => op.to_string(), "reason" => reason)
                    .increment(1);
                warn!(op, error = %err, "Gemini request failed");
                None
            }
            Err(_) => {
                metrics::counter!(shared::metrics::SCHEDULER_LLM_FAILURE_TOTAL, "op" => op.to_string(), "reason" => "timeout")
                    .increment(1);
                warn!(op, "Gemini request timed out");
                None
            }
        }
    }
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

#[derive(Deserialize)]
struct GeminiContent {
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

fn normalize_line(text: &str) -> String {
    text.lines()
        .find_map(|line| {
            let cleaned = line.trim().trim_matches(|c: char| !c.is_alphanumeric());
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned.to_lowercase())
            }
        })
        .unwrap_or_default()
}

impl GeminiClient {
    async fn throttle_gated_ops(&self) {
        let wait_until = {
            let mut state = self.gated_state.lock().await;
            let now = Instant::now();
            let start_at = if now < state.next_allowed {
                state.next_allowed
            } else {
                now
            };
            // Reserve the NEXT slot by advancing next_allowed
            state.next_allowed = start_at + state.current_interval;
            start_at
        };

        let now = Instant::now();
        if now < wait_until {
            tokio::time::sleep(wait_until - now).await;
        }
    }

    async fn on_rate_limited(&self) {
        let mut state = self.gated_state.lock().await;
        // Exponentially increase interval on 429
        state.current_interval = std::cmp::min(state.current_interval * 2, state.max_interval);
        // Push next_allowed into the future
        state.next_allowed = Instant::now() + state.current_interval;
        warn!(
            interval_ms = state.current_interval.as_millis(),
            "Adaptive rate limit: backing off"
        );
    }

    async fn on_success(&self) {
        let mut state = self.gated_state.lock().await;
        // Gradually decrease interval on success
        if state.current_interval > state.min_interval {
            let decrease = Duration::from_millis(ADAPTIVE_SUCCESS_DECREASE_MS);
            state.current_interval = if state.current_interval > state.min_interval + decrease {
                state.current_interval - decrease
            } else {
                state.min_interval
            };
        }
    }
}

fn normalize_bullets(text: &str, max_len: usize) -> String {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_start_matches(['-', '\u{2022}', '*']))
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .take(3)
        .map(|line| format!("- {line}"))
        .collect();

    if lines.is_empty() {
        return String::new();
    }

    let mut summary = lines.join("\n");
    if summary.chars().count() > max_len {
        summary = truncate_chars(&summary, max_len);
    }
    summary
}

fn normalize_sentence(text: &str, max_len: usize) -> String {
    let mut cleaned = text
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    cleaned = cleaned.replace("  ", " ").trim().to_string();
    if cleaned.chars().count() > max_len {
        cleaned = truncate_chars(&cleaned, max_len);
    }
    cleaned
}

fn truncate_chars(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    text.chars().take(max_len).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_line_extracts_category() {
        let input = "  [Web]  ";
        assert_eq!(normalize_line(input), "web");
    }

    #[test]
    fn normalize_bullets_formats_lines() {
        let input = "* one\n- two\n\u{2022} three\n- four";
        let out = normalize_bullets(input, 200);
        assert_eq!(out.lines().count(), 3);
        assert!(out.starts_with("- one"));
    }

    #[test]
    fn normalize_sentence_compacts_whitespace() {
        let input = "Line one.\n\nLine two.";
        let out = normalize_sentence(input, 200);
        assert_eq!(out, "Line one. Line two.");
    }

    #[test]
    fn truncate_chars_limits_length() {
        let input = "abcdef";
        assert_eq!(truncate_chars(input, 3), "abc");
        assert_eq!(truncate_chars(input, 10), "abcdef");
    }

    #[tokio::test]
    async fn adaptive_interval_increases_and_decays() {
        let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
        let client = GeminiClient::new(
            http,
            "key".to_string(),
            "model".to_string(),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_millis(500),
            Duration::from_millis(2000),
        );

        client.on_rate_limited().await;
        {
            let state = client.gated_state.lock().await;
            assert_eq!(state.current_interval, Duration::from_millis(1000));
        }

        client.on_rate_limited().await;
        {
            let state = client.gated_state.lock().await;
            assert_eq!(state.current_interval, Duration::from_millis(2000));
        }

        client.on_success().await;
        {
            let state = client.gated_state.lock().await;
            assert_eq!(state.current_interval, Duration::from_millis(1900));
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_gemini_smoke_test() {
        let api_key = std::env::var("GEMINI_API_KEY").ok();
        if api_key.as_deref().unwrap_or("").is_empty() {
            return;
        }

        let model =
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());
        let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
        let client = GeminiClient::new(
            http,
            api_key.unwrap(),
            model,
            Duration::from_secs(20),
            Duration::from_secs(6),
            Duration::from_millis(1000),
            Duration::from_millis(5000),
        );

        let category = client
            .classify_category("HackTheBoo 2024 - Spooky Time [web] pickle deserializer RCE")
            .await;
        assert!(category.is_some());

        let digest = client
            .digest_narrative("Current events: 2\n- DEFCON Quals | format: Jeopardy | weight: 90.0 | ends in 2 day(s)")
            .await;
        assert!(digest.is_some());
    }
}
