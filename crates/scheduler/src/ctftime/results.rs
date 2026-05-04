use html_scraper::{ElementRef, Html, Selector};
use reqwest::Client;
use shared::TeamResult;
use shared::{CtfError, CtfResult};
use std::sync::OnceLock;
use uuid::Uuid;

fn sel_table_row() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("table.table tr").expect("valid CSS selector"))
}

fn sel_anchor() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("a").expect("valid CSS selector"))
}

pub async fn fetch_event_results(client: &Client, ctftime_id: i64) -> CtfResult<Vec<TeamResult>> {
    let url = format!("https://ctftime.org/event/{}/results/", ctftime_id);
    let resp = client.get(&url).send().await.map_err(|e| {
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

    let html = resp.text().await.map_err(|e| CtfError::ExternalApi {
        status: status.as_u16(),
        message: format!("Failed to read response body: {e}"),
    })?;
    Ok(parse_results_from_html(&html, ctftime_id))
}

pub fn parse_results_from_html(html: &str, ctftime_id: i64) -> Vec<TeamResult> {
    let document = Html::parse_document(html);
    let mut results = Vec::new();
    let rows = document.select(sel_table_row());

    for row in rows.skip(1) {
        // Skip header row
        let cols: Vec<ElementRef> = row.children().filter_map(ElementRef::wrap).collect();
        if cols.len() < 3 {
            continue;
        }

        // Column 0: Place
        let place_text = cols[0].text().collect::<String>().trim().to_string();
        let place: Option<i32> = place_text.parse().ok();

        // Column 1: Team (contains an <a> with /team/{id})
        let team_id = cols[1].select(sel_anchor()).next().and_then(|a| {
            a.value()
                .attr("href")
                .and_then(|h| h.split('/').rfind(|s| !s.is_empty()))
                .and_then(|s| s.parse::<i64>().ok())
        });

        // Column 2: Score
        let score_text = cols[2].text().collect::<String>().trim().to_string();
        let score: Option<f64> = score_text.parse().ok();

        if let Some(tid) = team_id {
            results.push(TeamResult {
                id: Uuid::new_v4(),
                ctftime_team_id: tid,
                ctf_event_id: ctftime_id,
                place,
                score,
                total_teams: None,
                notified_at: None,
                created_at: chrono::Utc::now(),
            });
        }
    }

    let total_teams = results.len() as i32;
    for r in &mut results {
        r.total_teams = Some(total_teams);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_results_fixture() {
        let html = include_str!("../../tests/fixtures/results.html");
        let results = parse_results_from_html(html, 1234);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].place, Some(1));
        assert_eq!(results[0].ctftime_team_id, 123);
        assert_eq!(results[0].score, Some(1337.0));
        assert_eq!(results[0].total_teams, Some(3));

        assert_eq!(results[1].place, Some(2));
        assert_eq!(results[1].ctftime_team_id, 456);
        assert_eq!(results[1].score, Some(1000.5));

        assert_eq!(results[2].place, Some(3));
        assert_eq!(results[2].ctftime_team_id, 789);
        assert_eq!(results[2].score, Some(500.0));
    }
}
