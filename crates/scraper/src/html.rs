use anyhow::{Context, Result};
use html_scraper::{Html, Selector};
use reqwest::Client;

use crate::models::HtmlEventPatch;

pub async fn fetch_event_patch(client: &Client, ctftime_id: i64) -> Result<HtmlEventPatch> {
    let url = format!("https://ctftime.org/event/{}", ctftime_id);
    let html = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch event page")?
        .error_for_status()
        .context("CTFTime event page returned an error status")?
        .text()
        .await
        .context("Failed to read event page HTML")?;

    let document = Html::parse_document(&html);

    let title = extract_meta(&document, "og:title").or_else(|| extract_heading(&document));
    let description = extract_meta(&document, "og:description");
    let format = find_value_from_text_nodes(&document, "Format:");
    let weight = find_value_from_text_nodes(&document, "Rating weight:")
        .and_then(|value| value.split_whitespace().next().and_then(|part| part.parse().ok()));

    Ok(HtmlEventPatch {
        title,
        url: None,
        description,
        format,
        weight,
    })
}

fn extract_meta(document: &Html, key: &str) -> Option<String> {
    let selector = Selector::parse("meta").ok()?;

    for element in document.select(&selector) {
        let value = element.value();
        let matches = value
            .attr("property")
            .map(|attr| attr == key)
            .unwrap_or(false)
            || value
                .attr("name")
                .map(|attr| attr == key)
                .unwrap_or(false);
        if matches {
            if let Some(content) = value.attr("content") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

fn extract_heading(document: &Html) -> Option<String> {
    let selectors = ["h1", "h2"];
    for selector in selectors {
        let selector = Selector::parse(selector).ok()?;
        if let Some(element) = document.select(&selector).next() {
            let text = element.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    None
}

fn find_value_from_text_nodes(document: &Html, label: &str) -> Option<String> {
    for text in document.root_element().text() {
        let candidate = text.trim();
        if let Some(value) = extract_after_label(candidate, label) {
            return Some(value);
        }
    }

    None
}

fn extract_after_label(text: &str, label: &str) -> Option<String> {
    let index = text.find(label)?;
    let value = text[index + label.len()..].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
