use serde::Deserialize;
use shared::{CtfError, CtfResult};

const BASE: &str = "https://ctftime.org/api/v1";

#[derive(Debug, Deserialize, Clone)]
pub struct TeamEntry {
    pub team_name: String,
    pub points: f64,
    #[allow(dead_code)]
    pub team_id: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TeamSearchResult {
    pub id: i64,
    pub name: String,
    pub country: String,
    pub rating: Option<f64>,
    pub aliases: Vec<String>,
}
pub async fn request_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> CtfResult<reqwest::Response> {
    let mut retries = 0;
    let max_retries = 3;
    let mut backoff = std::time::Duration::from_secs(2);

    loop {
        let resp = client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    CtfError::Timeout
                } else {
                    CtfError::ExternalApi {
                        status: 0,
                        message: format!("Request failed: {e}"),
                    }
                }
            })?;

        let status = resp.status();

        // Retry on 429 (Rate Limit) and 5xx (Server Error)
        if (status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
            && retries < max_retries
        {
            tracing::warn!(?status, %url, ?backoff, "CTFtime API error, retrying...");
            tokio::time::sleep(backoff).await;
            retries += 1;
            backoff *= 2;
            continue;
        }

        return Ok(resp);
    }
}

pub async fn fetch_top(client: &reqwest::Client, year: i32) -> CtfResult<Vec<(u32, TeamEntry)>> {
    let url = format!("{}/top/{}/", BASE, year);
    let resp = request_with_retry(client, &url).await?;

    if !resp.status().is_success() {
        let status = resp.status();
        tracing::warn!(?status, %url, "CTFtime API returned error for top stats after retries");
        return Err(CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("CTFtime returned error for top stats: {}", status),
        });
    }

    let val: serde_json::Value = resp.json().await.map_err(|e| CtfError::ExternalApi {
        status: 0,
        message: format!("JSON parse failed: {e}"),
    })?;

    let mut entries: Vec<(u32, TeamEntry)> = Vec::new();

    // CTFtime API can be inconsistent:
    // 1. Array: [ { team... }, { team... } ]
    // 2. Map with year: { "2026": [ { team... }, ... ] }
    // 3. Map with ranks: { "1": { team... }, "2": { ... } }
    // 4. Map with year and ranks: { "2026": { "1": { ... }, "2": { ... } } }

    if let Some(arr) = val.as_array() {
        // Case 1: Direct array
        for (i, v) in arr.iter().enumerate() {
            let rank = (i + 1) as u32;
            let entry: TeamEntry =
                serde_json::from_value(v.clone()).map_err(|e| CtfError::ExternalApi {
                    status: 0,
                    message: format!("Invalid team entry at index {i}: {e}"),
                })?;
            entries.push((rank, entry));
        }
    } else if let Some(obj) = val.as_object() {
        // Case 2, 3, or 4: Object wrapper
        let (target_obj, is_array) = if obj.len() == 1 {
            let inner = obj.values().next().unwrap();
            if let Some(inner_arr) = inner.as_array() {
                (Some(inner_arr), None)
            } else if let Some(inner_obj) = inner.as_object() {
                (None, Some(inner_obj))
            } else {
                (None, Some(obj))
            }
        } else {
            (None, Some(obj))
        };

        if let Some(arr) = target_obj {
            for (i, v) in arr.iter().enumerate() {
                let rank = (i + 1) as u32;
                let entry: TeamEntry =
                    serde_json::from_value(v.clone()).map_err(|e| CtfError::ExternalApi {
                        status: 0,
                        message: format!("Invalid team entry at index {i}: {e}"),
                    })?;
                entries.push((rank, entry));
            }
        } else if let Some(map) = is_array {
            for (k, v) in map {
                let rank: u32 = k.parse().map_err(|_| CtfError::ExternalApi {
                    status: 0,
                    message: format!("Invalid rank key: {k}"),
                })?;
                let entry: TeamEntry =
                    serde_json::from_value(v.clone()).map_err(|e| CtfError::ExternalApi {
                        status: 0,
                        message: format!("Invalid team entry at rank {rank}: {e}"),
                    })?;
                entries.push((rank, entry));
            }
        }
    } else {
        return Err(CtfError::ExternalApi {
            status: 0,
            message: "Expected JSON object or array for top stats".into(),
        });
    }

    entries.sort_by_key(|(rank, _)| *rank);
    Ok(entries)
}

pub async fn search_team(
    client: &reqwest::Client,
    query: &str,
) -> CtfResult<Vec<TeamSearchResult>> {
    let query = query.trim();
    if query.is_empty() || query.len() > 100 {
        return Ok(vec![]);
    }

    // Only allow alphanumeric, space, and common symbols to prevent exploitation/leaks
    if !query
        .chars()
        .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
    {
        return Ok(vec![]);
    }

    let url = format!("{}/teams/?search={}", BASE, urlencoding::encode(query));
    let resp = request_with_retry(client, &url).await?;

    if !resp.status().is_success() {
        let status = resp.status();
        tracing::warn!(?status, %url, "CTFtime API returned error for team search after retries");
        return Err(CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("CTFtime returned error for team search: {}", status),
        });
    }

    resp.json().await.map_err(|e| CtfError::ExternalApi {
        status: 0,
        message: format!("JSON parse failed: {e}"),
    })
}

pub async fn get_team_name(client: &reqwest::Client, team_id: i64) -> CtfResult<Option<String>> {
    let url = format!("{}/teams/{}/", BASE, team_id);
    let resp = request_with_retry(client, &url).await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !resp.status().is_success() {
        let status = resp.status();
        tracing::warn!(?status, %url, team_id, "CTFtime API returned error for team lookup after retries");
        return Err(CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("CTFtime returned error for team {team_id}: {}", status),
        });
    }

    let val: serde_json::Value = resp.json().await.map_err(|e| CtfError::ExternalApi {
        status: 0,
        message: format!("JSON parse failed: {e}"),
    })?;
    Ok(val
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}
