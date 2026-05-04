use reqwest::Client;
use shared::{CtfError, CtfResult};

use crate::ctftime::models::RawCtftimeEvent;
use tracing::warn;

pub async fn request_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> CtfResult<reqwest::Response> {
    let mut retries = 0;
    let max_retries = 3;
    let mut backoff = std::time::Duration::from_secs(2);

    loop {
        let start = std::time::Instant::now();
        let resp_result = client.get(url).send().await;
        let duration = start.elapsed();

        // All scheduler requests currently go to /events/ (upcoming)
        let endpoint = "upcoming";

        metrics::histogram!(
            shared::metrics::CTFTIME_API_LATENCY,
            "endpoint" => endpoint
        )
        .record(duration.as_secs_f64());

        let resp = resp_result.map_err(|e| {
            metrics::counter!(
                shared::metrics::CTFTIME_API_REQUESTS_TOTAL,
                "endpoint" => endpoint,
                "status"   => "error"
            )
            .increment(1);

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
        metrics::counter!(
            shared::metrics::CTFTIME_API_REQUESTS_TOTAL,
            "endpoint" => endpoint,
            "status"   => status.as_u16().to_string()
        )
        .increment(1);

        // Retry on 429 (Rate Limit) and 5xx (Server Error)
        if (status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
            && retries < max_retries
        {
            warn!(?status, %url, ?backoff, "CTFtime API error, retrying...");
            tokio::time::sleep(backoff).await;
            retries += 1;
            backoff *= 2;
            continue;
        }

        return Ok(resp);
    }
}

pub async fn fetch_upcoming(client: &Client) -> CtfResult<Vec<RawCtftimeEvent>> {
    let url = format!("{}/events/?limit=50", shared::CTFTIME_API_BASE);
    let resp = request_with_retry(client, &url).await?;

    let status = resp.status();
    if !status.is_success() {
        return Err(CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("CTFTime API returned error after retries: {}", status),
        });
    }

    resp.json::<Vec<RawCtftimeEvent>>()
        .await
        .map_err(|e| CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("Failed to deserialize CTFTime JSON response: {e}"),
        })
}
